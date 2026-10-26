//! The VLP contextual display feature `0x19A1` (spec §5.4): the display claim
//! that hands the panel from the device's logo mode to the host, and the key
//! geometry the device publishes. The display index is resolved through the
//! VLP root on every connection; nothing about the grid is hard-coded.
//!
//! Keep-alive comes first: the caller starts pinging
//! ([`crate::keep_alive::KeepAlive`]) before it claims the display.

use std::time::Duration;

use keyvo_core::hid::HidTransport;
use keyvo_core::render::{KeyPlace, Panel, TileSize};

use crate::frame::{Frame, ReportId};
use crate::hidpp::{Hidpp, HidppError, Plane};
use crate::keep_alive;
use crate::root::{VlpProtocol, feature};

/// `getCapabilities`.
const FN_GET_CAPABILITIES: u8 = 0;
/// `getGeometry(screen)` (Options+ `getDisplayInfo`).
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
    /// `a1`: the device/logo mode (Options+ `Standby`), the state after a
    /// keep-alive expiry; a freshly plugged-in Keypad reports `a2`
    /// (measured, `docs/hardware-notes.md` item 5).
    pub const STANDBY: Self = Self(0xa1);
    /// `a4`: the plug-in splash (Options+ `SplashBright`); for a few seconds
    /// after a plug-in the display stays in it and does not take
    /// `setMode(a0)` (#52, `docs/hardware-notes.md` item 5).
    pub const SPLASH_BRIGHT: Self = Self(0xa4);
}

/// How the claim waits out the plug-in splash (spec §5.4):
/// `[timing].claim_splash_retry` and `[timing].claim_splash_timeout`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplashWait {
    /// Wait between two `setMode(a0)` while the display reports `a4`.
    pub retry: Duration,
    /// How long the claim waits in all. It re-sends `setMode(a0)` at most
    /// `timeout / retry` times; a zero `retry` does not wait.
    pub timeout: Duration,
}

impl Default for SplashWait {
    /// The `[timing]` defaults (spec §6.1): retry 500 ms, timeout 10 s.
    fn default() -> Self {
        Self {
            retry: Duration::from_millis(500),
            timeout: Duration::from_millis(10_000),
        }
    }
}

impl SplashWait {
    /// The number of `setMode(a0)` the wait may re-send.
    fn retries(self) -> u128 {
        self.timeout
            .as_nanos()
            .checked_div(self.retry.as_nanos())
            .unwrap_or(0)
    }
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
    /// How often `setMode(a0)` was sent again while the plug-in splash ran;
    /// 0 when the first one took.
    pub splash_retries: u32,
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
/// A display that reports `a4` after `setMode(a0)` still shows the plug-in
/// splash: the claim waits `splash.retry` for the mode-change event to
/// `a0`, then sends `setMode(a0)` again and confirms it with `getMode`,
/// until `a0` or until `splash.timeout` is used up. `between` runs before
/// every wait, so the caller's keep-alive keeps pinging.
///
/// # Errors
///
/// [`ClaimError::StreamingUnsupported`], [`ClaimError::ModeNotConfirmed`]
/// (at once for a mode other than `a0` and `a4`, for `a4` after the
/// timeout), and the request errors, also those of `between`.
pub fn claim<T: HidTransport>(
    hidpp: &mut Hidpp<T>,
    splash: SplashWait,
    mut between: impl FnMut(&mut Hidpp<T>) -> Result<(), HidppError>,
) -> Result<Claim, ClaimError> {
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
    let mut splash_retries = 0_u32;
    if boot_mode != DisplayMode::STREAMING {
        let streams = |frame: &Frame| {
            frame.report == ReportId::VlpControl
                && frame.feature_index == display_index
                && frame.function == EVENT_MODE_CHANGED
                && frame.swid == 0
                && frame.params.first() == Some(&DisplayMode::STREAMING.0)
        };
        let mut mode = set_streaming(hidpp, display_index)?;
        let mut rounds = 0..splash.retries();
        while mode == DisplayMode::SPLASH_BRIGHT && rounds.next().is_some() {
            between(hidpp)?;
            if hidpp.wait_for_within(splash.retry, streams)?.is_some() {
                mode = DisplayMode::STREAMING;
                mode_event = true;
                break;
            }
            splash_retries = splash_retries.saturating_add(1);
            mode = set_streaming(hidpp, display_index)?;
        }
        if mode != DisplayMode::STREAMING {
            return Err(ClaimError::ModeNotConfirmed { mode: mode.0 });
        }
        if !mode_event {
            mode_event = hidpp.wait_for(streams)?.is_some();
        }
        if splash_retries > 0 {
            // Mode events the splash sent before `a0` are older than the
            // confirmed `a0`; left kept, the claim watch would take them
            // for a lost claim.
            let watch = ClaimWatch::new(hidpp.device_index(), display_index);
            hidpp.discard_kept(|kept| watch.is_mode_event(kept));
        }
    }
    Ok(Claim {
        protocol,
        display_index,
        modes,
        boot_mode,
        geometry,
        mode_event,
        splash_retries,
    })
}

/// `setMode(a0)` confirmed with `getMode`.
fn stream<T: HidTransport>(hidpp: &mut Hidpp<T>, display_index: u8) -> Result<(), ClaimError> {
    let mode = set_streaming(hidpp, display_index)?;
    if mode != DisplayMode::STREAMING {
        return Err(ClaimError::ModeNotConfirmed { mode: mode.0 });
    }
    Ok(())
}

/// `setMode(a0)`, then the mode `getMode` reports.
fn set_streaming<T: HidTransport>(
    hidpp: &mut Hidpp<T>,
    display_index: u8,
) -> Result<DisplayMode, HidppError> {
    hidpp.request(
        Plane::Vlp,
        display_index,
        FN_SET_MODE,
        &[DisplayMode::STREAMING.0],
    )?;
    current_mode(hidpp)
}

/// Recognises the reports that mean the claim is lost (spec §5.3): the
/// display's mode-change event to any mode but `a0`, and the `KEEP_ALIVE`
/// expiry event. One expiry sends both in the same millisecond (first
/// light, `13 ff 02 10 cb a1` and `11 ff 04 00`); [`ClaimWatch::reclaim_on`]
/// turns them into one reclaim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimWatch {
    device: u8,
    display: u8,
    keep_alive: Option<u8>,
}

impl ClaimWatch {
    /// Watches the mode-change events of the display at `display_index` on
    /// the VLP plane of `device_index`.
    #[must_use]
    pub const fn new(device_index: u8, display_index: u8) -> Self {
        Self {
            device: device_index,
            display: display_index,
            keep_alive: None,
        }
    }

    /// Also watches the expiry event of `KEEP_ALIVE` at `keep_alive_index`
    /// on the classic plane, once the keep-alive runs.
    #[must_use]
    pub const fn with_keep_alive(mut self, keep_alive_index: u8) -> Self {
        self.keep_alive = Some(keep_alive_index);
        self
    }

    /// True when `frame` reports a lost claim.
    #[must_use]
    pub fn is_loss(&self, frame: &Frame) -> bool {
        self.is_mode_event(frame) && frame.params.first() != Some(&DisplayMode::STREAMING.0)
            || self.is_expiry(frame)
    }

    /// Re-sends `setMode(a0)`, confirmed with `getMode`, when `frame`
    /// reports a lost claim; true when it did, so the caller repaints.
    /// The mode-change and expiry events kept while the reclaim waited for
    /// its replies (the twin of the event that caused it, the `a0`
    /// announcement of its own `setMode`) are dropped: the confirmed `a0`
    /// is newer than they are.
    ///
    /// # Errors
    ///
    /// [`ClaimError::ModeNotConfirmed`] and the request errors.
    pub fn reclaim_on<T: HidTransport>(
        &self,
        hidpp: &mut Hidpp<T>,
        frame: &Frame,
    ) -> Result<bool, ClaimError> {
        if !self.is_loss(frame) {
            return Ok(false);
        }
        stream(hidpp, self.display)?;
        hidpp.discard_kept(|kept| self.is_mode_event(kept) || self.is_expiry(kept));
        Ok(true)
    }

    fn is_mode_event(self, frame: &Frame) -> bool {
        frame.report == ReportId::VlpControl
            && frame.device_index == self.device
            && frame.feature_index == self.display
            && frame.function == EVENT_MODE_CHANGED
            && frame.swid == 0
    }

    fn is_expiry(self, frame: &Frame) -> bool {
        frame.report == ReportId::Long
            && frame.device_index == self.device
            && Some(frame.feature_index) == self.keep_alive
            && frame.function == keep_alive::EVENT_EXPIRED
            && frame.swid == 0
    }
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

/// Bytes of the fn0 reply Options+ reads: screen count, maximum image size
/// (u16), maximum frame rate, three unread bytes, the flag byte.
const CAPABILITIES_LEN: usize = 8;
/// `maxImageSize` counts units of this many bytes.
const IMAGE_SIZE_UNIT: u32 = 1024;

/// What the display can take, from display fn0 (spec §5.5; field names as
/// Options+ logs them, `docs/research/options-plus-display-pipeline.md`
/// section 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "five independent flag bits of the device, one field each"
)]
pub struct Capabilities {
    /// `deviceScreenCount`.
    pub screen_count: u8,
    /// `maxImageSize` in bytes: the largest image one `setImage` may carry.
    pub max_image_size: u32,
    /// `maxImageFPS`.
    pub max_image_fps: u8,
    /// Flag bit 0, `deferrableDisplayUpdate`.
    pub deferrable_display_update: bool,
    /// Flag bit 1, `rgb565`.
    pub rgb565: bool,
    /// Flag bit 2, `rgb888`.
    pub rgb888: bool,
    /// Flag bit 3, `jpeg`.
    pub jpeg: bool,
    /// Flag bit 4, `calibrated`.
    pub calibrated: bool,
    /// Flag bits 5 and 6, `origin`.
    pub origin: u8,
}

/// Asks the display for its capabilities, resolving the display index
/// first.
///
/// # Errors
///
/// [`HidppError::Malformed`] for a reply shorter than the eight bytes
/// Options+ reads, plus the request errors.
pub fn capabilities<T: HidTransport>(hidpp: &mut Hidpp<T>) -> Result<Capabilities, HidppError> {
    let index = hidpp.feature_index(Plane::Vlp, feature::VLP_DISPLAY)?;
    let reply = hidpp.request(Plane::Vlp, index, FN_GET_CAPABILITIES, &[])?;
    decode_capabilities(&reply.params)
}

fn decode_capabilities(params: &[u8]) -> Result<Capabilities, HidppError> {
    let Some(&[screen_count, s0, s1, max_image_fps, _, _, _, flags]) =
        params.first_chunk::<CAPABILITIES_LEN>()
    else {
        return Err(HidppError::Malformed("short capabilities reply".into()));
    };
    let bit = |n: u8| flags & (1 << n) != 0;
    Ok(Capabilities {
        screen_count,
        max_image_size: u32::from(u16::from_be_bytes([s0, s1])) * IMAGE_SIZE_UNIT,
        max_image_fps,
        deferrable_display_update: bit(0),
        rgb565: bit(1),
        rgb888: bit(2),
        jpeg: bit(3),
        calibrated: bit(4),
        origin: (flags >> 5) & 0b11,
    })
}

/// The screen keyvo asks about; the Keypad has one (captured request `01`).
const SCREEN: u8 = 1;
/// Bytes before the first key record: shape, dimension (u16), width (u16),
/// height (u16), the button (key) count and the visible-area count (0 in
/// the capture).
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
    /// The keys (Options+ buttons) in table order; byte 7 counts them.
    pub keys: Vec<KeyRect>,
    /// Byte 8, the visible-area count. Their records follow the keys';
    /// keyvo does not read them (none on the Keypad).
    pub visible_areas: u8,
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
        visible_areas: 0,
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
    let [shape, d0, d1, w0, w1, h0, h1, count, visible_areas] = *header;
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
        visible_areas,
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

    /// A splash wait of three retries.
    const SPLASH: SplashWait = SplashWait {
        retry: Duration::from_millis(100),
        timeout: Duration::from_millis(300),
    };

    /// `setMode(a0)` and the `getMode` that answers `mode`.
    fn set_mode(mode: u8) -> String {
        format!(
            "> {}\n< {}\n> {}\n< {}\n",
            vlp(2, 4, 0xe0, &[0xa0]),
            vlp(2, 4, 0xc0, &[0xa0]),
            vlp(2, 5, 0xe0, &[]),
            vlp(2, 5, 0xc0, &[mode]),
        )
    }

    #[test]
    fn a_splash_that_outlasts_the_timeout_is_not_a_claim() {
        // The first setMode and one more per retry, all answered a4.
        let script = claim_head(&[0xa0, 0xa1, 0xa4], 0xa4) + &set_mode(0xa4).repeat(4);
        let mut hidpp = hidpp(&script);
        let mut polls = 0;
        let result = claim(&mut hidpp, SPLASH, |_| {
            polls += 1;
            Ok(())
        });
        assert_eq!(result, Err(ClaimError::ModeNotConfirmed { mode: 0xa4 }));
        assert_eq!(polls, 3);
        assert!(hidpp.transport_mut().is_done());
    }

    #[test]
    fn the_mode_change_event_to_a0_ends_the_splash_wait() {
        // The unsolicited event of display-claim.txt step 7.
        let event = format!("< 13 ff 02 10 c6 a0{}\n", " 00".repeat(26));
        let script = claim_head(&[0xa0, 0xa4], 0xa4) + &set_mode(0xa4) + &event;
        let mut hidpp = hidpp(&script);
        let claim = claim(&mut hidpp, SPLASH, |_| Ok(())).unwrap();
        assert!(claim.mode_event);
        assert_eq!(claim.splash_retries, 0);
        assert!(hidpp.transport_mut().is_done());
    }

    #[test]
    fn a_failing_keep_alive_ends_the_splash_wait() {
        let script = claim_head(&[0xa0, 0xa4], 0xa4) + &set_mode(0xa4);
        let mut hidpp = hidpp(&script);
        assert_eq!(
            claim(&mut hidpp, SPLASH, |_| Err(HidppError::InvalidSoftwareId(
                0
            ))),
            Err(ClaimError::Hidpp(HidppError::InvalidSoftwareId(0)))
        );
    }

    #[test]
    fn a_zero_retry_does_not_wait() {
        let script = claim_head(&[0xa0, 0xa4], 0xa4) + &set_mode(0xa4);
        let mut hidpp = hidpp(&script);
        let none = SplashWait {
            retry: Duration::ZERO,
            ..SPLASH
        };
        assert_eq!(
            claim(&mut hidpp, none, |_| Ok(())),
            Err(ClaimError::ModeNotConfirmed { mode: 0xa4 })
        );
    }

    #[test]
    fn a_display_that_already_streams_is_not_switched_again() {
        let mut hidpp = hidpp(&claim_head(&[0xa0, 0xa1], 0xa0));
        let claim = claim(&mut hidpp, SPLASH, |_| Ok(())).unwrap();
        assert_eq!(claim.boot_mode, DisplayMode::STREAMING);
        assert!(!claim.mode_event);
        assert!(hidpp.transport_mut().is_done());
    }

    #[test]
    fn a_display_without_streaming_mode_is_refused() {
        let mut hidpp = hidpp(&claim_head(&[0xa1, 0xa2], 0xa1));
        assert_eq!(
            claim(&mut hidpp, SPLASH, |_| Ok(())),
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
            claim(&mut hidpp, SPLASH, |_| Ok(())),
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
        let claim = claim(&mut hidpp, SPLASH, |_| Ok(())).unwrap();
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

    #[test]
    fn origin_is_the_two_bits_above_calibrated() {
        let caps = decode_capabilities(&[0x01, 0x00, 0x01, 0x0a, 0, 0, 0, 0xe4]).unwrap();
        assert_eq!(caps.max_image_size, 1024);
        assert_eq!(caps.origin, 3);
        assert!(caps.rgb888 && !caps.calibrated && !caps.jpeg && !caps.rgb565);
        assert!(!caps.deferrable_display_update);
    }

    #[test]
    fn a_short_capabilities_reply_is_malformed() {
        assert!(matches!(
            decode_capabilities(&[0x01, 0x00, 0x7a, 0x1e, 0, 0, 0]),
            Err(HidppError::Malformed(_))
        ));
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
    fn byte_8_is_the_visible_area_count() {
        let mut params = table(1, &[KEY_1]);
        params[8] = 2;
        let geometry = decode_geometry(&params).unwrap();
        assert_eq!((geometry.keys.len(), geometry.visible_areas), (1, 2));
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
