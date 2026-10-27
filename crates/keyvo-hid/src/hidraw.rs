//! The Linux [`HidTransport`] on a hidraw node (spec §5.1). One `read` is
//! one inbound report and one `write` one outbound report, report id first;
//! the read timeout is a `poll`. The node is opened read-write and never
//! grabbed exclusively. `POLLHUP`, `EIO` and `ENODEV` mean the device is
//! gone.

use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use keyvo_core::hid::{HidTransport, TransportError};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::io::Errno;

/// Bytes one read takes: more than the largest report the Keypad declares
/// (the 4095-byte bulk report), so a report is never cut.
const READ_BUFFER: usize = 4096;

/// A hidraw node, or any file descriptor that moves whole reports.
#[derive(Debug)]
pub struct HidrawTransport {
    file: File,
}

impl HidrawTransport {
    /// Opens `path` (`/dev/hidraw2`) for reading and writing.
    ///
    /// # Errors
    ///
    /// [`TransportError::Io`] naming the path, for a missing node or missing
    /// permissions (the udev rule).
    pub fn open(path: &Path) -> Result<Self, TransportError> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map(Self::from_file)
            .map_err(|error| TransportError::Io(format!("{}: {error}", path.display())))
    }

    /// A transport on an open file, such as one end of a socket pair in
    /// tests.
    #[must_use]
    pub const fn from_file(file: File) -> Self {
        Self { file }
    }
}

impl HidTransport for HidrawTransport {
    fn write(&mut self, report: &[u8]) -> Result<(), TransportError> {
        loop {
            match self.file.write(report) {
                Ok(written) if written == report.len() => return Ok(()),
                Ok(written) => {
                    return Err(TransportError::Io(format!(
                        "short write: {written} of {} bytes",
                        report.len()
                    )));
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) => return Err(transport_error(&error)),
            }
        }
    }

    fn read(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, TransportError> {
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let wait = Timespec::try_from(remaining)
                .map_err(|_| TransportError::Io(format!("read timeout {timeout:?} too long")))?;
            let mut fds = [PollFd::new(&self.file, PollFlags::IN)];
            match poll(&mut fds, Some(&wait)) {
                Ok(0) => return Ok(None),
                Ok(_) => {}
                Err(Errno::INTR) => continue,
                Err(errno) => return Err(transport_error(&errno.into())),
            }
            let events = fds[0].revents();
            // Reports still queued are read before a hang-up is reported.
            if !events.contains(PollFlags::IN) {
                if events.intersects(PollFlags::HUP | PollFlags::ERR | PollFlags::NVAL) {
                    return Err(TransportError::Disconnected);
                }
                continue;
            }
            let mut buffer = vec![0; READ_BUFFER];
            match self.file.read(&mut buffer) {
                Ok(0) => return Err(TransportError::Disconnected),
                Ok(len) => {
                    buffer.truncate(len);
                    return Ok(Some(buffer));
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) => return Err(transport_error(&error)),
            }
        }
    }
}

/// `EIO`, `ENODEV` (hidraw after an unplug) and `EPIPE` are a disconnect;
/// anything else an I/O error with the OS message.
fn transport_error(error: &std::io::Error) -> TransportError {
    let gone = [Errno::IO, Errno::NODEV, Errno::PIPE]
        .iter()
        .any(|errno| error.raw_os_error() == Some(errno.raw_os_error()));
    if gone {
        TransportError::Disconnected
    } else {
        TransportError::Io(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::OwnedFd;
    use std::os::unix::net::{UnixDatagram, UnixStream};

    const SHORT: Duration = Duration::from_millis(20);

    /// A transport on one end of a datagram pair, which keeps report
    /// boundaries the way hidraw does, and the device's end.
    fn pair() -> (HidrawTransport, UnixDatagram) {
        let (host, device) = UnixDatagram::pair().unwrap();
        (
            HidrawTransport::from_file(File::from(OwnedFd::from(host))),
            device,
        )
    }

    #[test]
    fn a_read_returns_one_whole_report() {
        let (mut transport, device) = pair();
        device.send(&[0x11, 0xff, 0x00, 0x1e, 0x04, 0x05]).unwrap();
        let bulk = vec![0x14; 4095];
        device.send(&bulk).unwrap();
        assert_eq!(
            transport.read(SHORT).unwrap(),
            Some(vec![0x11, 0xff, 0x00, 0x1e, 0x04, 0x05])
        );
        assert_eq!(transport.read(SHORT).unwrap(), Some(bulk));
    }

    #[test]
    fn a_read_without_a_report_times_out() {
        let (mut transport, _device) = pair();
        let start = Instant::now();
        assert_eq!(transport.read(SHORT).unwrap(), None);
        assert!(start.elapsed() >= SHORT);
        assert_eq!(transport.read(Duration::ZERO).unwrap(), None);
    }

    #[test]
    fn a_write_sends_one_whole_report() {
        let (mut transport, device) = pair();
        let report = [0x13, 0xff, 0x00, 0x0e, 0xe0, 0x19, 0xa1];
        transport.write(&report).unwrap();
        let mut buffer = [0; 64];
        let len = device.recv(&mut buffer).unwrap();
        assert_eq!(&buffer[..len], &report);
    }

    #[test]
    fn a_closed_peer_is_a_disconnect_after_the_queued_reports() {
        let (host, device) = UnixStream::pair().unwrap();
        let mut transport = HidrawTransport::from_file(File::from(OwnedFd::from(host)));
        (&device).write_all(&[0x13, 0xff]).unwrap();
        drop(device);
        assert_eq!(transport.read(SHORT).unwrap(), Some(vec![0x13, 0xff]));
        assert_eq!(transport.read(SHORT), Err(TransportError::Disconnected));
        assert_eq!(
            transport.write(&[0x11, 0xff]),
            Err(TransportError::Disconnected)
        );
    }

    #[test]
    fn opening_a_missing_node_names_it() {
        let error = HidrawTransport::open(Path::new("/nonexistent/hidraw99")).unwrap_err();
        match error {
            TransportError::Io(message) => assert!(message.starts_with("/nonexistent/hidraw99: ")),
            other => panic!("expected an i/o error, got {other:?}"),
        }
    }

    #[test]
    fn only_device_gone_errors_are_a_disconnect() {
        for errno in [Errno::IO, Errno::NODEV, Errno::PIPE] {
            assert_eq!(transport_error(&errno.into()), TransportError::Disconnected);
        }
        assert!(matches!(
            transport_error(&Errno::ACCESS.into()),
            TransportError::Io(_)
        ));
    }
}
