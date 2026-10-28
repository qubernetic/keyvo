//! Key input on the Keypad (spec §5.6) and the Dialpad (spec §5.8): LCD key
//! reports on the VLP plane and the diverted-control notifications of
//! `0x1B04` on the classic plane, both turned into press and release events
//! by diffing the held set a report carries against the one before, and the
//! Dialpad's rotation notifications of `0x4610`, one event each.
//!
//! Only reports with function 0 and software id 0 are input: an image
//! fragment's ACK on the same VLP index carries the write's function and
//! software id (spec §5.5), so it never reaches the diff.

use crate::frame::{Frame, ReportId};
use crate::multiroller;

/// First parameter byte of an LCD key report, before the held keys
/// (`13 ff 02 00 <seq> 01 <k1> <k2> … 00`; logilinux and Julusian check it).
const KEY_LIST_MARKER: u8 = 0x01;

/// A control that can be held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Control {
    /// An LCD key by its 1-based position in the published geometry.
    Key(u8),
    /// A diverted `0x1B04` control by its CID (`0x01A1` left page button,
    /// `0x01A2` right).
    Page(u16),
}

/// Whether a control went down or up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    /// The control is held from now on.
    Press,
    /// The control is no longer held.
    Release,
}

/// One press, release or turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    /// A control went down or up.
    Button {
        /// The control.
        control: Control,
        /// Down or up.
        edge: Edge,
    },
    /// A diverted `0x4610` roller turned.
    Rotate {
        /// The roller id, `0..count` of `getCapabilities`.
        roller: u8,
        /// Signed increments, as the device sends them.
        delta: i8,
    },
}

impl InputEvent {
    /// `control` went down.
    #[must_use]
    pub const fn press(control: Control) -> Self {
        Self::Button {
            control,
            edge: Edge::Press,
        }
    }

    /// `control` went up.
    #[must_use]
    pub const fn release(control: Control) -> Self {
        Self::Button {
            control,
            edge: Edge::Release,
        }
    }
}

/// The input decoder of one Keypad connection: which reports are input,
/// and what is held right now.
#[derive(Debug, Clone)]
pub struct Input {
    device_index: u8,
    display_index: Option<u8>,
    key_count: u8,
    reprog_index: Option<u8>,
    roller_index: Option<u8>,
    keys: Vec<Control>,
    pages: Vec<Control>,
}

impl Input {
    /// Decodes key reports of `device_index` on the display feature's VLP
    /// index; keys outside `1..=key_count` (the geometry's key count) are
    /// ignored.
    #[must_use]
    pub const fn new(device_index: u8, display_index: u8, key_count: u8) -> Self {
        Self {
            device_index,
            display_index: Some(display_index),
            key_count,
            reprog_index: None,
            roller_index: None,
            keys: Vec::new(),
            pages: Vec::new(),
        }
    }

    /// The Dialpad's decoder: no LCD keys, the diverted buttons of `0x1B04`
    /// at `reprog_index` and the rotations of `0x4610` at `roller_index`
    /// (`11 <index> <roller index> 00 <roller> <delta> …`).
    #[must_use]
    pub const fn dialpad(device_index: u8, reprog_index: u8, roller_index: u8) -> Self {
        Self {
            device_index,
            display_index: None,
            key_count: 0,
            reprog_index: Some(reprog_index),
            roller_index: Some(roller_index),
            keys: Vec::new(),
            pages: Vec::new(),
        }
    }

    /// Forgets what is held, without events: the device lost its link, and
    /// its next report starts from nothing held.
    pub fn forget_held(&mut self) {
        self.keys.clear();
        self.pages.clear();
    }

    /// Also decodes the diverted-control notifications of `0x1B04` at
    /// `reprog_index` on the classic plane (`11 ff <index> 00 <cid:u16> …
    /// 00 00`), once the page buttons are diverted.
    #[must_use]
    pub const fn with_page_buttons(mut self, reprog_index: u8) -> Self {
        self.reprog_index = Some(reprog_index);
        self
    }

    /// The events `frame` causes: releases of what is no longer held, in
    /// the order it was pressed, then presses of what is newly held, in the
    /// report's order. Empty for any report that is not input.
    pub fn feed(&mut self, frame: &Frame) -> Vec<InputEvent> {
        if frame.device_index != self.device_index || frame.function != 0 || frame.swid != 0 {
            return Vec::new();
        }
        if frame.report == ReportId::VlpControl
            && Some(frame.feature_index) == self.display_index
            && let Some(held) = self.held_keys(&frame.params)
        {
            return diff(&mut self.keys, held);
        }
        if frame.report == ReportId::Long && Some(frame.feature_index) == self.reprog_index {
            return diff(&mut self.pages, held_cids(&frame.params));
        }
        if frame.report == ReportId::Long
            && Some(frame.feature_index) == self.roller_index
            && let Some(rotation) = multiroller::rotation(&frame.params)
        {
            return vec![InputEvent::Rotate {
                roller: rotation.roller,
                delta: rotation.delta,
            }];
        }
        Vec::new()
    }

    fn held_keys(&self, params: &[u8]) -> Option<Vec<Control>> {
        let (&marker, keys) = params.split_first()?;
        if marker != KEY_LIST_MARKER {
            return None;
        }
        let mut held = Vec::new();
        for &key in keys.iter().take_while(|&&key| key != 0) {
            let control = Control::Key(key);
            if key <= self.key_count && !held.contains(&control) {
                held.push(control);
            }
        }
        Some(held)
    }
}

/// The CIDs of a diverted-control notification: big-endian u16 values up
/// to the first `0x0000`; several when several controls are held.
fn held_cids(params: &[u8]) -> Vec<Control> {
    let mut held = Vec::new();
    for pair in params.as_chunks::<2>().0 {
        let cid = u16::from_be_bytes(*pair);
        if cid == 0 {
            break;
        }
        if !held.contains(&Control::Page(cid)) {
            held.push(Control::Page(cid));
        }
    }
    held
}

/// Replaces `held` with `now` and returns the edges between them.
fn diff(held: &mut Vec<Control>, now: Vec<Control>) -> Vec<InputEvent> {
    let released = held
        .iter()
        .filter(|c| !now.contains(c))
        .map(|&c| InputEvent::release(c));
    let pressed = now
        .iter()
        .filter(|c| !held.contains(c))
        .map(|&c| InputEvent::press(c));
    let events = released.chain(pressed).collect();
    *held = now;
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Frame;

    const DISPLAY: u8 = 0x02;

    fn frame(hex: &str) -> Frame {
        let mut bytes: Vec<u8> = hex
            .split_whitespace()
            .map(|byte| u8::from_str_radix(byte, 16).unwrap())
            .collect();
        let len = if bytes[0] == 0x13 { 32 } else { 20 };
        bytes.resize(len, 0);
        Frame::parse(&bytes).unwrap()
    }

    fn input() -> Input {
        Input::new(0xff, DISPLAY, 9)
    }

    fn feed(input: &mut Input, reports: &[&str]) -> Vec<InputEvent> {
        reports
            .iter()
            .flat_map(|hex| input.feed(&frame(hex)))
            .collect()
    }

    #[test]
    fn a_held_set_report_presses_and_an_empty_one_releases() {
        let events = feed(&mut input(), &["13 ff 02 00 c0 01 05", "13 ff 02 00 c1 01"]);
        assert_eq!(
            events,
            [
                InputEvent::press(Control::Key(5)),
                InputEvent::release(Control::Key(5))
            ]
        );
    }

    #[test]
    fn overlapping_keys_release_before_they_press_in_report_order() {
        let events = feed(
            &mut input(),
            &[
                "13 ff 02 00 c0 01 01",
                "13 ff 02 00 c1 01 01 03 02",
                "13 ff 02 00 c2 01 02 04",
            ],
        );
        assert_eq!(
            events,
            [
                InputEvent::press(Control::Key(1)),
                InputEvent::press(Control::Key(3)),
                InputEvent::press(Control::Key(2)),
                InputEvent::release(Control::Key(1)),
                InputEvent::release(Control::Key(3)),
                InputEvent::press(Control::Key(4)),
            ]
        );
    }

    #[test]
    fn a_repeated_held_set_yields_nothing() {
        let events = feed(
            &mut input(),
            &["13 ff 02 00 c0 01 07", "13 ff 02 00 c1 01 07"],
        );
        assert_eq!(events, [InputEvent::press(Control::Key(7))]);
    }

    #[test]
    fn keys_outside_the_published_geometry_and_duplicates_are_ignored() {
        let events = feed(&mut input(), &["13 ff 02 00 c0 01 0a 02 02 ff"]);
        assert_eq!(events, [InputEvent::press(Control::Key(2))]);
    }

    #[test]
    fn an_ack_never_yields_a_key_event() {
        let mut input = input();
        // Image fragment ACKs (spec §5.5): function 2, swid 0xe and 0xd,
        // indices 0 and 1; the second one reads `01` where a key report
        // carries its marker.
        for ack in [
            "13 ff 02 2e c0 00 00",
            "13 ff 02 2e c1 00 01",
            "13 ff 02 2d c1 01 01",
            "13 ff 02 0e c0 01 01",
        ] {
            assert_eq!(input.feed(&frame(ack)), []);
        }
        // Nothing was recorded as held either.
        assert_eq!(feed(&mut input, &["13 ff 02 00 c0 01"]), []);
    }

    const REPROG: u8 = 0x0b;
    const LEFT: Control = Control::Page(0x01a1);
    const RIGHT: Control = Control::Page(0x01a2);

    #[test]
    fn diverted_page_buttons_press_and_release_by_cid_list() {
        let mut input = input().with_page_buttons(REPROG);
        let events = feed(
            &mut input,
            &[
                "11 ff 0b 00 01 a1",
                "11 ff 0b 00 01 a1 01 a2",
                "11 ff 0b 00 01 a2",
                "11 ff 0b 00",
            ],
        );
        assert_eq!(
            events,
            [
                InputEvent::press(LEFT),
                InputEvent::press(RIGHT),
                InputEvent::release(LEFT),
                InputEvent::release(RIGHT),
            ]
        );
    }

    #[test]
    fn page_buttons_and_keys_are_held_independently() {
        let mut input = input().with_page_buttons(REPROG);
        let events = feed(
            &mut input,
            &[
                "13 ff 02 00 c0 01 03",
                "11 ff 0b 00 01 a1",
                "13 ff 02 00 c1 01",
            ],
        );
        assert_eq!(
            events,
            [
                InputEvent::press(Control::Key(3)),
                InputEvent::press(LEFT),
                InputEvent::release(Control::Key(3)),
            ]
        );
    }

    #[test]
    fn page_button_replies_and_undiverted_input_are_not_events() {
        let mut input = input().with_page_buttons(REPROG);
        for other in [
            "11 ff 0b 3e 01 a1 03", // setCidReporting reply
            "11 ff 0b 2e 01 a1 01", // getCidReporting reply
            "11 ff 0c 00 01 a1",    // another feature
        ] {
            assert_eq!(input.feed(&frame(other)), [], "{other}");
        }
        // Without the reprog index nothing on the classic plane is input.
        assert_eq!(self::input().feed(&frame("11 ff 0b 00 01 a1")), []);
    }

    /// The Dialpad on receiver slot 3: `0x1B04` at 0x0a, `0x4610` at 0x0d.
    fn dialpad() -> Input {
        Input::dialpad(0x03, 0x0a, 0x0d)
    }

    #[test]
    fn a_rotation_notification_is_a_rotate_event() {
        let events = feed(
            &mut dialpad(),
            &["11 03 0d 00 01 03 00 00 12 34", "11 03 0d 00 00 ff"],
        );
        assert_eq!(
            events,
            [
                InputEvent::Rotate {
                    roller: 1,
                    delta: 3
                },
                InputEvent::Rotate {
                    roller: 0,
                    delta: -1
                },
            ]
        );
    }

    #[test]
    fn the_dialpad_buttons_press_once_while_the_device_repeats_them() {
        let events = feed(
            &mut dialpad(),
            &[
                "11 03 0a 00 00 53",
                "11 03 0a 00 00 53",
                "11 03 0a 00 00 53",
                "11 03 0a 00",
            ],
        );
        assert_eq!(
            events,
            [
                InputEvent::press(Control::Page(0x0053)),
                InputEvent::release(Control::Page(0x0053)),
            ]
        );
    }

    #[test]
    fn other_multiroller_reports_are_not_rotations() {
        let mut input = dialpad();
        for other in [
            "11 03 0d 3e 00 01", // setMode reply
            "11 03 0d 10 01 03", // another event number
            "11 02 0d 00 01 03", // another slot
            "13 03 0d 00 c0 01", // VLP plane
        ] {
            assert_eq!(input.feed(&frame(other)), [], "{other}");
        }
        // The Keypad's decoder knows no rollers.
        assert_eq!(
            self::input()
                .with_page_buttons(0x0b)
                .feed(&frame("11 ff 0d 00 01 03")),
            []
        );
    }

    #[test]
    fn other_reports_on_the_vlp_plane_are_not_key_input() {
        let mut input = input();
        for other in [
            "13 ff 02 10 c6 a0",    // display mode-change event
            "13 ff 03 00 c0 01 01", // another VLP feature
            "13 01 02 00 c0 01 01", // another device index
            "13 ff 02 00 c0 02 01", // not the key-list marker
            "11 ff 02 00 01 01",    // classic plane
        ] {
            assert_eq!(input.feed(&frame(other)), [], "{other}");
        }
    }
}
