//! HID++ report framing on both planes of the Keypad (spec §5.2).
//!
//! Every report starts with `<report id> <device index> <feature index>
//! <(function << 4) | swid>`. VLP reports (`0x13`, `0x14`) carry a sequence
//! byte after that; the parameters follow and the report is zero-padded to
//! its fixed length.

use std::fmt;

/// Sequence byte of a host request that fits one VLP packet (first and last).
pub const VLP_SEQ_SINGLE: u8 = 0xe0;

/// The HID report ids keyvo speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReportId {
    /// `0x10`, 7 bytes: classic HID++ short report.
    Short,
    /// `0x11`, 20 bytes: classic HID++ long report.
    Long,
    /// `0x13`, 32 bytes: VLP control channel (requests, replies, ACKs, input).
    VlpControl,
    /// `0x14`, 4095 bytes: VLP bulk channel (LCD image fragments).
    VlpBulk,
}

impl ReportId {
    /// The report id byte.
    #[must_use]
    pub const fn byte(self) -> u8 {
        match self {
            Self::Short => 0x10,
            Self::Long => 0x11,
            Self::VlpControl => 0x13,
            Self::VlpBulk => 0x14,
        }
    }

    /// The report id for a byte, if keyvo knows it.
    #[must_use]
    pub const fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x10 => Some(Self::Short),
            0x11 => Some(Self::Long),
            0x13 => Some(Self::VlpControl),
            0x14 => Some(Self::VlpBulk),
            _ => None,
        }
    }

    /// Total report length in bytes, report id included.
    #[must_use]
    pub const fn size(self) -> usize {
        match self {
            Self::Short => 7,
            Self::Long => 20,
            Self::VlpControl => 32,
            Self::VlpBulk => 4095,
        }
    }

    /// True for the VLP reports, which carry a sequence byte.
    #[must_use]
    pub const fn is_vlp(self) -> bool {
        matches!(self, Self::VlpControl | Self::VlpBulk)
    }

    /// Length of the header before the parameters.
    #[must_use]
    pub const fn header_len(self) -> usize {
        if self.is_vlp() { 5 } else { 4 }
    }
}

impl fmt::Display for ReportId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:02x}", self.byte())
    }
}

/// Errors of [`Frame::parse`] and [`Frame::encode`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FrameError {
    /// A zero-length report.
    #[error("empty report")]
    Empty,
    /// A report id keyvo does not speak.
    #[error("unknown report id 0x{0:02x}")]
    UnknownReport(u8),
    /// The report is not exactly its fixed length.
    #[error("report {report} must be {expected} bytes, got {got}")]
    Length {
        /// Report id.
        report: ReportId,
        /// Fixed length of that report.
        expected: usize,
        /// Received length.
        got: usize,
    },
    /// The parameters do not fit the report.
    #[error("report {report} takes at most {max} parameter bytes, got {got}")]
    ParamsTooLong {
        /// Report id.
        report: ReportId,
        /// Parameter capacity of that report.
        max: usize,
        /// Requested parameter count.
        got: usize,
    },
    /// A function id or software id above `0xf`.
    #[error("{field} 0x{value:02x} does not fit in four bits")]
    NibbleOutOfRange {
        /// `function` or `swid`.
        field: &'static str,
        /// The offending value.
        value: u8,
    },
    /// A VLP frame without a sequence byte, or a classic frame with one.
    #[error("report {report} sequence byte presence does not match the report type")]
    SeqMismatch {
        /// Report id.
        report: ReportId,
    },
}

/// One HID++ report on either plane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Report id; selects the plane and the length.
    pub report: ReportId,
    /// HID++ device index; `0xff` for a device attached directly.
    pub device_index: u8,
    /// Feature index in the plane's own index space.
    pub feature_index: u8,
    /// Function id, high nibble of byte 3.
    pub function: u8,
    /// Software id, low nibble of byte 3; `0` marks device notifications.
    pub swid: u8,
    /// VLP sequence byte; `None` on the classic plane.
    pub seq: Option<u8>,
    /// Parameters. After [`Frame::parse`] this includes the zero padding.
    pub params: Vec<u8>,
}

impl Frame {
    /// A frame without a sequence byte; add one with [`Frame::with_seq`] for VLP.
    #[must_use]
    pub fn new(
        report: ReportId,
        device_index: u8,
        feature_index: u8,
        function: u8,
        swid: u8,
        params: &[u8],
    ) -> Self {
        Self {
            report,
            device_index,
            feature_index,
            function,
            swid,
            seq: None,
            params: params.to_vec(),
        }
    }

    /// Sets the VLP sequence byte.
    #[must_use]
    pub fn with_seq(mut self, seq: u8) -> Self {
        self.seq = Some(seq);
        self
    }

    /// Byte 3: `(function << 4) | swid`.
    #[must_use]
    pub const fn function_swid(&self) -> u8 {
        (self.function << 4) | (self.swid & 0x0f)
    }

    /// Encodes the frame, zero-padded to the report's fixed length.
    ///
    /// # Errors
    ///
    /// Returns [`FrameError`] when a field does not fit the report.
    pub fn encode(&self) -> Result<Vec<u8>, FrameError> {
        for (field, value) in [("function", self.function), ("swid", self.swid)] {
            if value > 0x0f {
                return Err(FrameError::NibbleOutOfRange { field, value });
            }
        }
        if self.report.is_vlp() != self.seq.is_some() {
            return Err(FrameError::SeqMismatch {
                report: self.report,
            });
        }
        let max = self.report.size() - self.report.header_len();
        if self.params.len() > max {
            return Err(FrameError::ParamsTooLong {
                report: self.report,
                max,
                got: self.params.len(),
            });
        }
        let mut bytes = Vec::with_capacity(self.report.size());
        bytes.extend([
            self.report.byte(),
            self.device_index,
            self.feature_index,
            self.function_swid(),
        ]);
        bytes.extend(self.seq);
        bytes.extend_from_slice(&self.params);
        bytes.resize(self.report.size(), 0);
        Ok(bytes)
    }

    /// Parses one inbound report.
    ///
    /// # Errors
    ///
    /// Returns [`FrameError`] for an unknown report id or a wrong length.
    pub fn parse(bytes: &[u8]) -> Result<Self, FrameError> {
        let first = *bytes.first().ok_or(FrameError::Empty)?;
        let report = ReportId::from_byte(first).ok_or(FrameError::UnknownReport(first))?;
        if bytes.len() != report.size() {
            return Err(FrameError::Length {
                report,
                expected: report.size(),
                got: bytes.len(),
            });
        }
        let header = report.header_len();
        Ok(Self {
            report,
            device_index: bytes[1],
            feature_index: bytes[2],
            function: bytes[3] >> 4,
            swid: bytes[3] & 0x0f,
            seq: report.is_vlp().then(|| bytes[4]),
            params: bytes[header..].to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_report_packs_function_and_swid_into_byte_3() {
        let frame = Frame::new(ReportId::Long, 0xff, 0x04, 0x1, 0xe, &[0x0b, 0xb8]);
        let bytes = frame.encode().unwrap();
        assert_eq!(bytes.len(), 20);
        assert_eq!(&bytes[..6], &[0x11, 0xff, 0x04, 0x1e, 0x0b, 0xb8]);
        assert!(bytes[6..].iter().all(|b| *b == 0));

        let parsed = Frame::parse(&bytes).unwrap();
        assert_eq!(parsed.report, ReportId::Long);
        assert_eq!(parsed.device_index, 0xff);
        assert_eq!(parsed.feature_index, 0x04);
        assert_eq!(parsed.function, 0x1);
        assert_eq!(parsed.swid, 0xe);
        assert_eq!(parsed.seq, None);
        assert_eq!(&parsed.params[..2], &[0x0b, 0xb8]);
        assert_eq!(parsed.params.len(), 16);
    }

    #[test]
    fn vlp_control_report_carries_a_sequence_byte() {
        let frame = Frame::new(ReportId::VlpControl, 0xff, 0x00, 0x0, 0xe, &[0x19, 0xa1])
            .with_seq(VLP_SEQ_SINGLE);
        let bytes = frame.encode().unwrap();
        assert_eq!(bytes.len(), 32);
        assert_eq!(&bytes[..7], &[0x13, 0xff, 0x00, 0x0e, 0xe0, 0x19, 0xa1]);

        let parsed = Frame::parse(&bytes).unwrap();
        assert_eq!(parsed.seq, Some(0xe0));
        assert_eq!(parsed.params.len(), 27);
        assert_eq!(parsed.encode().unwrap(), bytes);
    }

    #[test]
    fn short_report_is_seven_bytes() {
        let bytes = [0x10, 0xff, 0x8f, 0x00, 0x1e, 0x05, 0x00];
        let parsed = Frame::parse(&bytes).unwrap();
        assert_eq!(parsed.report, ReportId::Short);
        assert_eq!(parsed.encode().unwrap(), bytes);
    }

    #[test]
    fn hcooper_lcd_packet_round_trips() {
        // captures/external/hcooper/derived/frame521.hex: the first 4095-byte
        // LCD write of 02-binding-paint.pcap (frame #521).
        let hex = include_str!("../../../captures/external/hcooper/derived/frame521.hex");
        let bytes = decode_hex(hex.trim());
        assert_eq!(bytes.len(), 4095);

        let parsed = Frame::parse(&bytes).unwrap();
        assert_eq!(parsed.report, ReportId::VlpBulk);
        assert_eq!(parsed.feature_index, 0x02);
        assert_eq!(parsed.function, 0x2);
        assert_eq!(parsed.swid, 0xd);
        assert_eq!(parsed.seq, Some(0xe0));
        assert_eq!(&parsed.params[..4], &[0x01, 0x00, 0x01, 0x00]);
        assert_eq!(parsed.encode().unwrap(), bytes);
    }

    #[test]
    fn encode_rejects_values_that_do_not_fit() {
        let too_long = [0u8; 17];
        assert_eq!(
            Frame::new(ReportId::Long, 0xff, 0, 0, 1, &too_long).encode(),
            Err(FrameError::ParamsTooLong {
                report: ReportId::Long,
                max: 16,
                got: 17
            })
        );
        assert_eq!(
            Frame::new(ReportId::Long, 0xff, 0, 0x10, 1, &[]).encode(),
            Err(FrameError::NibbleOutOfRange {
                field: "function",
                value: 0x10
            })
        );
        assert_eq!(
            Frame::new(ReportId::Long, 0xff, 0, 0, 0x10, &[]).encode(),
            Err(FrameError::NibbleOutOfRange {
                field: "swid",
                value: 0x10
            })
        );
        assert_eq!(
            Frame::new(ReportId::VlpControl, 0xff, 0, 0, 1, &[]).encode(),
            Err(FrameError::SeqMismatch {
                report: ReportId::VlpControl
            })
        );
        assert_eq!(
            Frame::new(ReportId::Long, 0xff, 0, 0, 1, &[])
                .with_seq(0xe0)
                .encode(),
            Err(FrameError::SeqMismatch {
                report: ReportId::Long
            })
        );
    }

    #[test]
    fn parse_rejects_unknown_ids_and_wrong_lengths() {
        assert_eq!(
            Frame::parse(&[0x02, 0x00]),
            Err(FrameError::UnknownReport(0x02))
        );
        assert_eq!(Frame::parse(&[]), Err(FrameError::Empty));
        assert_eq!(
            Frame::parse(&[0x11, 0xff, 0x00, 0x1e]),
            Err(FrameError::Length {
                report: ReportId::Long,
                expected: 20,
                got: 4
            })
        );
    }

    fn decode_hex(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect()
    }
}
