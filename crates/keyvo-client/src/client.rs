//! One connection to the daemon.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

use keyvo_core::press::PressKind;
use keyvo_core::profile::KeySlot;
use keyvo_core::socket::{
    Command, ErrorBody, Event, EventClass, ProtocolError, Reply, Request, ServerMessage,
};
use keyvo_core::state::View;
use keyvo_core::trace::TraceRecord;
use serde_json::Value;

/// What went wrong on the connection.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// Nobody listens on the socket.
    #[error("cannot reach the keyvo daemon at {}: {source}", path.display())]
    Connect {
        /// The socket.
        path: PathBuf,
        /// Why.
        source: std::io::Error,
    },
    /// Reading or writing the stream failed.
    #[error("socket: {0}")]
    Io(#[from] std::io::Error),
    /// The daemon hung up before it answered.
    #[error("the daemon closed the connection")]
    Closed,
    /// A line from the daemon is not a v1 message.
    #[error("the daemon sent a line this client does not read: {0}")]
    Protocol(#[from] ProtocolError),
    /// The daemon refused the request.
    #[error("the daemon refused the request: {}", .0.message)]
    Refused(ErrorBody),
    /// The daemon answered the named command with another command's reply.
    #[error("the daemon answered `{0}` with another reply")]
    Unexpected(&'static str),
}

impl Client<UnixStream> {
    /// A client on the daemon's socket at `path`.
    ///
    /// # Errors
    ///
    /// When nobody listens there.
    pub fn connect(path: &Path) -> Result<Self, ClientError> {
        UnixStream::connect(path)
            .map(Self::new)
            .map_err(|source| ClientError::Connect {
                path: path.to_owned(),
                source,
            })
    }
}

/// A connection that sends one request at a time.
#[derive(Debug)]
pub struct Client<S> {
    stream: BufReader<S>,
    next_id: u64,
    /// Events that came in while a request waited for its answer.
    events: VecDeque<Event>,
}

impl<S: Read + Write> Client<S> {
    /// A client on `stream`.
    pub fn new(stream: S) -> Self {
        Self {
            stream: BufReader::new(stream),
            next_id: 1,
            events: VecDeque::new(),
        }
    }

    /// The stream back.
    pub fn into_inner(self) -> S {
        self.stream.into_inner()
    }

    /// Sends `command` and waits for its answer.
    ///
    /// # Errors
    ///
    /// When the daemon refuses it, hangs up first or sends a line that is
    /// not a v1 message, or the stream fails.
    pub fn request(&mut self, command: Command) -> Result<Reply, ClientError> {
        let id = Value::from(self.next_id);
        self.next_id += 1;
        let request = Request {
            id: Some(id.clone()),
            command,
        };
        let stream = self.stream.get_mut();
        stream.write_all(format!("{}\n", request.to_line()).as_bytes())?;
        stream.flush()?;
        loop {
            match self.read()?.ok_or(ClientError::Closed)? {
                ServerMessage::Event(event) => self.events.push_back(event),
                // A response without an id answers a line the daemon could
                // not read, and only one request is ever open.
                ServerMessage::Response(response)
                    if response.id.as_ref().is_none_or(|got| *got == id) =>
                {
                    return response.result.map_err(ClientError::Refused);
                }
                ServerMessage::Response(_) => {}
            }
        }
    }

    /// A dry-run press of `key` (the simulator's path): the daemon resolves
    /// and traces it, executes nothing, and returns the trace entry.
    ///
    /// # Errors
    ///
    /// As [`Client::request`], and when the daemon answers with another
    /// reply.
    pub fn dry_run(&mut self, key: KeySlot, kind: PressKind) -> Result<TraceRecord, ClientError> {
        match self.request(Command::Press {
            key,
            kind,
            dry_run: true,
        })? {
            Reply::Pressed(pressed) => Ok(pressed.entry),
            _ => Err(ClientError::Unexpected("press")),
        }
    }

    /// Subscribes to every event class, after the newest `replay` trace
    /// entries; the view the pad shows at that moment.
    ///
    /// # Errors
    ///
    /// As [`Client::request`], and when the daemon answers with another
    /// reply.
    pub fn subscribe(&mut self, replay: usize) -> Result<View, ClientError> {
        match self.request(Command::Subscribe {
            events: EventClass::ALL.to_vec(),
            replay,
        })? {
            Reply::Subscribed(subscribed) => Ok(subscribed.view),
            _ => Err(ClientError::Unexpected("subscribe")),
        }
    }

    /// The next event of a subscription, the ones that came in while a
    /// request waited first; `None` once the daemon hangs up.
    ///
    /// # Errors
    ///
    /// When the daemon sends a line that is not a v1 message, or the stream
    /// fails.
    pub fn next_event(&mut self) -> Result<Option<Event>, ClientError> {
        if let Some(event) = self.events.pop_front() {
            return Ok(Some(event));
        }
        while let Some(message) = self.read()? {
            if let ServerMessage::Event(event) = message {
                return Ok(Some(event));
            }
        }
        Ok(None)
    }

    /// The next line from the daemon; `None` when it hung up.
    fn read(&mut self) -> Result<Option<ServerMessage>, ClientError> {
        let mut line = String::new();
        loop {
            line.clear();
            if self.stream.read_line(&mut line)? == 0 {
                return Ok(None);
            }
            if !line.trim().is_empty() {
                return Ok(Some(ServerMessage::parse(&line)?));
            }
        }
    }
}
