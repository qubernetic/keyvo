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

/// Longest line the client reads from the daemon, in bytes without the
/// newline (`docs/socket-api.md`, Framing). The daemon's own answers and
/// events are far shorter; a longer line ends the read with
/// [`ClientError::TooLong`] instead of growing without bound.
pub const LINE_LIMIT: usize = 1024 * 1024;

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
    /// The daemon sent a line longer than [`LINE_LIMIT`] bytes.
    #[error("the daemon sent a line longer than {0} bytes")]
    TooLong(usize),
    /// The socket is not this user's daemon's: its directory is open to
    /// other users, or another user listens on it.
    #[error("refusing the keyvo socket at {}: {reason}", path.display())]
    Untrusted {
        /// The socket.
        path: PathBuf,
        /// Why.
        reason: String,
    },
}

/// Opens the daemon's socket at `path` for this user. Nothing is sent
/// before two checks pass: the socket's directory is private to this user,
/// as the daemon creates it, and the process listening on it runs as this
/// user (`SO_PEERCRED`). `keyvo ctl` and the app open the socket here.
///
/// # Errors
///
/// [`ClientError::Connect`] when nobody listens there,
/// [`ClientError::Untrusted`] when a check fails, [`ClientError::Io`] when
/// a check cannot be made.
pub fn open(path: &Path) -> Result<UnixStream, ClientError> {
    let stream = UnixStream::connect(path).map_err(|source| ClientError::Connect {
        path: path.to_owned(),
        source,
    })?;
    let untrusted = |reason| ClientError::Untrusted {
        path: path.to_owned(),
        reason,
    };
    let uid = rustix::process::getuid().as_raw();
    let dir = match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    };
    check_dir(dir, &std::fs::symlink_metadata(dir)?, uid).map_err(untrusted)?;
    check_peer(peer_uid(&stream)?, uid).map_err(untrusted)?;
    Ok(stream)
}

/// The uid of the process at the other end of `stream` (`SO_PEERCRED`).
fn peer_uid(stream: &UnixStream) -> std::io::Result<u32> {
    peer_cred(stream).map(uid_of)
}

/// The raw `struct ucred` of the peer: pid, uid, gid. The pid is the peer
/// as seen from this process's pid namespace, so a sandboxed client (a
/// Flatpak app) sees the host daemon as pid 0. rustix's `UCred` cannot hold
/// a zero pid, hence the plain `getsockopt` into `libc::ucred`.
fn peer_cred(stream: &UnixStream) -> std::io::Result<libc::ucred> {
    use std::os::fd::AsRawFd;

    let mut cred = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = libc::socklen_t::try_from(size_of::<libc::ucred>())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    // SAFETY: the fd is open for the borrow of `stream`; `cred` is a
    // writable `struct ucred` of `len` bytes, and the kernel writes at most
    // `len` bytes into it.
    let ret = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&raw mut cred).cast(),
            &raw mut len,
        )
    };
    if ret != 0 {
        return Err(std::io::Error::last_os_error());
    }
    full_ucred(len)?;
    Ok(cred)
}

/// `Err` unless the kernel filled `len` bytes, a whole `struct ucred`.
fn full_ucred(len: libc::socklen_t) -> std::io::Result<()> {
    if usize::try_from(len).ok() == Some(size_of::<libc::ucred>()) {
        Ok(())
    } else {
        Err(std::io::Error::other("SO_PEERCRED returned a short ucred"))
    }
}

/// The uid of a raw `ucred`; only the uid decides trust, neither the pid
/// (namespace-dependent) nor the gid is examined.
fn uid_of(cred: libc::ucred) -> u32 {
    cred.uid
}

/// `Err` with the reason unless `dir` (its `meta` not following a symlink)
/// is a directory owned by `uid` that nobody else may enter.
fn check_dir(dir: &Path, meta: &std::fs::Metadata, uid: u32) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;

    if !meta.is_dir() || meta.uid() != uid || meta.mode() & 0o077 != 0 {
        return Err(format!(
            "its directory {} is not private to this user (owner {}, mode {:o})",
            dir.display(),
            meta.uid(),
            meta.mode() & 0o7777
        ));
    }
    Ok(())
}

/// `Err` with the reason unless the daemon's uid `peer` is `uid`.
fn check_peer(peer: u32, uid: u32) -> Result<(), String> {
    if peer == uid {
        Ok(())
    } else {
        Err(format!(
            "the process listening on it runs as uid {peer}, not as this user ({uid})"
        ))
    }
}

impl Client<UnixStream> {
    /// A client on the daemon's socket at `path`, opened with [`open`].
    ///
    /// # Errors
    ///
    /// As [`open`].
    pub fn connect(path: &Path) -> Result<Self, ClientError> {
        open(path).map(Self::new)
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

    /// The next line from the daemon, at most [`LINE_LIMIT`] bytes of it
    /// held; `None` when it hung up.
    fn read(&mut self) -> Result<Option<ServerMessage>, ClientError> {
        let mut bytes = Vec::new();
        loop {
            bytes.clear();
            let cap = u64::try_from(LINE_LIMIT)
                .unwrap_or(u64::MAX)
                .saturating_add(1);
            if (&mut self.stream).take(cap).read_until(b'\n', &mut bytes)? == 0 {
                return Ok(None);
            }
            if bytes.last() != Some(&b'\n') && bytes.len() > LINE_LIMIT {
                return Err(ClientError::TooLong(LINE_LIMIT));
            }
            let line = std::str::from_utf8(&bytes).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "a line is not UTF-8")
            })?;
            if !line.trim().is_empty() {
                return Ok(Some(ServerMessage::parse(line)?));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    use super::*;

    #[test]
    fn the_peer_must_be_this_user() {
        assert_eq!(check_peer(1000, 1000), Ok(()));
        for peer in [0, 1001, u32::MAX] {
            let reason = check_peer(peer, 1000).unwrap_err();
            assert!(reason.contains(&format!("uid {peer}")), "{reason}");
            assert!(reason.contains("not as this user (1000)"), "{reason}");
        }
    }

    #[test]
    fn peer_uid_reads_the_uid_of_the_other_end() {
        let (ours, _theirs) = UnixStream::pair().unwrap();
        assert_eq!(peer_uid(&ours).unwrap(), rustix::process::getuid().as_raw());
    }

    #[test]
    fn a_peer_seen_as_pid_0_is_trusted_by_its_uid() {
        // A sandboxed client sees the host daemon as pid 0 (Flatpak).
        let cred = |pid, uid, gid| libc::ucred { pid, uid, gid };
        assert_eq!(check_peer(uid_of(cred(0, 1000, 1000)), 1000), Ok(()));
        assert_eq!(check_peer(uid_of(cred(0, 1000, 4242)), 1000), Ok(()));
        let reason = check_peer(uid_of(cred(0, 1001, 1000)), 1000).unwrap_err();
        assert!(reason.contains("uid 1001"), "{reason}");
    }

    #[test]
    fn only_a_full_ucred_is_read() {
        assert!(full_ucred(12).is_ok());
        for len in [0, 8] {
            assert!(full_ucred(len).is_err(), "{len}");
        }
    }

    #[test]
    fn the_directory_must_be_this_users_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path();
        let meta = |mode| {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
            std::fs::symlink_metadata(path).unwrap()
        };
        let owner = meta(0o700).uid();
        assert_eq!(check_dir(path, &meta(0o700), owner), Ok(()));
        assert_eq!(check_dir(path, &meta(0o500), owner), Ok(()));
        for mode in [0o755, 0o750, 0o705, 0o701, 0o710, 0o777] {
            let reason = check_dir(path, &meta(mode), owner).unwrap_err();
            assert!(reason.contains(&format!("mode {mode:o}")), "{reason}");
        }
        let reason = check_dir(path, &meta(0o700), owner + 1).unwrap_err();
        assert!(reason.contains(&format!("owner {owner}")), "{reason}");

        let file = path.join("file");
        std::fs::write(&file, "").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        let meta = std::fs::symlink_metadata(&file).unwrap();
        assert!(check_dir(&file, &meta, owner).is_err());
    }
}
