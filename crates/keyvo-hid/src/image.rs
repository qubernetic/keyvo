//! LCD image writes on the VLP bulk report `0x14` (spec §5.5): a JPEG is cut
//! into 4095-byte fragments addressed to the display feature `0x19A1`.
//!
//! The first fragment carries a 20-byte header: the VLP header, `01 00 01 00`
//! (display 1, no deferred update, one image, format JPEG in the Julusian
//! reading), the rectangle and the 24-bit JPEG length. Continuation fragments
//! carry the 5-byte VLP header only. Every fragment is zero-padded.

use std::time::Duration;

use crate::display::Geometry;
use crate::frame::{Frame, ReportId, VLP_SEQ_LAST};
use keyvo_core::hid::HidTransport;

use crate::hidpp::{Hidpp, HidppError, Plane, SoftwareId};
use crate::root::feature;

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

/// Where an image goes. Both placements come from the geometry the device
/// publishes (display fn1, spec §5.4); nothing about the grid is hard-coded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// One key, numbered from 1 in table order.
    Key(u8),
    /// The whole panel: the bounding box of all keys (435x434 at 23,6 on the
    /// Keypad, as Options+ paints it).
    Panel,
}

impl Target {
    /// The rectangle of this target in `geometry`.
    ///
    /// # Errors
    ///
    /// [`ImageError::NoSuchKey`] for a key the table does not list,
    /// [`ImageError::NoKeys`] for a panel without keys.
    pub fn rect(self, geometry: &Geometry) -> Result<Rect, ImageError> {
        match self {
            Self::Key(key) => geometry
                .keys
                .iter()
                .find(|rect| rect.index == key)
                .map(|rect| Rect {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                })
                .ok_or(ImageError::NoSuchKey {
                    key,
                    count: geometry.keys.len(),
                }),
            Self::Panel => {
                let left = geometry.keys.iter().map(|k| k.x).min();
                let top = geometry.keys.iter().map(|k| k.y).min();
                let right = geometry
                    .keys
                    .iter()
                    .map(|k| k.x.saturating_add(k.width))
                    .max();
                let bottom = geometry
                    .keys
                    .iter()
                    .map(|k| k.y.saturating_add(k.height))
                    .max();
                let (Some(x), Some(y), Some(right), Some(bottom)) = (left, top, right, bottom)
                else {
                    return Err(ImageError::NoKeys);
                };
                Ok(Rect {
                    x,
                    y,
                    width: right - x,
                    height: bottom - y,
                })
            }
        }
    }
}

/// The index the first fragment of an image carries in its sequence byte.
/// Options+ counts from 0 (captured), Julusian and logilinux from 1; both
/// paint with either software id (measured, `docs/hardware-notes.md` item 7),
/// so the Options+ default stays.
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
    /// The geometry does not list the key.
    #[error("no key {key}; the geometry lists {count}")]
    NoSuchKey {
        /// The requested key.
        key: u8,
        /// Keys in the geometry.
        count: usize,
    },
    /// A panel paint on a geometry without keys.
    #[error("the geometry lists no keys to place the panel by")]
    NoKeys,
    /// The bytes do not start with the JPEG SOI marker.
    #[error("not a jpeg: no start-of-image marker")]
    NotJpeg,
    /// No complete frame header (SOF) before the scan data or the end.
    #[error("jpeg has no complete frame header")]
    NoFrameHeader,
    /// A frame header other than baseline SOF0 (progressive, lossless, ...).
    #[error("jpeg frame marker 0x{marker:02x} is not baseline (0xc0)")]
    NotBaseline {
        /// The SOF marker byte.
        marker: u8,
    },
    /// The JPEG is not the size of its rectangle; the device would draw
    /// nothing and still accept the write (spec §5.5).
    #[error("jpeg is {jpeg_width}x{jpeg_height}, the rectangle {width}x{height}")]
    DimensionMismatch {
        /// Width in the SOF header.
        jpeg_width: u16,
        /// Height in the SOF header.
        jpeg_height: u16,
        /// Rectangle width.
        width: u16,
        /// Rectangle height.
        height: u16,
    },
}

/// SOI, the first two bytes of every JPEG.
const JPEG_SOI: [u8; 2] = [0xff, 0xd8];
/// SOF0, the baseline frame header.
const JPEG_SOF_BASELINE: u8 = 0xc0;
/// SOS: scan data follows; a frame header must come before it.
const JPEG_SOS: u8 = 0xda;
/// EOI.
const JPEG_EOI: u8 = 0xd9;

/// Width and height from the frame header of a baseline JPEG, walking the
/// marker segments after SOI.
fn jpeg_dimensions(jpeg: &[u8]) -> Result<(u16, u16), ImageError> {
    if !jpeg.starts_with(&JPEG_SOI) {
        return Err(ImageError::NotJpeg);
    }
    let mut position = JPEG_SOI.len();
    loop {
        let Some(&[0xff, marker]) = jpeg.get(position..position + 2) else {
            return Err(ImageError::NoFrameHeader);
        };
        match marker {
            // A fill byte before the marker.
            0xff => position += 1,
            // Markers without a segment: TEM, RST0..RST7.
            0x01 | 0xd0..=0xd7 => position += 2,
            JPEG_SOS | JPEG_EOI => return Err(ImageError::NoFrameHeader),
            _ => {
                let segment = jpeg
                    .get(position + 2..)
                    .and_then(|rest| rest.split_first_chunk::<2>())
                    .map(|(length, _)| usize::from(u16::from_be_bytes(*length)))
                    .ok_or(ImageError::NoFrameHeader)?;
                // SOF0..SOF15 except DHT (c4), JPG (c8) and DAC (cc).
                if matches!(marker, 0xc0..=0xcf) && !matches!(marker, 0xc4 | 0xc8 | 0xcc) {
                    if marker != JPEG_SOF_BASELINE {
                        return Err(ImageError::NotBaseline { marker });
                    }
                    // Length (2), precision (1), height (2), width (2).
                    let Some(&[_, _, _, h0, h1, w0, w1]) = jpeg.get(position + 2..position + 9)
                    else {
                        return Err(ImageError::NoFrameHeader);
                    };
                    return Ok((u16::from_be_bytes([w0, w1]), u16::from_be_bytes([h0, h1])));
                }
                position += 2 + segment;
            }
        }
    }
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
    let (jpeg_width, jpeg_height) = jpeg_dimensions(jpeg)?;
    if (jpeg_width, jpeg_height) != (rect.width, rect.height) {
        return Err(ImageError::DimensionMismatch {
            jpeg_width,
            jpeg_height,
            width: rect.width,
            height: rect.height,
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

/// What one [`write`] sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Written {
    /// Fragments sent.
    pub fragments: usize,
    /// Fragments the device acknowledged within `[timing].ack_timeout`.
    pub acked: usize,
}

/// Paints `jpeg` at `rect`: resolves the display index, then sends the
/// fragments one by one, each after the device acknowledged the one before
/// on `0x13` (`13 ff <display> <fn|swid> c<index> 00 <last>`, `<last>` 01 on
/// the last fragment, else 00; measured, `docs/hardware-notes.md` item 7).
/// When an ACK does not come within `[timing].ack_timeout` the next fragment
/// goes out anyway; [`Written::acked`] tells how many came. Other reports
/// read while waiting stay with the [`Hidpp`] for the input path.
///
/// # Errors
///
/// The errors of [`fragments`], the feature lookup and the transport.
pub fn write<T: HidTransport>(
    hidpp: &mut Hidpp<T>,
    config: &ImageConfig,
    rect: Rect,
    jpeg: &[u8],
) -> Result<Written, ImageError> {
    let display_index = hidpp.feature_index(Plane::Vlp, feature::VLP_DISPLAY)?;
    let device_index = hidpp.device_index();
    let fragments = fragments(device_index, display_index, config, rect, jpeg)?;
    let mut acked = 0;
    for fragment in &fragments {
        hidpp
            .transport_mut()
            .write(fragment)
            .map_err(HidppError::from)?;
        let index = fragment[4] & SEQ_INDEX;
        let ack = hidpp.wait_for_within(config.ack_timeout, |frame| {
            frame.report == ReportId::VlpControl
                && frame.device_index == device_index
                && frame.feature_index == display_index
                && frame.function == FN_SET_IMAGE
                && frame.swid == config.swid.get()
                && frame.seq.is_some_and(|seq| seq & SEQ_INDEX == index)
        })?;
        acked += usize::from(ack.is_some());
    }
    Ok(Written {
        fragments: fragments.len(),
        acked,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use keyvo_core::hid::FakeHidTransport;

    const KEY: Rect = Rect {
        x: 181,
        y: 163,
        width: 118,
        height: 118,
    };

    /// `len` bytes that start like a baseline JPEG of `width` x `height`:
    /// SOI, then a SOF0 segment; the rest is filler the writer never reads.
    fn jpeg(width: u16, height: u16, len: usize) -> Vec<u8> {
        jpeg_with_sof(0xc0, width, height, len)
    }

    fn jpeg_with_sof(marker: u8, width: u16, height: u16, len: usize) -> Vec<u8> {
        let mut bytes = vec![0xff, 0xd8, 0xff, marker, 0x00, 0x11, 0x08];
        bytes.extend(height.to_be_bytes());
        bytes.extend(width.to_be_bytes());
        bytes.extend([0x03, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
        bytes.resize(len, 0x55);
        bytes
    }

    #[test]
    fn an_ack_for_another_software_id_does_not_count() {
        let config = ImageConfig::default();
        let image = jpeg(118, 118, 300);
        let fragment = fragments(0xff, 2, &config, KEY, &image).unwrap().remove(0);
        let mut get_feature = vec![0x13, 0xff, 0x00, 0x0d, 0xe0, 0x19, 0xa1];
        get_feature.resize(32, 0);
        let mut found = vec![0x13, 0xff, 0x00, 0x0d, 0xc0, 0x19, 0xa1, 0x02];
        found.resize(32, 0);
        // Fragment 0's ACK, but tagged with swid 0xe, not the writer's 0xd.
        let mut other_ack = vec![0x13, 0xff, 0x02, 0x2e, 0xc0];
        other_ack.resize(32, 0);
        let transport = FakeHidTransport::new()
            .expect(&get_feature, &[&found])
            .expect(&fragment, &[&other_ack]);
        let mut hidpp = Hidpp::new(transport, 0xff, config.swid, Duration::from_secs(1));
        assert_eq!(
            write(&mut hidpp, &config, KEY, &image),
            Ok(Written {
                fragments: 1,
                acked: 0
            })
        );
        assert_eq!(hidpp.take_unmatched(), Some(other_ack));
    }

    #[test]
    fn a_panel_needs_at_least_one_key() {
        let geometry = Geometry {
            shape: 1,
            dimension: 0x60,
            width: 480,
            height: 480,
            keys: Vec::new(),
            visible_areas: 0,
        };
        assert_eq!(Target::Panel.rect(&geometry), Err(ImageError::NoKeys));
    }

    #[test]
    fn the_sof_dimensions_must_equal_the_rectangle() {
        let config = ImageConfig::default();
        assert_eq!(
            fragments(0xff, 2, &config, KEY, &jpeg(118, 117, 300)),
            Err(ImageError::DimensionMismatch {
                jpeg_width: 118,
                jpeg_height: 117,
                width: 118,
                height: 118,
            })
        );
    }

    #[test]
    fn segments_before_the_frame_header_are_skipped() {
        // SOI, APP0 (JFIF, 16 bytes), a fill byte, DQT (4 bytes), then SOF0.
        let mut bytes = vec![0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10];
        bytes.extend([0x4a, 0x46, 0x49, 0x46, 0, 1, 1, 0, 0, 1, 0, 1, 0, 0]);
        bytes.extend([0xff, 0xff, 0xdb, 0x00, 0x04, 0x00, 0x01]);
        bytes.extend(&jpeg(118, 118, 300)[2..]);
        assert_eq!(
            fragments(0xff, 2, &ImageConfig::default(), KEY, &bytes).map(|f| f.len()),
            Ok(1)
        );
    }

    #[test]
    fn only_baseline_jpeg_is_written() {
        assert_eq!(
            fragments(
                0xff,
                2,
                &ImageConfig::default(),
                KEY,
                &jpeg_with_sof(0xc2, 118, 118, 300)
            ),
            Err(ImageError::NotBaseline { marker: 0xc2 })
        );
    }

    #[test]
    fn bytes_without_soi_or_frame_header_are_refused() {
        let config = ImageConfig::default();
        assert_eq!(
            fragments(0xff, 2, &config, KEY, &[]),
            Err(ImageError::NotJpeg)
        );
        assert_eq!(
            fragments(0xff, 2, &config, KEY, &[0x89, 0x50, 0x4e, 0x47]),
            Err(ImageError::NotJpeg)
        );
        // Scan data before any frame header.
        assert_eq!(
            fragments(0xff, 2, &config, KEY, &[0xff, 0xd8, 0xff, 0xda, 0x00, 0x02]),
            Err(ImageError::NoFrameHeader)
        );
        // A segment length that runs past the end.
        assert_eq!(
            fragments(
                0xff,
                2,
                &config,
                KEY,
                &[0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10, 0x4a]
            ),
            Err(ImageError::NoFrameHeader)
        );
        // A frame header cut short.
        assert_eq!(
            fragments(0xff, 2, &config, KEY, &jpeg(118, 118, 300)[..8]),
            Err(ImageError::NoFrameHeader)
        );
    }

    fn sequence_bytes(config: &ImageConfig, len: usize) -> Vec<u8> {
        fragments(0xff, 2, config, KEY, &jpeg(118, 118, len))
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
            fragments(0xff, 2, &base_zero, KEY, &jpeg(118, 118, max + 1)),
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
        let fragments =
            fragments(0xff, 2, &ImageConfig::default(), KEY, &jpeg(118, 118, 300)).unwrap();
        assert_eq!(
            fragments[0][..21],
            [
                0x14, 0xff, 0x02, 0x2d, 0xe0, 0x01, 0x00, 0x01, 0x00, 0x00, 0xb5, 0x00, 0xa3, 0x00,
                0x76, 0x00, 0x76, 0x00, 0x01, 0x2c, 0xff
            ]
        );
        assert_eq!(fragments[0].len(), 4095);
    }
}
