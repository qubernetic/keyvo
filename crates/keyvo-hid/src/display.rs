//! The VLP contextual display feature `0x19A1` (spec §5.4): the display claim
//! that hands the panel from the device's logo mode to the host, and the key
//! geometry the device publishes. The display index is resolved through the
//! VLP root on every connection; nothing about the grid is hard-coded.
//!
//! Keep-alive comes first: the caller starts pinging
//! ([`crate::keep_alive::KeepAlive`]) before it claims the display.

use keyvo_core::hid::HidTransport;
use keyvo_core::render::{KeyPlace, Panel, TileSize};

use crate::frame::ReportId;
use crate::hidpp::{Hidpp, HidppError, Plane};
use crate::root::{VlpProtocol, feature};

/// `getGeometry(screen)`.
const FN_GET_GEOMETRY: u8 = 1;
/// `getSupportedDeviceStates`: the modes the display accepts.
const FN_GET_MODES: u8 = 3;
/// `setMode(mode)`.
const FN_SET_MODE: u8 = 4;
/// `getMode`: the current mode.
const FN_GET_MODE: u8 = 5;
/// Function byte of the unsolicited mode-change event (`13 ff 02 10 c6 a0`).
const EVENT_MODE_CHANGED: u8 = 1;

/// A display mode (device state) byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayMode(pub u8);

impl DisplayMode {
    /// `a0`: host-controlled display (Options+ `Streaming`); what the claim
    /// sets.
    pub const STREAMING: Self = Self(0xa0);
    /// `a1`: the device/logo mode (Options+ `Standby`), current at boot.
    pub const STANDBY: Self = Self(0xa1);
}

/// What the claim found and did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    /// The VLP root's protocol capabilities.
    pub protocol: VlpProtocol,
    /// Index of the display feature on the VLP plane.
    pub display_index: u8,
    /// The modes the display accepts.
    pub modes: Vec<DisplayMode>,
    /// The mode the display was in before the claim.
    pub boot_mode: DisplayMode,
    /// The key geometry.
    pub geometry: Geometry,
    /// True when the device announced the switch with its mode-change event.
    /// The switch itself is verified with `getMode`; the event is consumed so
    /// it does not reach the input path.
    pub mode_event: bool,
}

/// Errors of [`claim`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ClaimError {
    /// A request failed.
    #[error(transparent)]
    Hidpp(#[from] HidppError),
    /// The display does not list the streaming mode.
    #[error("display does not support streaming mode a0; modes: {modes:02x?}")]
    StreamingUnsupported {
        /// The mode bytes it lists.
        modes: Vec<u8>,
    },
    /// After `setMode(a0)` the display still reports another mode.
    #[error("display stayed in mode 0x{mode:02x} after setMode(a0)")]
    ModeNotConfirmed {
        /// The mode `getMode` reported.
        mode: u8,
    },
}

/// Claims the display (spec §5.4): VLP root protocol capabilities, the
/// display index, the supported modes, the current mode, the geometry, then
/// `setMode(a0)` confirmed with `getMode` unless the display already streams.
/// VLP root fn2 (the ping Options+ sends) is left out until the hardware
/// check shows it is needed.
///
/// # Errors
///
/// [`ClaimError::StreamingUnsupported`], [`ClaimError::ModeNotConfirmed`],
/// and the request errors.
pub fn claim<T: HidTransport>(hidpp: &mut Hidpp<T>) -> Result<Claim, ClaimError> {
    let protocol = hidpp.vlp_protocol()?;
    let display_index = hidpp.feature_index(Plane::Vlp, feature::VLP_DISPLAY)?;
    let modes = supported_modes(hidpp)?;
    if !modes.contains(&DisplayMode::STREAMING) {
        return Err(ClaimError::StreamingUnsupported {
            modes: modes.iter().map(|mode| mode.0).collect(),
        });
    }
    let boot_mode = current_mode(hidpp)?;
    let geometry = geometry(hidpp)?;
    let mut mode_event = false;
    if boot_mode != DisplayMode::STREAMING {
        hidpp.request(
            Plane::Vlp,
            display_index,
            FN_SET_MODE,
            &[DisplayMode::STREAMING.0],
        )?;
        let mode = current_mode(hidpp)?;
        if mode != DisplayMode::STREAMING {
            return Err(ClaimError::ModeNotConfirmed { mode: mode.0 });
        }
        mode_event = hidpp
            .wait_for(|frame| {
                frame.report == ReportId::VlpControl
                    && frame.feature_index == display_index
                    && frame.function == EVENT_MODE_CHANGED
                    && frame.swid == 0
                    && frame.params.first() == Some(&DisplayMode::STREAMING.0)
            })?
            .is_some();
    }
    Ok(Claim {
        protocol,
        display_index,
        modes,
        boot_mode,
        geometry,
        mode_event,
    })
}

/// The modes the display accepts (display fn3), in the device's order.
///
/// # Errors
///
/// The request errors.
pub fn supported_modes<T: HidTransport>(
    hidpp: &mut Hidpp<T>,
) -> Result<Vec<DisplayMode>, HidppError> {
    let index = hidpp.feature_index(Plane::Vlp, feature::VLP_DISPLAY)?;
    let reply = hidpp.request(Plane::Vlp, index, FN_GET_MODES, &[])?;
    Ok(reply
        .params
        .iter()
        .take_while(|&&mode| mode != 0)
        .map(|&mode| DisplayMode(mode))
        .collect())
}

/// The display's current mode (display fn5).
///
/// # Errors
///
/// [`HidppError::Malformed`] for an empty reply, plus the request errors.
pub fn current_mode<T: HidTransport>(hidpp: &mut Hidpp<T>) -> Result<DisplayMode, HidppError> {
    let index = hidpp.feature_index(Plane::Vlp, feature::VLP_DISPLAY)?;
    let reply = hidpp.request(Plane::Vlp, index, FN_GET_MODE, &[])?;
    reply
        .params
        .first()
        .map(|&mode| DisplayMode(mode))
        .ok_or_else(|| HidppError::Malformed("empty getMode reply".into()))
}

/// The screen keyvo asks about; the Keypad has one (captured request `01`).
const SCREEN: u8 = 1;
/// Bytes before the first key record: shape, dimension (u16), width (u16),
/// height (u16), key count, and a second count (0 in the capture, meaning
/// unconfirmed).
const GEOMETRY_HEADER_LEN: usize = 9;
/// Bytes of one key record: shape, x, y, w, h (u16 each).
const KEY_RECORD_LEN: usize = 9;

/// One key's rectangle on the panel, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyRect {
    /// 1-based position in the table (row-major on the Keypad).
    pub index: u8,
    /// Shape byte of the record (1 in the capture).
    pub shape: u8,
    /// Left edge.
    pub x: u16,
    /// Top edge.
    pub y: u16,
    /// Width.
    pub width: u16,
    /// Height.
    pub height: u16,
}

/// The display geometry from display fn1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Geometry {
    /// Display shape byte (1 in the capture).
    pub shape: u8,
    /// The `dimension` field (0x0060 in the capture, meaning unconfirmed).
    pub dimension: u16,
    /// Panel width in pixels.
    pub width: u16,
    /// Panel height in pixels.
    pub height: u16,
    /// The keys in table order.
    pub keys: Vec<KeyRect>,
}

impl Geometry {
    /// The panel the renderer draws on: the panel size and every key's
    /// rectangle, in table order.
    #[must_use]
    pub fn panel(&self) -> Panel {
        Panel {
            size: TileSize {
                width: self.width,
                height: self.height,
            },
            keys: self
                .keys
                .iter()
                .map(|key| KeyPlace {
                    x: key.x,
                    y: key.y,
                    size: TileSize {
                        width: key.width,
                        height: key.height,
                    },
                })
                .collect(),
        }
    }
}

/// Origins of the nine 118x118 keys in the captured table (spec §5.4).
const CAPTURED_ORIGINS: [(u16, u16); 9] = [
    (24, 6),
    (182, 7),
    (340, 6),
    (23, 164),
    (181, 163),
    (339, 164),
    (23, 322),
    (181, 321),
    (340, 321),
];
/// Side of every key in the captured table.
const CAPTURED_KEY_SIDE: u16 = 118;

/// The MX Creative Keypad's geometry as captured (spec §5.4, fixture
/// `display-geometry.txt`), for rendering without a device (`keyvo
/// preview`). With a device, keyvo always uses the table the device
/// publishes.
#[must_use]
pub fn captured_geometry() -> Geometry {
    Geometry {
        shape: 1,
        dimension: 0x0060,
        width: 480,
        height: 480,
        keys: (1..)
            .zip(CAPTURED_ORIGINS)
            .map(|(index, (x, y))| KeyRect {
                index,
                shape: 1,
                x,
                y,
                width: CAPTURED_KEY_SIDE,
                height: CAPTURED_KEY_SIDE,
            })
            .collect(),
    }
}

/// Asks the display for its geometry table, resolving the display index
/// first.
///
/// # Errors
///
/// [`HidppError::Malformed`] when the table is shorter than its key count
/// says or a key lies outside the panel, plus the request errors.
pub fn geometry<T: HidTransport>(hidpp: &mut Hidpp<T>) -> Result<Geometry, HidppError> {
    let index = hidpp.feature_index(Plane::Vlp, feature::VLP_DISPLAY)?;
    let reply = hidpp.request(Plane::Vlp, index, FN_GET_GEOMETRY, &[SCREEN])?;
    decode_geometry(&reply.params)
}

fn decode_geometry(params: &[u8]) -> Result<Geometry, HidppError> {
    let Some((header, records)) = params.split_first_chunk::<GEOMETRY_HEADER_LEN>() else {
        return Err(HidppError::Malformed("short geometry header".into()));
    };
    let [shape, d0, d1, w0, w1, h0, h1, count, _] = *header;
    let width = u16::from_be_bytes([w0, w1]);
    let height = u16::from_be_bytes([h0, h1]);
    let needed = usize::from(count) * KEY_RECORD_LEN;
    let Some(records) = records.get(..needed) else {
        return Err(HidppError::Malformed(format!(
            "geometry announces {count} keys but carries {} record bytes",
            records.len()
        )));
    };
    let keys = (1..=count)
        .zip(records.as_chunks::<KEY_RECORD_LEN>().0)
        .map(|(index, record)| key_rect(index, record, width, height))
        .collect::<Result<_, _>>()?;
    Ok(Geometry {
        shape,
        dimension: u16::from_be_bytes([d0, d1]),
        width,
        height,
        keys,
    })
}

fn key_rect(
    index: u8,
    record: &[u8; KEY_RECORD_LEN],
    width: u16,
    height: u16,
) -> Result<KeyRect, HidppError> {
    let [shape, x0, x1, y0, y1, w0, w1, h0, h1] = *record;
    let key = KeyRect {
        index,
        shape,
        x: u16::from_be_bytes([x0, x1]),
        y: u16::from_be_bytes([y0, y1]),
        width: u16::from_be_bytes([w0, w1]),
        height: u16::from_be_bytes([h0, h1]),
    };
    let right = u32::from(key.x) + u32::from(key.width);
    let bottom = u32::from(key.y) + u32::from(key.height);
    if right > u32::from(width) || bottom > u32::from(height) {
        return Err(HidppError::Malformed(format!(
            "key {index} at ({}, {}) {}x{} lies outside the {width}x{height} panel",
            key.x, key.y, key.width, key.height
        )));
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hidpp::{DEVICE_DIRECT, SoftwareId};
    use keyvo_core::hid::FakeHidTransport;
    use std::time::Duration;

    #[test]
    fn the_panel_places_every_key_at_its_rectangle() {
        let panel = captured_geometry().panel();
        assert_eq!(
            panel.size,
            TileSize {
                width: 480,
                height: 480
            }
        );
        assert_eq!(panel.keys.len(), 9);
        assert_eq!(
            panel.keys[4],
            KeyPlace {
                x: 181,
                y: 163,
                size: TileSize {
                    width: 118,
                    height: 118
                }
            }
        );
    }

    /// One VLP report, swid 1, zero-padded.
    fn vlp(index: u8, function: u8, seq: u8, params: &[u8]) -> String {
        let mut bytes = vec![0x13, 0xff, index, (function << 4) | 0x1, seq];
        bytes.extend(params);
        bytes.resize(32, 0);
        bytes
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The claim up to and including the geometry, with `modes` and `boot`.
    fn claim_head(modes: &[u8], boot: u8) -> String {
        let geometry = [
            0x01, 0x00, 0x60, 0x01, 0xe0, 0x01, 0xe0, 0x01, 0x00, 0x01, 0x00, 0x18, 0x00, 0x06,
            0x00, 0x76, 0x00, 0x76,
        ];
        [
            format!("> {}", vlp(0, 1, 0xe0, &[])),
            format!(
                "< {}",
                vlp(0, 1, 0xc0, &[0x01, 0x00, 0x00, 0x01, 0xe8, 0x20])
            ),
            format!("> {}", vlp(0, 0, 0xe0, &[0x19, 0xa1])),
            format!("< {}", vlp(0, 0, 0xc0, &[0x19, 0xa1, 0x02])),
            format!("> {}", vlp(2, 3, 0xe0, &[])),
            format!("< {}", vlp(2, 3, 0xc0, modes)),
            format!("> {}", vlp(2, 5, 0xe0, &[])),
            format!("< {}", vlp(2, 5, 0xc0, &[boot])),
            format!("> {}", vlp(2, 1, 0xe0, &[0x01])),
            format!("< {}", vlp(2, 1, 0xc0, &geometry)),
        ]
        .join("\n")
            + "\n"
    }

    fn hidpp(script: &str) -> Hidpp<FakeHidTransport> {
        Hidpp::new(
            FakeHidTransport::from_script(script).unwrap(),
            DEVICE_DIRECT,
            SoftwareId::new(0x1).unwrap(),
            Duration::from_secs(1),
        )
    }

    #[test]
    fn a_display_that_already_streams_is_not_switched_again() {
        let mut hidpp = hidpp(&claim_head(&[0xa0, 0xa1], 0xa0));
        let claim = claim(&mut hidpp).unwrap();
        assert_eq!(claim.boot_mode, DisplayMode::STREAMING);
        assert!(!claim.mode_event);
        assert!(hidpp.transport_mut().is_done());
    }

    #[test]
    fn a_display_without_streaming_mode_is_refused() {
        let mut hidpp = hidpp(&claim_head(&[0xa1, 0xa2], 0xa1));
        assert_eq!(
            claim(&mut hidpp),
            Err(ClaimError::StreamingUnsupported {
                modes: vec![0xa1, 0xa2],
            })
        );
    }

    #[test]
    fn a_mode_that_does_not_change_is_not_a_claim() {
        let script = claim_head(&[0xa0, 0xa1], 0xa1)
            + &format!(
                "> {}\n< {}\n> {}\n< {}\n",
                vlp(2, 4, 0xe0, &[0xa0]),
                vlp(2, 4, 0xc0, &[0xa0]),
                vlp(2, 5, 0xe0, &[]),
                vlp(2, 5, 0xc0, &[0xa1]),
            );
        let mut hidpp = hidpp(&script);
        assert_eq!(
            claim(&mut hidpp),
            Err(ClaimError::ModeNotConfirmed { mode: 0xa1 })
        );
    }

    #[test]
    fn a_missing_mode_change_event_is_reported_not_fatal() {
        let script = claim_head(&[0xa0, 0xa1], 0xa1)
            + &format!(
                "> {}\n< {}\n> {}\n< {}\n",
                vlp(2, 4, 0xe0, &[0xa0]),
                vlp(2, 4, 0xc0, &[0xa0]),
                vlp(2, 5, 0xe0, &[]),
                vlp(2, 5, 0xc0, &[0xa0]),
            );
        let mut hidpp = hidpp(&script);
        let claim = claim(&mut hidpp).unwrap();
        assert!(!claim.mode_event);
        assert!(hidpp.transport_mut().is_done());
    }

    #[test]
    fn claim_errors_name_the_mode_bytes() {
        let unsupported = ClaimError::StreamingUnsupported {
            modes: vec![0xa1, 0xd0],
        };
        assert_eq!(
            unsupported.to_string(),
            "display does not support streaming mode a0; modes: [a1, d0]"
        );
        let stuck = ClaimError::ModeNotConfirmed { mode: 0xa1 };
        assert_eq!(
            stuck.to_string(),
            "display stayed in mode 0xa1 after setMode(a0)"
        );
    }

    fn table(count: u8, records: &[[u8; KEY_RECORD_LEN]]) -> Vec<u8> {
        let mut bytes = vec![0x01, 0x00, 0x60, 0x01, 0xe0, 0x01, 0xe0, count, 0x00];
        bytes.extend(records.iter().flatten());
        bytes
    }

    const KEY_1: [u8; KEY_RECORD_LEN] = [0x01, 0x00, 0x18, 0x00, 0x06, 0x00, 0x76, 0x00, 0x76];

    #[test]
    fn padding_after_the_records_is_ignored() {
        let mut params = table(1, &[KEY_1]);
        params.resize(54, 0);
        let geometry = decode_geometry(&params).unwrap();
        assert_eq!(geometry.keys.len(), 1);
        assert_eq!((geometry.shape, geometry.dimension), (1, 0x60));
    }

    #[test]
    fn fewer_records_than_announced_is_malformed() {
        assert!(matches!(
            decode_geometry(&table(2, &[KEY_1])),
            Err(HidppError::Malformed(_))
        ));
    }

    #[test]
    fn a_short_header_is_malformed() {
        assert!(matches!(
            decode_geometry(&[0x01, 0x00]),
            Err(HidppError::Malformed(_))
        ));
    }

    #[test]
    fn a_key_outside_the_panel_is_malformed() {
        let off_panel = [0x01, 0x01, 0x80, 0x00, 0x06, 0x00, 0x76, 0x00, 0x76];
        assert!(matches!(
            decode_geometry(&table(1, &[off_panel])),
            Err(HidppError::Malformed(_))
        ));
    }
}
