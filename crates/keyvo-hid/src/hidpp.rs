//! Request/reply over a [`HidTransport`] on both HID++ planes.
//!
//! A reply matches its request on report id, device index, feature index and
//! the `(function << 4) | swid` byte (spec §5.1), never on "the next inbound
//! report". Reports that answer nothing are kept for the input path.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use keyvo_core::hid::{HidTransport, TransportError, to_hex};

use crate::frame::{Frame, FrameError, ReportId, VLP_SEQ_LAST, VLP_SEQ_SINGLE};

/// HID++ device index of a device attached directly (USB), not via a receiver.
pub const DEVICE_DIRECT: u8 = 0xff;

/// Feature index byte that marks an HID++ 2.0 error reply.
const ERROR_INDEX: u8 = 0xff;

/// The two protocol planes of the Keypad (spec §5.2), each with its own root
/// and its own feature index space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Plane {
    /// Classic HID++ 2.0 on long reports (`0x11`).
    Classic,
    /// The very long packet plane on the control report (`0x13`).
    Vlp,
}

impl Plane {
    /// The report id requests on this plane use.
    #[must_use]
    pub const fn request_report(self) -> ReportId {
        match self {
            Self::Classic => ReportId::Long,
            Self::Vlp => ReportId::VlpControl,
        }
    }
}

/// The 4-bit software id the host puts in every request; `0` is reserved for
/// device notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoftwareId(u8);

impl SoftwareId {
    /// A software id in `1..=15`.
    ///
    /// # Errors
    ///
    /// Returns [`HidppError::InvalidSoftwareId`] outside that range.
    pub const fn new(value: u8) -> Result<Self, HidppError> {
        if value == 0 || value > 0x0f {
            Err(HidppError::InvalidSoftwareId(value))
        } else {
            Ok(Self(value))
        }
    }

    /// The raw nibble.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

/// HID++ 2.0 error codes (Logitech HID++ 2.0 draft specification).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// `0`
    NoError,
    /// `1`
    Unknown,
    /// `2`
    InvalidArgument,
    /// `3`
    OutOfRange,
    /// `4`
    HardwareError,
    /// `5`
    LogitechInternal,
    /// `6`
    InvalidFeatureIndex,
    /// `7`
    InvalidFunctionId,
    /// `8`
    Busy,
    /// `9`
    Unsupported,
    /// Any code the draft specification does not name.
    Other(u8),
}

impl From<u8> for ErrorCode {
    fn from(code: u8) -> Self {
        match code {
            0 => Self::NoError,
            1 => Self::Unknown,
            2 => Self::InvalidArgument,
            3 => Self::OutOfRange,
            4 => Self::HardwareError,
            5 => Self::LogitechInternal,
            6 => Self::InvalidFeatureIndex,
            7 => Self::InvalidFunctionId,
            8 => Self::Busy,
            9 => Self::Unsupported,
            other => Self::Other(other),
        }
    }
}

/// Errors of the HID++ request layer.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum HidppError {
    /// The transport failed.
    #[error(transparent)]
    Transport(#[from] TransportError),
    /// A request did not fit its report.
    #[error(transparent)]
    Frame(#[from] FrameError),
    /// A software id outside `1..=15`.
    #[error("software id 0x{0:02x} is outside 1..=15")]
    InvalidSoftwareId(u8),
    /// No matching reply within the reply timeout.
    #[error("no reply on {report} to feature index 0x{feature_index:02x} function {function}")]
    Timeout {
        /// Report id of the request.
        report: ReportId,
        /// Feature index of the request.
        feature_index: u8,
        /// Function id of the request.
        function: u8,
    },
    /// The device answered with an HID++ 2.0 error reply.
    #[error("device error {code:?} on feature index 0x{feature_index:02x} function {function}")]
    Device {
        /// Feature index of the request.
        feature_index: u8,
        /// Function id of the request.
        function: u8,
        /// The error code.
        code: ErrorCode,
    },
    /// A VLP error reply. Its layout past the classic header is not captured
    /// yet, so the report is kept whole (hex).
    #[error("vlp error reply: {raw}")]
    VlpErrorReply {
        /// The whole report as hex.
        raw: String,
    },
    /// The feature is not in the plane's feature table.
    #[error("feature 0x{feature:04x} is not present on the {plane:?} plane")]
    FeatureNotPresent {
        /// The plane that was asked.
        plane: Plane,
        /// The feature id.
        feature: u16,
    },
    /// A reply matched its request but its content is not what the
    /// function returns.
    #[error("malformed reply: {0}")]
    Malformed(String),
}

/// An HID++ endpoint: one device on one transport, both planes.
#[derive(Debug)]
pub struct Hidpp<T> {
    transport: T,
    device_index: u8,
    swid: SoftwareId,
    reply_timeout: Duration,
    unmatched: VecDeque<Vec<u8>>,
    /// Feature indices resolved so far, per plane and feature id.
    pub(crate) features: HashMap<(Plane, u16), u8>,
}

impl<T: HidTransport> Hidpp<T> {
    /// Talks to `device_index` on `transport`, tagging requests with `swid`
    /// and waiting at most `reply_timeout` for each reply.
    pub fn new(transport: T, device_index: u8, swid: SoftwareId, reply_timeout: Duration) -> Self {
        Self {
            transport,
            device_index,
            swid,
            reply_timeout,
            unmatched: VecDeque::new(),
            features: HashMap::new(),
        }
    }

    /// Sends one request and returns the reply that matches it. VLP requests
    /// go out as a single packet (sequence byte `e0`); a VLP reply over
    /// several packets comes back as one frame whose parameters are the
    /// packets' parameters joined, up to the packet with [`VLP_SEQ_LAST`].
    ///
    /// # Errors
    ///
    /// [`HidppError::Device`] or [`HidppError::VlpErrorReply`] for an error
    /// reply, [`HidppError::Timeout`] when nothing matches in time, and the
    /// transport and framing errors.
    pub fn request(
        &mut self,
        plane: Plane,
        feature_index: u8,
        function: u8,
        params: &[u8],
    ) -> Result<Frame, HidppError> {
        let report = plane.request_report();
        let mut request = Frame::new(
            report,
            self.device_index,
            feature_index,
            function,
            self.swid.get(),
            params,
        );
        if plane == Plane::Vlp {
            request = request.with_seq(VLP_SEQ_SINGLE);
        }
        self.transport.write(&request.encode()?)?;

        let deadline = Instant::now() + self.reply_timeout;
        // The packets of a VLP reply read so far, joined; the deadline covers
        // the whole reply.
        let mut partial: Option<Frame> = None;
        loop {
            // Checked before every read: a device that streams events must
            // not keep the loop alive past the deadline.
            let remaining = deadline.saturating_duration_since(Instant::now());
            let raw = if remaining.is_zero() {
                None
            } else {
                self.transport.read(remaining)?
            };
            let Some(raw) = raw else {
                return Err(HidppError::Timeout {
                    report,
                    feature_index,
                    function,
                });
            };
            match self.answer(&request, &raw) {
                Some(Ok(frame)) => {
                    let last = frame.seq.is_none_or(|seq| seq & VLP_SEQ_LAST != 0);
                    let joined = match partial.take() {
                        Some(mut head) => {
                            head.params.extend(frame.params);
                            head
                        }
                        None => frame,
                    };
                    if last {
                        return Ok(joined);
                    }
                    partial = Some(joined);
                }
                Some(Err(error)) => return Err(error),
                None => self.unmatched.push_back(raw),
            }
        }
    }

    /// The next inbound report read while waiting for a reply that answered
    /// no request (device events, other software's traffic).
    pub fn take_unmatched(&mut self) -> Option<Vec<u8>> {
        self.unmatched.pop_front()
    }

    /// The underlying transport.
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// `Some` when `raw` answers `request`: the reply, or the error reply.
    fn answer(&self, request: &Frame, raw: &[u8]) -> Option<Result<Frame, HidppError>> {
        let frame = Frame::parse(raw).ok()?;
        if frame.report != request.report || frame.device_index != self.device_index {
            return None;
        }
        let tag = request.function_swid();
        if frame.feature_index == request.feature_index && frame.function_swid() == tag {
            return Some(Ok(frame));
        }
        // An error reply moves the request's index and tag one byte right,
        // after the 0xff marker: `<id> <dev> ff <index> <fn|swid> <code>`.
        let is_error = frame.feature_index == ERROR_INDEX
            && raw.get(3) == Some(&request.feature_index)
            && raw.get(4) == Some(&tag);
        if !is_error {
            return None;
        }
        Some(Err(match request.report {
            ReportId::Long => HidppError::Device {
                feature_index: request.feature_index,
                function: request.function,
                code: ErrorCode::from(raw.get(5).copied().unwrap_or_default()),
            },
            _ => HidppError::VlpErrorReply { raw: to_hex(raw) },
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keyvo_core::hid::FakeHidTransport;

    const TIMEOUT: Duration = Duration::from_secs(1);
    const SWID: u8 = 0xe;

    fn hidpp(script: &str) -> Hidpp<FakeHidTransport> {
        Hidpp::new(
            FakeHidTransport::from_script(script).unwrap(),
            DEVICE_DIRECT,
            SoftwareId::new(SWID).unwrap(),
            TIMEOUT,
        )
    }

    fn long(bytes: &str) -> String {
        pad(bytes, 20)
    }

    fn vlp(bytes: &str) -> String {
        pad(bytes, 32)
    }

    fn pad(bytes: &str, len: usize) -> String {
        let mut out: Vec<&str> = bytes.split_whitespace().collect();
        out.resize(len, "00");
        out.join(" ")
    }

    #[test]
    fn reply_matches_report_feature_function_and_swid() {
        let script = format!(
            "> {}\n< {}\n< {}\n< {}\n< {}\n",
            long("11 ff 04 0e"),
            vlp("13 ff 02 10 c6 a0"), // unsolicited VLP event
            long("11 ff 04 0d 09"),   // same request, other swid
            long("11 ff 04 1e 09"),   // other function
            long("11 ff 04 0e 01 f4 27 10"),
        );
        let mut hidpp = hidpp(&script);
        let reply = hidpp.request(Plane::Classic, 0x04, 0x0, &[]).unwrap();
        assert_eq!(&reply.params[..4], &[0x01, 0xf4, 0x27, 0x10]);

        let unmatched: Vec<_> = std::iter::from_fn(|| hidpp.take_unmatched()).collect();
        assert_eq!(unmatched.len(), 3);
        assert_eq!(unmatched[0][..4], [0x13, 0xff, 0x02, 0x10]);
    }

    #[test]
    fn vlp_request_carries_the_single_packet_sequence_byte() {
        let script = format!(
            "> {}\n< {}\n< {}\n",
            vlp("13 ff 00 0e e0 19 a1"),
            long("11 ff 00 0e 19 a1"), // classic report with the same bytes
            vlp("13 ff 00 0e c0 19 a1 02"),
        );
        let mut hidpp = hidpp(&script);
        let reply = hidpp.request(Plane::Vlp, 0x00, 0x0, &[0x19, 0xa1]).unwrap();
        assert_eq!(reply.seq, Some(0xc0));
        assert_eq!(&reply.params[..3], &[0x19, 0xa1, 0x02]);
    }

    #[test]
    fn a_vlp_reply_over_several_packets_is_joined_until_the_last_one() {
        let script = format!(
            "> {}\n< {}\n< {}\n< {}\n< {}\n",
            vlp("13 ff 02 1e e0 01"),
            vlp("13 ff 02 1e 80 01 02"), // first of several
            vlp("13 ff 02 10 c6 a0"),    // unsolicited event in between
            vlp("13 ff 02 1e 01 03"),    // continuation 1
            vlp("13 ff 02 1e 42 04"),    // last, index 2
        );
        let mut hidpp = hidpp(&script);
        let reply = hidpp.request(Plane::Vlp, 0x02, 0x1, &[0x01]).unwrap();
        let packet = ReportId::VlpControl.size() - ReportId::VlpControl.header_len();
        assert_eq!(reply.params.len(), 3 * packet);
        assert_eq!(&reply.params[..2], &[0x01, 0x02]);
        assert_eq!(reply.params[packet], 0x03);
        assert_eq!(reply.params[2 * packet], 0x04);
        assert_eq!(hidpp.take_unmatched().map(|r| r[3]), Some(0x10));
        assert!(hidpp.transport_mut().is_done());
    }

    #[test]
    fn a_vlp_reply_missing_its_last_packet_times_out() {
        let script = format!(
            "> {}\n< {}\n",
            vlp("13 ff 02 1e e0 01"),
            vlp("13 ff 02 1e 80 01 02"),
        );
        let mut hidpp = hidpp(&script);
        assert!(matches!(
            hidpp.request(Plane::Vlp, 0x02, 0x1, &[0x01]),
            Err(HidppError::Timeout { .. })
        ));
    }

    #[test]
    fn classic_error_reply_becomes_a_device_error() {
        let script = format!(
            "> {}\n< {}\n< {}\n",
            long("11 ff 04 1e 0b b8"),
            long("11 ff ff 04 0d 02"), // error for another swid
            long("11 ff ff 04 1e 02"),
        );
        let mut hidpp = hidpp(&script);
        assert_eq!(
            hidpp.request(Plane::Classic, 0x04, 0x1, &[0x0b, 0xb8]),
            Err(HidppError::Device {
                feature_index: 0x04,
                function: 0x1,
                code: ErrorCode::InvalidArgument,
            })
        );
    }

    #[test]
    fn vlp_error_reply_is_reported_raw() {
        let script = format!(
            "> {}\n< {}\n",
            vlp("13 ff 02 9e e0"),
            vlp("13 ff ff 02 9e 07"),
        );
        let mut hidpp = hidpp(&script);
        match hidpp.request(Plane::Vlp, 0x02, 0x9, &[]) {
            Err(HidppError::VlpErrorReply { raw }) => {
                assert!(raw.starts_with("13 ff ff 02 9e 07"));
            }
            other => panic!("expected a VLP error reply, got {other:?}"),
        }
    }

    #[test]
    fn no_reply_is_a_timeout() {
        let mut hidpp = hidpp(&format!("> {}\n", long("11 ff 04 0e")));
        assert_eq!(
            hidpp.request(Plane::Classic, 0x04, 0x0, &[]),
            Err(HidppError::Timeout {
                report: ReportId::Long,
                feature_index: 0x04,
                function: 0x0,
            })
        );
    }

    #[test]
    fn a_passed_deadline_stops_reading_even_while_reports_keep_coming() {
        let script = format!(
            "> {}\n< {}\n< {}\n",
            long("11 ff 04 0e"),
            vlp("13 ff 02 10 c6 a0"),
            long("11 ff 04 0e 01 f4 27 10"),
        );
        let mut hidpp = Hidpp::new(
            FakeHidTransport::from_script(&script).unwrap(),
            DEVICE_DIRECT,
            SoftwareId::new(SWID).unwrap(),
            Duration::ZERO,
        );
        assert!(matches!(
            hidpp.request(Plane::Classic, 0x04, 0x0, &[]),
            Err(HidppError::Timeout { .. })
        ));
    }

    #[test]
    fn error_codes_follow_the_hidpp_2_0_table() {
        let names: Vec<_> = (0..=10).map(ErrorCode::from).collect();
        assert_eq!(
            names,
            [
                ErrorCode::NoError,
                ErrorCode::Unknown,
                ErrorCode::InvalidArgument,
                ErrorCode::OutOfRange,
                ErrorCode::HardwareError,
                ErrorCode::LogitechInternal,
                ErrorCode::InvalidFeatureIndex,
                ErrorCode::InvalidFunctionId,
                ErrorCode::Busy,
                ErrorCode::Unsupported,
                ErrorCode::Other(10),
            ]
        );
    }

    #[test]
    fn software_id_zero_is_reserved_for_notifications() {
        assert_eq!(SoftwareId::new(0), Err(HidppError::InvalidSoftwareId(0)));
        assert_eq!(
            SoftwareId::new(0x10),
            Err(HidppError::InvalidSoftwareId(0x10))
        );
        assert_eq!(SoftwareId::new(0xd).unwrap().get(), 0xd);
    }
}
