//! `keyvo --dump`: a [`HidTransport`] that passes every report on and prints
//! it, one line each, so the raw-frame checks of `docs/hardware-notes.md`
//! need no external sniffer on the hidraw node (spec §5.10).

use std::io::Write;
use std::time::{Duration, Instant};

use keyvo_core::clock::Clock;
use keyvo_core::hid::{HidTransport, TransportError, to_hex};

use crate::frame::ReportId;

/// Wraps a transport and prints its traffic to `out`.
pub struct Dump<T, C, W> {
    inner: T,
    clock: C,
    start: Instant,
    out: W,
}

impl<T: HidTransport, C: Clock, W: Write> Dump<T, C, W> {
    /// Dumps the traffic of `inner` to `out`, timed by `clock` from now.
    pub fn new(inner: T, clock: C, out: W) -> Self {
        Self {
            start: clock.now(),
            inner,
            clock,
            out,
        }
    }

    /// The wrapped transport.
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Prints `<ms since start> <direction> <hex>`; a VLP bulk report
    /// (`0x14`, 4094 bytes a fragment) prints only its header and length.
    fn line(&mut self, direction: char, report: &[u8]) {
        let ms = self
            .clock
            .now()
            .saturating_duration_since(self.start)
            .as_millis();
        let text = if report.first() == Some(&ReportId::VlpBulk.byte()) {
            let header = &report[..report.len().min(ReportId::VlpBulk.header_len())];
            format!("{} .. ({} bytes)", to_hex(header), report.len())
        } else {
            to_hex(report)
        };
        // A dump that cannot be printed must not stop the command.
        let _ = writeln!(self.out, "{ms:>6} {direction} {text}");
    }
}

impl<T: HidTransport, C: Clock, W: Write> HidTransport for Dump<T, C, W> {
    /// Logs the report before the write, so a refused write shows too.
    fn write(&mut self, report: &[u8]) -> Result<(), TransportError> {
        self.line('>', report);
        self.inner.write(report)
    }

    /// Logs a report read; a timeout or an error prints nothing.
    fn read(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, TransportError> {
        let report = self.inner.read(timeout)?;
        if let Some(report) = &report {
            self.line('<', report);
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keyvo_core::clock::FakeClock;
    use keyvo_core::hid::FakeHidTransport;

    #[test]
    fn frames_in_both_directions_are_logged_in_order_and_a_bulk_write_by_its_header() {
        let ping = [0x11, 0xff, 0x00, 0x1e, 0x00, 0x00, 0x39];
        let reply = [0x11, 0xff, 0x00, 0x1e, 0x04, 0x05, 0x39];
        let mut bulk = vec![0x14, 0xff, 0x02, 0x5d, 0xc0, 0x12, 0x34];
        bulk.resize(4094, 0xaa);
        let fake = FakeHidTransport::new()
            .expect(&ping, &[&reply])
            .expect(&bulk, &[]);
        let clock = FakeClock::new();
        let mut log = Vec::new();
        {
            let mut dump = Dump::new(fake, &clock, &mut log);
            clock.advance(Duration::from_millis(3));
            dump.write(&ping).unwrap();
            clock.advance(Duration::from_millis(1));
            assert_eq!(dump.read(Duration::ZERO).unwrap(), Some(reply.to_vec()));
            // A timeout is not a frame.
            assert_eq!(dump.read(Duration::ZERO).unwrap(), None);
            clock.advance(Duration::from_millis(1250));
            dump.write(&bulk).unwrap();
            assert!(dump.inner_mut().is_done());
        }
        let lines = [
            "     3 > 11 ff 00 1e 00 00 39",
            "     4 < 11 ff 00 1e 04 05 39",
            "  1254 > 14 ff 02 5d c0 .. (4094 bytes)",
        ];
        assert_eq!(String::from_utf8(log).unwrap(), lines.join("\n") + "\n");
    }

    #[test]
    fn a_refused_write_is_logged_and_its_error_passes_through() {
        let mut log = Vec::new();
        let mut dump = Dump::new(FakeHidTransport::new(), FakeClock::new(), &mut log);
        assert!(dump.write(&[0x11, 0xff]).is_err());
        drop(dump);
        // The write was attempted, so it is logged; nothing else is.
        assert_eq!(String::from_utf8(log).unwrap(), "     0 > 11 ff\n");
    }
}
