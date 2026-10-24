//! LCD image writes on the VLP bulk report `0x14` (spec §5.5): a JPEG is cut
//! into 4095-byte fragments addressed to the display feature `0x19A1`.
//!
//! The first fragment carries a 20-byte header: the VLP header, `01 00 01 00`
//! (display 1, no deferred update, one image, format JPEG in the Julusian
//! reading), the rectangle and the 24-bit JPEG length. Continuation fragments
//! carry the 5-byte VLP header only. Every fragment is zero-padded.

use std::time::Duration;

use crate::frame::{Frame, ReportId, VLP_SEQ_LAST};
use crate::hidpp::{HidppError, SoftwareId};

/// `setImage`, the display function the fragments carry.
const FN_SET_IMAGE: u8 = 2;
/// Sequence bit of the first fragment of an image.
const SEQ_FIRST: u8 = 0x80;
/// Sequence bit set on every fragment (`e0`, `a0`, `61` in the captures).
const SEQ_ALWAYS: u8 = 0x20;
/// The fragment index bits of the sequence byte.
const SEQ_INDEX: u8 = 0x0f;
/// Display 1, no deferred update, one image, format JPEG (Julusian's reading
/// of the constant `01 00 01 00`, unconfirmed).
const IMAGE_PREFIX: [u8; 4] = [0x01, 0x00, 0x01, 0x00];
/// Header bytes of the first fragment: the VLP header, [`IMAGE_PREFIX`],
/// x, y, w, h (u16 each) and the 24-bit length.
const FIRST_HEADER_LEN: usize = 20;
/// JPEG bytes in the first fragment.
const FIRST_PAYLOAD: usize = ReportId::VlpBulk.size() - FIRST_HEADER_LEN;
/// JPEG bytes in a continuation fragment.
const NEXT_PAYLOAD: usize = ReportId::VlpBulk.size() - ReportId::VlpBulk.header_len();

/// Software id Options+ tags its image writes with (captured).
pub const DEFAULT_SWID: SoftwareId = SoftwareId(0xd);
/// Default of `[timing].ack_timeout` (spec §6.1).
pub const DEFAULT_ACK_TIMEOUT: Duration = Duration::from_millis(50);

/// Placement of an image on the panel, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    /// Left edge.
    pub x: u16,
    /// Top edge.
    pub y: u16,
    /// Width.
    pub width: u16,
    /// Height.
    pub height: u16,
}

/// The index the first fragment of an image carries in its sequence byte.
/// Options+ counts from 0 (captured), Julusian and logilinux from 1; both are
/// reported to work, the hardware check (plan §5 item 7) decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IndexBase {
    /// `e0`, `a0 61`: Options+.
    #[default]
    Zero,
    /// `e1`, `a1 62`: Julusian, logilinux.
    One,
}

impl IndexBase {
    const fn first(self) -> u8 {
        match self {
            Self::Zero => 0,
            Self::One => 1,
        }
    }
}

/// The settings of an image write. Software id and index base are settings
/// until the hardware check (plan §5 item 7) fixes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageConfig {
    /// Software id of the writes and their ACKs; Options+ uses `0xd`.
    pub swid: SoftwareId,
    /// Fragment index of the first fragment.
    pub index_base: IndexBase,
    /// `[timing].ack_timeout`: how long a fragment waits for its ACK before
    /// the next one goes out anyway.
    pub ack_timeout: Duration,
}

impl ImageConfig {
    /// The largest JPEG one write carries: as many fragments as the 4-bit
    /// fragment index counts from the index base without wrapping. What the
    /// device does past that is not measured yet (plan §5 item 10).
    #[must_use]
    pub const fn max_image_len(&self) -> usize {
        let fragments = (SEQ_INDEX - self.index_base.first()) as usize + 1;
        FIRST_PAYLOAD + (fragments - 1) * NEXT_PAYLOAD
    }
}

impl Default for ImageConfig {
    fn default() -> Self {
        Self {
            swid: DEFAULT_SWID,
            index_base: IndexBase::default(),
            ack_timeout: DEFAULT_ACK_TIMEOUT,
        }
    }
}

/// Errors of an image write.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ImageError {
    /// A request or the transport failed.
    #[error(transparent)]
    Hidpp(#[from] HidppError),
    /// The JPEG needs more fragments than one write can number.
    #[error("jpeg of {len} bytes exceeds the {max}-byte limit of one write")]
    TooLarge {
        /// JPEG length.
        len: usize,
        /// [`ImageConfig::max_image_len`].
        max: usize,
    },
}

/// Cuts `jpeg` into the `0x14` reports that paint it at `rect`.
///
/// # Errors
///
/// [`ImageError::TooLarge`] past [`ImageConfig::max_image_len`].
pub fn fragments(
    device_index: u8,
    display_index: u8,
    config: &ImageConfig,
    rect: Rect,
    jpeg: &[u8],
) -> Result<Vec<Vec<u8>>, ImageError> {
    let limit = config.max_image_len();
    if jpeg.len() > limit {
        return Err(ImageError::TooLarge {
            len: jpeg.len(),
            max: limit,
        });
    }
    let report = ReportId::VlpBulk;
    let (head, tail) = jpeg.split_at(jpeg.len().min(FIRST_PAYLOAD));
    let mut payloads = vec![head];
    payloads.extend(tail.chunks(NEXT_PAYLOAD));
    let count = payloads.len();
    // The low three bytes; the limit keeps the length far below 2^24.
    let length = jpeg.len().to_be_bytes();
    let length = &length[length.len() - 3..];

    (config.index_base.first()..)
        .zip(payloads)
        .enumerate()
        .map(|(position, (index, payload))| {
            let mut seq = SEQ_ALWAYS | (index & SEQ_INDEX);
            let mut params = Vec::with_capacity(report.size());
            if position == 0 {
                seq |= SEQ_FIRST;
                params.extend(IMAGE_PREFIX);
                for value in [rect.x, rect.y, rect.width, rect.height] {
                    params.extend(value.to_be_bytes());
                }
                params.extend(length);
            }
            if position + 1 == count {
                seq |= VLP_SEQ_LAST;
            }
            params.extend_from_slice(payload);
            let frame = Frame::new(
                report,
                device_index,
                display_index,
                FN_SET_IMAGE,
                config.swid.get(),
                &params,
            )
            .with_seq(seq);
            Ok(frame.encode().map_err(HidppError::from)?)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: Rect = Rect {
        x: 181,
        y: 163,
        width: 118,
        height: 118,
    };

    fn sequence_bytes(config: &ImageConfig, len: usize) -> Vec<u8> {
        fragments(0xff, 2, config, KEY, &vec![0x55; len])
            .unwrap()
            .iter()
            .map(|fragment| fragment[4])
            .collect()
    }

    #[test]
    fn index_base_one_gives_the_flag_bytes_of_the_libraries() {
        // notno's packetizer tests (docs/research/logilinux-and-julusian-
        // analysis.md): e1 for one packet, a1 22 63 for three.
        let config = ImageConfig {
            index_base: IndexBase::One,
            ..ImageConfig::default()
        };
        assert_eq!(sequence_bytes(&config, 100), [0xe1]);
        assert_eq!(sequence_bytes(&config, 4075 + 4090 + 1), [0xa1, 0x22, 0x63]);
    }

    #[test]
    fn index_base_zero_is_the_options_plus_default() {
        let config = ImageConfig::default();
        assert_eq!(config.index_base, IndexBase::Zero);
        assert_eq!(sequence_bytes(&config, 4075), [0xe0]);
        assert_eq!(sequence_bytes(&config, 4076), [0xa0, 0x61]);
        assert_eq!(sequence_bytes(&config, 4075 + 4090 + 1), [0xa0, 0x21, 0x62]);
    }

    #[test]
    fn an_image_beyond_the_fragment_index_space_is_refused() {
        // 16 fragments with index base 0, 15 with base 1; no index wraps.
        let base_zero = ImageConfig::default();
        let max = 4075 + 15 * 4090;
        assert_eq!(base_zero.max_image_len(), max);
        assert_eq!(sequence_bytes(&base_zero, max).len(), 16);
        assert_eq!(
            fragments(0xff, 2, &base_zero, KEY, &vec![0; max + 1]),
            Err(ImageError::TooLarge { len: max + 1, max })
        );
        let base_one = ImageConfig {
            index_base: IndexBase::One,
            ..base_zero
        };
        assert_eq!(base_one.max_image_len(), max - 4090);
    }

    #[test]
    fn header_carries_the_rectangle_and_the_24_bit_length() {
        let fragments = fragments(0xff, 2, &ImageConfig::default(), KEY, &[0x55; 300]).unwrap();
        assert_eq!(
            fragments[0][..21],
            [
                0x14, 0xff, 0x02, 0x2d, 0xe0, 0x01, 0x00, 0x01, 0x00, 0x00, 0xb5, 0x00, 0xa3, 0x00,
                0x76, 0x00, 0x76, 0x00, 0x01, 0x2c, 0x55
            ]
        );
        assert_eq!(fragments[0].len(), 4095);
    }
}
