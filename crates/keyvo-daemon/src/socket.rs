//! The control socket (spec §8, `docs/socket-api.md`): a Unix stream
//! socket, mode `0600` in the private runtime directory, speaking the
//! newline-delimited JSON of `keyvo_core::socket`.
//!
//! Every connection is served by its own task. Requests on one connection
//! are answered in order, each by one response line; a line that is not a
//! request gets an error response and the connection stays open. After
//! `subscribe` the connection also carries the events of the chosen
//! classes, interleaved with later responses. A subscriber that falls more
//! than the event backlog behind loses the oldest events (logged).
//!
//! [`Limits`] bound what clients can make the daemon hold: a line longer
//! than `[daemon].socket_line_bytes` gets an error response and the
//! connection is closed, a connection over `[daemon].socket_connections`
//! gets an `unavailable` error and is closed, and a failed `accept` waits
//! `[timing].accept_backoff` before the next one.

use std::io;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use keyvo_core::config::Config;
use keyvo_core::dialpad::Control;
use keyvo_core::socket::{
    ConfigStatus, DeviceStatus, ErrorCode, Event as Wire, EventClass, Pressed, Reply, Request,
    Response, Subscribed,
};
use keyvo_core::state::View;
use keyvo_core::trace::TraceRecord;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Semaphore, broadcast, mpsc, oneshot};

use crate::daemon::{self, Answer, Event};

/// The socket's file mode: the user alone (`SECURITY.md`).
pub const SOCKET_MODE: u32 = 0o600;

/// What a [`Server`] allows its clients (spec §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Longest request line in bytes, without its newline.
    pub line_bytes: usize,
    /// Connections served at once.
    pub connections: usize,
    /// Wait after a failed `accept`.
    pub accept_backoff: Duration,
}

impl Limits {
    /// The limits `config` sets: `[daemon]` and `[timing].accept_backoff`.
    #[must_use]
    pub fn from_config(config: &Config) -> Self {
        Self {
            line_bytes: config.daemon.socket_line_bytes.get(),
            connections: config.daemon.socket_connections.get(),
            accept_backoff: config.timing.accept_backoff,
        }
    }
}

impl Default for Limits {
    /// The documented defaults.
    fn default() -> Self {
        Self::from_config(&Config::default())
    }
}

/// A bound control socket. Dropping it removes the socket file.
#[derive(Debug)]
pub struct Server {
    listener: UnixListener,
    path: PathBuf,
}

impl Server {
    /// Binds the socket at `path`, creating its directory private to this
    /// user. A socket file left by a daemon that is gone is replaced.
    ///
    /// # Errors
    ///
    /// When the directory is not private, another daemon answers on
    /// `path`, something else than a socket is in the way, or the bind
    /// fails.
    pub fn bind(path: &Path) -> io::Result<Self> {
        if let Some(dir) = path.parent() {
            crate::runtime::ensure_private(dir)?;
        }
        match std::fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_socket() => {
                if std::os::unix::net::UnixStream::connect(path).is_ok() {
                    return Err(io::Error::new(
                        io::ErrorKind::AddrInUse,
                        format!("another keyvo daemon serves {}", path.display()),
                    ));
                }
                std::fs::remove_file(path)?;
            }
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("{} exists and is not a socket", path.display()),
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let listener = UnixListener::bind(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(SOCKET_MODE))?;
        Ok(Self {
            listener,
            path: path.to_path_buf(),
        })
    }

    /// The socket file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Accepts clients within `limits` until the future is dropped; their
    /// commands go to the controller on `requests`.
    pub async fn serve(&self, requests: mpsc::UnboundedSender<daemon::Request>, limits: Limits) {
        let slots = Arc::new(Semaphore::new(
            limits.connections.min(Semaphore::MAX_PERMITS),
        ));
        loop {
            match self.listener.accept().await {
                Ok((stream, _)) => match Arc::clone(&slots).try_acquire_owned() {
                    Ok(slot) => {
                        let requests = requests.clone();
                        tokio::spawn(async move {
                            connection(stream, requests, limits.line_bytes).await;
                            drop(slot);
                        });
                    }
                    Err(_) => refuse(&stream, limits.connections),
                },
                Err(error) => {
                    tracing::warn!(
                        "socket accept failed: {error}; next try in {} ms",
                        limits.accept_backoff.as_millis()
                    );
                    tokio::time::sleep(limits.accept_backoff).await;
                }
            }
        }
    }
}

/// Tells a client over the connection cap so, without waiting for it; the
/// connection closes when `stream` is dropped.
fn refuse(stream: &UnixStream, connections: usize) {
    tracing::warn!("socket client refused: {connections} connections are open");
    let response = Response::error(
        None,
        ErrorCode::Unavailable,
        &format!("the daemon serves at most {connections} connections; try again later"),
    );
    // A fresh connection has room for one line; a full one gets nothing.
    let _ = stream.try_write(format!("{}\n", response.to_line()).as_bytes());
}

/// One read of [`read_line`].
enum Line {
    /// A complete line, without its line end.
    Text(String),
    /// More than the limit came before the newline.
    TooLong,
    /// The client hung up.
    End,
}

/// Reads the next line into `buf`, never holding more than `limit` bytes
/// of it. Cancel-safe: the only await is `fill_buf`, and the bytes of an
/// unfinished line stay in `buf` for the next call.
async fn read_line(
    reader: &mut BufReader<OwnedReadHalf>,
    buf: &mut Vec<u8>,
    limit: usize,
) -> io::Result<Line> {
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            // A last line without its newline still counts, as in `lines()`.
            return if buf.is_empty() {
                Ok(Line::End)
            } else {
                text(std::mem::take(buf))
            };
        }
        let (taken, newline) = match available.iter().position(|byte| *byte == b'\n') {
            Some(at) => (at, true),
            None => (available.len(), false),
        };
        if buf.len() + taken > limit {
            return Ok(Line::TooLong);
        }
        buf.extend_from_slice(&available[..taken]);
        reader.consume(taken + usize::from(newline));
        if newline {
            return text(std::mem::take(buf));
        }
    }
}

/// `bytes` as a line, without a trailing `\r`.
fn text(mut bytes: Vec<u8>) -> io::Result<Line> {
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    String::from_utf8(bytes)
        .map(Line::Text)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "a line is not UTF-8"))
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// The events one connection subscribed to.
struct Subscription {
    classes: Vec<EventClass>,
    /// What this subscriber last heard the pad shows.
    view: View,
    events: broadcast::Receiver<Event>,
}

impl Subscription {
    /// The wire events of `event` for this subscriber.
    fn wire(&mut self, event: Event) -> Vec<Wire> {
        let events = match event {
            Event::Connected { keys, geometry } => vec![Wire::Device(DeviceStatus {
                connected: true,
                keys: Some(keys),
                geometry: Some(geometry),
                error: None,
            })],
            Event::Disconnected { error } => vec![Wire::Device(DeviceStatus {
                connected: false,
                keys: None,
                geometry: None,
                error: Some(error),
            })],
            Event::Painted { .. } => Vec::new(),
            Event::Key { key, edge } => vec![Wire::Key {
                key: Some(key),
                control: None,
                edge,
            }],
            Event::Button { button, edge } => vec![Wire::Key {
                key: None,
                control: Some(Control::Button(button)),
                edge,
            }],
            Event::Dialpad(status) => vec![Wire::Dialpad(status)],
            Event::Dial {
                roller,
                delta,
                actions,
            } => vec![Wire::Dial {
                roller,
                delta,
                actions,
            }],
            Event::Context(context) => vec![Wire::Context(context)],
            Event::Trace(entry) => vec![Wire::Trace { entry }],
            Event::Switched(view) => {
                let events = Wire::switched(&self.view, &view);
                self.view = view;
                events
            }
            Event::Reloaded => vec![Wire::Config(ConfigStatus {
                reloaded: true,
                error: None,
            })],
            Event::ReloadFailed { error } => vec![Wire::Config(ConfigStatus {
                reloaded: false,
                error: Some(error),
            })],
        };
        events
            .into_iter()
            .filter(|event| self.classes.contains(&event.class()))
            .collect()
    }
}

/// The next daemon event of `subscription`, or never without one.
async fn next(
    subscription: &mut Option<Subscription>,
) -> Result<Event, broadcast::error::RecvError> {
    match subscription {
        Some(subscription) => subscription.events.recv().await,
        None => std::future::pending().await,
    }
}

async fn send(out: &mut OwnedWriteHalf, line: &str) -> io::Result<()> {
    out.write_all(format!("{line}\n").as_bytes()).await
}

/// Serves one client until it hangs up, sends a line longer than
/// `line_bytes`, or a write fails.
async fn connection(
    stream: UnixStream,
    requests: mpsc::UnboundedSender<daemon::Request>,
    line_bytes: usize,
) {
    let (read, mut out) = stream.into_split();
    let mut reader = BufReader::new(read);
    let mut buf = Vec::new();
    let mut subscription: Option<Subscription> = None;
    loop {
        let written = tokio::select! {
            line = read_line(&mut reader, &mut buf, line_bytes) => match line {
                Ok(Line::Text(line)) => {
                    if line.trim().is_empty() {
                        Ok(())
                    } else {
                        answer(&line, &requests, &mut subscription, &mut out).await
                    }
                }
                Ok(Line::TooLong) => {
                    tracing::warn!("socket client sent a line over {line_bytes} bytes; closed");
                    let response = Response::error(
                        None,
                        ErrorCode::InvalidRequest,
                        &format!("the line is longer than {line_bytes} bytes; closing"),
                    );
                    let _ = send(&mut out, &response.to_line()).await;
                    break;
                }
                Ok(Line::End) => break,
                Err(error) => {
                    tracing::debug!("socket client dropped: {error}");
                    break;
                }
            },
            event = next(&mut subscription) => match event {
                Ok(event) => {
                    let mut result = Ok(());
                    if let Some(subscription) = subscription.as_mut() {
                        for wire in subscription.wire(event) {
                            result = send(&mut out, &wire.to_line()).await;
                            if result.is_err() {
                                break;
                            }
                        }
                    }
                    result
                }
                Err(broadcast::error::RecvError::Lagged(missed)) => {
                    tracing::warn!("a socket subscriber missed {missed} events");
                    Ok(())
                }
                Err(broadcast::error::RecvError::Closed) => {
                    subscription = None;
                    Ok(())
                }
            },
        };
        if written.is_err() {
            break;
        }
    }
}

/// Answers the request `line`; a `subscribe` replaces the connection's
/// subscription and sends its backlog after the response.
async fn answer(
    line: &str,
    requests: &mpsc::UnboundedSender<daemon::Request>,
    subscription: &mut Option<Subscription>,
    out: &mut OwnedWriteHalf,
) -> io::Result<()> {
    let request = match Request::parse(line) {
        Ok(request) => request,
        Err(error) => {
            let response = Response::error(error.id, error.code, &error.message);
            return send(out, &response.to_line()).await;
        }
    };
    let id = request.id;
    let classes = match &request.command {
        keyvo_core::socket::Command::Subscribe { events, .. } => events.clone(),
        _ => Vec::new(),
    };
    let (reply, answered) = oneshot::channel();
    let asked = requests.send(daemon::Request {
        command: request.command,
        reply,
    });
    let Ok(answer) = (match asked {
        Ok(()) => answered.await.map_err(|_| ()),
        Err(_) => Err(()),
    }) else {
        let response = Response::error(id, ErrorCode::Unavailable, "the daemon is shutting down");
        return send(out, &response.to_line()).await;
    };
    let mut backlog: Vec<TraceRecord> = Vec::new();
    let response = match answer {
        Answer::Status(status) => Response::ok(id, Reply::Status(status)),
        Answer::Subscribed {
            view,
            backlog: entries,
            events,
        } => {
            if classes.contains(&EventClass::Trace) {
                backlog = entries;
            }
            *subscription = Some(Subscription {
                classes: classes.clone(),
                view: view.clone(),
                events,
            });
            Response::ok(
                id,
                Reply::Subscribed(Subscribed {
                    events: classes,
                    view,
                }),
            )
        }
        Answer::Pressed(entry) => Response::ok(id, Reply::Pressed(Pressed { entry, runs: None })),
        Answer::Turned { entry, runs } => Response::ok(
            id,
            Reply::Pressed(Pressed {
                entry,
                runs: Some(runs),
            }),
        ),
        Answer::Invalid(message) => Response::error(id, ErrorCode::InvalidRequest, &message),
        Answer::Recipes(recipes) => Response::ok(id, Reply::Recipes(recipes)),
        Answer::Reloaded(Ok(())) | Answer::Noted => Response::ok(id, Reply::Done {}),
        Answer::Reloaded(Err(error)) => Response::error(id, ErrorCode::ReloadFailed, &error),
    };
    send(out, &response.to_line()).await?;
    for entry in backlog {
        send(out, &Wire::Trace { entry }.to_line()).await?;
    }
    Ok(())
}
