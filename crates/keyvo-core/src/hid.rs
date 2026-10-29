//! The `HidTransport` platform trait (spec §4.1) and `FakeHidTransport`, the
//! scripted transport that replays captured request/response pairs in tests.

use std::time::Duration;

/// Errors of a [`HidTransport`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TransportError {
    /// The device is gone (`POLLHUP`, `EIO`, `ENODEV` on hidraw).
    #[error("device disconnected")]
    Disconnected,
    /// Any other I/O failure, with the OS message.
    #[error("hid i/o error: {0}")]
    Io(String),
    /// A [`FakeHidTransport`] received a write its script did not expect.
    #[error("unexpected write {got}, script expected {expected}")]
    UnexpectedWrite {
        /// Hex of the written report.
        got: String,
        /// Hex of the next scripted request, or `end of script`.
        expected: String,
    },
}

/// Whole HID reports in and out of one device node. Linux implements it on
/// hidraw, other platforms on hidapi; tests use [`FakeHidTransport`].
pub trait HidTransport {
    /// Writes one report; byte 0 is the report id.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the device rejects the write or is gone.
    fn write(&mut self, report: &[u8]) -> Result<(), TransportError>;

    /// Reads the next inbound report, waiting at most `timeout`. `Ok(None)`
    /// means the timeout passed without a report.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the device is gone or the read fails.
    fn read(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, TransportError>;
}

/// One scripted exchange: the report the host must write next and the
/// reports the device answers with, each after its delay.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Exchange {
    request: Vec<u8>,
    responses: Vec<(Duration, Vec<u8>)>,
}

/// A [`HidTransport`] that replays a script of request/response pairs.
///
/// Every write must equal the next scripted request byte for byte; its
/// responses then become readable in order. The `<` lines of a script before
/// its first `>` are readable before the first write, like unsolicited
/// device events. A read with nothing queued is a timeout.
///
/// A response can carry a delay counted from the write it answers: it is
/// readable only once that much time has passed on the fake's own clock,
/// which advances by the timeout of every read that waits. A slow device
/// costs a test no wall-clock time.
#[derive(Debug, Default)]
pub struct FakeHidTransport {
    script: std::collections::VecDeque<Exchange>,
    /// Queued reports and the time on `clock` from which each is readable.
    inbound: std::collections::VecDeque<(Duration, Vec<u8>)>,
    clock: Duration,
    written: Vec<Vec<u8>>,
}

impl FakeHidTransport {
    /// An empty script: every write is unexpected, every read times out.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends an exchange: `request` is the next expected write and
    /// `responses` are queued for reading once it arrives.
    #[must_use]
    pub fn expect(mut self, request: &[u8], responses: &[&[u8]]) -> Self {
        self.script.push_back(Exchange {
            request: request.to_vec(),
            responses: responses
                .iter()
                .map(|r| (Duration::ZERO, r.to_vec()))
                .collect(),
        });
        self
    }

    /// Parses a script in the fixture notation of `tests/protocol-fixtures/hid/`:
    /// `> <hex>` is a host write, `< <hex>` a device report, `#` starts a
    /// comment. `<` lines before the first `>` are unsolicited reports; the
    /// others answer the `>` line above them. `< +<ms> <hex>` delays the
    /// report by `<ms>` milliseconds after that write.
    ///
    /// # Errors
    ///
    /// Returns a [`ScriptError`] naming the 1-based line of the first bad line.
    pub fn from_script(text: &str) -> Result<Self, ScriptError> {
        let mut fake = Self::new();
        for (number, raw) in text.lines().enumerate() {
            let line = number + 1;
            let content = raw.split('#').next().unwrap_or_default().trim();
            let Some((direction, hex)) = content.split_at_checked(1) else {
                continue;
            };
            let (delay, hex) = parse_delay(hex).map_err(|message| ScriptError { line, message })?;
            let report = parse_hex(hex).map_err(|message| ScriptError { line, message })?;
            match direction {
                ">" => {
                    fake = fake.expect(&report, &[]);
                }
                "<" => match fake.script.back_mut() {
                    Some(exchange) => exchange.responses.push((delay, report)),
                    None => fake.inbound.push_back((delay, report)),
                },
                _ => {
                    return Err(ScriptError {
                        line,
                        message: "expected `>`, `<` or `#`".into(),
                    });
                }
            }
        }
        Ok(fake)
    }

    /// True when every scripted request was written and every queued report
    /// was read.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.script.is_empty() && self.inbound.is_empty()
    }

    /// Every report written so far, in order.
    #[must_use]
    pub fn written(&self) -> &[Vec<u8>] {
        &self.written
    }
}

impl HidTransport for FakeHidTransport {
    fn write(&mut self, report: &[u8]) -> Result<(), TransportError> {
        match self.script.front() {
            Some(next) if next.request == report => {
                if let Some(exchange) = self.script.pop_front() {
                    let now = self.clock;
                    self.inbound.extend(
                        exchange
                            .responses
                            .into_iter()
                            .map(|(delay, report)| (now.saturating_add(delay), report)),
                    );
                }
                self.written.push(report.to_vec());
                Ok(())
            }
            next => Err(TransportError::UnexpectedWrite {
                got: to_hex(report),
                expected: next.map_or_else(|| "end of script".to_owned(), |e| to_hex(&e.request)),
            }),
        }
    }

    fn read(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, TransportError> {
        let until = self.clock.saturating_add(timeout);
        match self.inbound.front() {
            Some((due, _)) if *due <= until => {
                self.clock = self.clock.max(*due);
                Ok(self.inbound.pop_front().map(|(_, report)| report))
            }
            _ => {
                self.clock = until;
                Ok(None)
            }
        }
    }
}

/// A malformed line in a [`FakeHidTransport`] script.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("script line {line}: {message}")]
pub struct ScriptError {
    /// 1-based line number.
    pub line: usize,
    /// What is wrong with the line.
    pub message: String,
}

/// The optional `+<ms>` delay in front of a report, and the rest.
fn parse_delay(text: &str) -> Result<(Duration, &str), String> {
    let text = text.trim_start();
    let Some(rest) = text.strip_prefix('+') else {
        return Ok((Duration::ZERO, text));
    };
    let (ms, hex) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let ms = ms.parse().map_err(|_| format!("invalid delay `+{ms}`"))?;
    Ok((Duration::from_millis(ms), hex))
}

fn parse_hex(text: &str) -> Result<Vec<u8>, String> {
    let report = text
        .split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).map_err(|_| format!("invalid hex byte `{byte}`")))
        .collect::<Result<Vec<_>, _>>()?;
    if report.is_empty() {
        return Err("empty report".into());
    }
    Ok(report)
}

/// Space-separated lowercase hex, the notation of the capture decodes.
#[must_use]
pub fn to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: Duration = Duration::ZERO;

    #[test]
    fn fake_replays_the_response_of_a_matching_request() {
        let mut fake = FakeHidTransport::new().expect(
            &[0x11, 0xff, 0x00, 0x1e],
            &[&[0x11, 0xff, 0x00, 0x1e, 0x04]],
        );
        fake.write(&[0x11, 0xff, 0x00, 0x1e]).unwrap();
        assert_eq!(
            fake.read(T).unwrap(),
            Some(vec![0x11, 0xff, 0x00, 0x1e, 0x04])
        );
        assert_eq!(fake.read(T).unwrap(), None);
        assert!(fake.is_done());
        assert_eq!(fake.written(), &[vec![0x11, 0xff, 0x00, 0x1e]]);
    }

    #[test]
    fn fake_rejects_a_write_the_script_does_not_expect() {
        let mut fake = FakeHidTransport::new().expect(&[0x11, 0x01], &[]);
        assert_eq!(
            fake.write(&[0x11, 0x02]),
            Err(TransportError::UnexpectedWrite {
                got: "11 02".into(),
                expected: "11 01".into()
            })
        );
        fake.write(&[0x11, 0x01]).unwrap();
        assert_eq!(
            fake.write(&[0x11, 0x01]),
            Err(TransportError::UnexpectedWrite {
                got: "11 01".into(),
                expected: "end of script".into()
            })
        );
    }

    #[test]
    fn script_text_pairs_requests_with_responses_and_leading_events() {
        let script = "\
# comment
< 13 ff 02 10 c6 a0

> 11 ff 00 1e  # trailing comment
< 11 ff 00 1e 04
< 11 ff 00 1e 05
";
        let mut fake = FakeHidTransport::from_script(script).unwrap();
        assert_eq!(
            fake.read(T).unwrap(),
            Some(vec![0x13, 0xff, 0x02, 0x10, 0xc6, 0xa0])
        );
        assert_eq!(fake.read(T).unwrap(), None);
        fake.write(&[0x11, 0xff, 0x00, 0x1e]).unwrap();
        assert_eq!(
            fake.read(T).unwrap(),
            Some(vec![0x11, 0xff, 0x00, 0x1e, 0x04])
        );
        assert_eq!(
            fake.read(T).unwrap(),
            Some(vec![0x11, 0xff, 0x00, 0x1e, 0x05])
        );
        assert!(fake.is_done());
    }

    #[test]
    fn a_delayed_report_is_readable_once_its_delay_has_passed() {
        let ms = Duration::from_millis;
        let script = "\
> 11 01
< +734 11 01 00
> 11 02
< +6 11 01 01
< +541 11 02 00
";
        let mut fake = FakeHidTransport::from_script(script).unwrap();
        fake.write(&[0x11, 0x01]).unwrap();
        // Not within 300 ms; a later read waits the rest.
        assert_eq!(fake.read(ms(300)).unwrap(), None);
        assert_eq!(fake.read(ms(434)).unwrap(), Some(vec![0x11, 0x01, 0x00]));
        // The delay counts from the write the report answers.
        fake.write(&[0x11, 0x02]).unwrap();
        assert_eq!(fake.read(T).unwrap(), None);
        assert_eq!(fake.read(ms(6)).unwrap(), Some(vec![0x11, 0x01, 0x01]));
        assert_eq!(fake.read(ms(1000)).unwrap(), Some(vec![0x11, 0x02, 0x00]));
        assert!(fake.is_done());
        assert_eq!(
            FakeHidTransport::from_script("< +x 11\n").unwrap_err(),
            ScriptError {
                line: 1,
                message: "invalid delay `+x`".into()
            }
        );
    }

    #[test]
    fn script_errors_name_the_line() {
        assert_eq!(
            FakeHidTransport::from_script("> 11 ff\n? 00\n").unwrap_err(),
            ScriptError {
                line: 2,
                message: "expected `>`, `<` or `#`".into()
            }
        );
        assert_eq!(
            FakeHidTransport::from_script("> 11 zz\n").unwrap_err(),
            ScriptError {
                line: 1,
                message: "invalid hex byte `zz`".into()
            }
        );
        assert_eq!(
            FakeHidTransport::from_script(">\n").unwrap_err(),
            ScriptError {
                line: 1,
                message: "empty report".into()
            }
        );
    }
}
