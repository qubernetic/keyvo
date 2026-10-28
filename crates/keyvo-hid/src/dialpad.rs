//! The MX Creative Dialpad (spec §5.8): found on a Bolt receiver slot at
//! runtime, or on device index `0xff` over Bluetooth; its four buttons
//! diverted through `0x1B04` and its two rollers through `0x4610`, both
//! resolved with `ROOT.getFeature`; its input read from the receiver's
//! HID++ node next to the kernel and Solaar (spec §5.1).
//!
//! The Dialpad has no keep-alive, no claim and no display. Its diverts are
//! volatile: [`Dialpad::divert`] re-sends them when the receiver announces
//! the Dialpad again, and [`Dialpad::restore`] gives them back on a clean
//! stop.

use std::collections::VecDeque;
use std::fmt;
use std::ops::RangeInclusive;
use std::time::{Duration, Instant};

use keyvo_core::hid::HidTransport;

use crate::firmware;
use crate::frame::{Frame, ReportId};
use crate::hidpp::{DEVICE_DIRECT, Hidpp, HidppError, Plane, ReceiverError};
use crate::input::{Input, InputEvent};
use crate::multiroller::{self, Mode, Roller};
use crate::reprog::{self, DivertError, DivertTiming};
use crate::root::feature;
use crate::sysfs::MX_CREATIVE_DIALPAD;

/// The device indices of a Bolt receiver's slots.
pub const SLOTS: RangeInclusive<u8> = 1..=6;

/// Default of `[timing].slot_probe` (spec §6.1): how long the search waits
/// for one slot's answer. A receiver answers for an empty slot at once; a
/// paired device answers within milliseconds when it is awake.
pub const DEFAULT_SLOT_PROBE: Duration = Duration::from_millis(300);

/// Sub id of the receiver's arrival notification (`10 <slot> 41 …`).
const ARRIVAL: u8 = 0x41;
/// Bit of the arrival's flags byte: the link is not established.
const LINK_LOST: u8 = 0x40;

/// How the Dialpad is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Link {
    /// On a Bolt receiver's node: search every slot of [`SLOTS`].
    Receiver,
    /// On a Bolt receiver's node, on this slot only (after its arrival).
    Slot(u8),
    /// A node of its own (Bluetooth), device index `0xff`.
    Direct,
}

/// The `[timing]` values of the Dialpad.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialpadTiming {
    /// `[timing].slot_probe`: the wait for one slot's answer.
    pub slot_probe: Duration,
    /// `[timing].divert_settle` and `divert_spacing`.
    pub divert: DivertTiming,
}

impl Default for DialpadTiming {
    fn default() -> Self {
        Self {
            slot_probe: DEFAULT_SLOT_PROBE,
            divert: DivertTiming::default(),
        }
    }
}

/// What one device index answered to the search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// The receiver answered with an error: no device on the slot.
    Empty(ReceiverError),
    /// No answer in time, or the receiver could not reach the device: a
    /// paired device that sleeps or is out of range.
    Silent,
    /// A device without `0x4610`.
    Absent,
    /// A device with `0x4610` whose model id does not carry `BC00`.
    Other {
        /// Its model id.
        model_id: [u8; 6],
    },
    /// The Dialpad.
    Dialpad,
}

impl fmt::Display for Slot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty(_) => f.write_str("empty"),
            Self::Silent => f.write_str("silent"),
            Self::Absent => f.write_str("no 0x4610"),
            Self::Other { model_id } => {
                write!(f, "model {}", keyvo_core::hid::to_hex(model_id))
            }
            Self::Dialpad => f.write_str("dialpad"),
        }
    }
}

/// The search's answers, device index first, in the order asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slots(pub Vec<(u8, Slot)>);

impl Slots {
    /// The slots that did not answer: worth asking again on their arrival.
    #[must_use]
    pub fn silent(&self) -> Vec<u8> {
        self.0
            .iter()
            .filter(|(_, slot)| *slot == Slot::Silent)
            .map(|(index, _)| *index)
            .collect()
    }
}

impl fmt::Display for Slots {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (n, (index, slot)) in self.0.iter().enumerate() {
            let sep = if n == 0 { "" } else { ", " };
            if *index == DEVICE_DIRECT {
                write!(f, "{sep}direct {slot}")?;
            } else {
                write!(f, "{sep}{index} {slot}")?;
            }
        }
        Ok(())
    }
}

/// Errors of the Dialpad driver.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DialpadError {
    /// A request failed.
    #[error(transparent)]
    Hidpp(#[from] HidppError),
    /// A divert failed.
    #[error(transparent)]
    Divert(#[from] DivertError),
    /// No device index answered as the Dialpad.
    #[error("no MX Creative Dialpad found (slots: {0})")]
    NotFound(Slots),
}

/// One thing the Dialpad did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialpadEvent {
    /// A button edge or a turn.
    Input(InputEvent),
    /// The receiver lost the link to the Dialpad (asleep, out of range);
    /// nothing is held any more.
    LinkLost,
    /// The receiver announced the Dialpad again; every divert was re-sent.
    Rediverted,
}

/// A receiver's arrival notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arrival {
    /// The slot.
    pub slot: u8,
    /// The device's WPID.
    pub wpid: u16,
    /// True when the link is established.
    pub linked: bool,
}

/// The arrival notification `frame` carries: `10 <slot> 41 <protocol>
/// <flags> <wpid lo> <wpid hi>`, flags bit 6 set when the link is not
/// established (static pass §8).
#[must_use]
pub fn arrival(frame: &Frame) -> Option<Arrival> {
    if frame.report != ReportId::Short || frame.feature_index != ARRIVAL {
        return None;
    }
    let [flags, lo, hi, ..] = frame.params[..] else {
        return None;
    };
    Some(Arrival {
        slot: frame.device_index,
        wpid: u16::from_le_bytes([lo, hi]),
        linked: flags & LINK_LOST == 0,
    })
}

/// Waits at most `timeout` for the receiver to announce a Dialpad with its
/// link established, or any device on one of the `silent` slots; its slot.
///
/// # Errors
///
/// The transport errors.
pub fn wait_for_arrival<T: HidTransport>(
    hidpp: &mut Hidpp<T>,
    silent: &[u8],
    timeout: Duration,
) -> Result<Option<u8>, HidppError> {
    let frame = hidpp.wait_for_within(timeout, |frame| {
        arrival(frame).is_some_and(|a| {
            a.linked && (a.wpid == MX_CREATIVE_DIALPAD || silent.contains(&a.slot))
        })
    })?;
    Ok(frame.map(|frame| frame.device_index))
}

/// One Dialpad connection: where it is, what it has, what is held.
#[derive(Debug, Clone)]
pub struct Dialpad {
    link: Link,
    device_index: u8,
    slots: Slots,
    reprog_index: u8,
    roller_index: u8,
    buttons: Vec<u16>,
    rollers: Vec<Roller>,
    timing: DialpadTiming,
    input: Input,
    pending: VecDeque<InputEvent>,
}

impl Dialpad {
    /// Finds the Dialpad and reads what it has, without diverting anything
    /// (`keyvo probe`): the slot search on a receiver, the `0x1B04` control
    /// walk after `timing.divert.settle`, the roller capabilities. Leaves
    /// `hidpp` on the Dialpad's device index.
    ///
    /// # Errors
    ///
    /// [`DialpadError::NotFound`] when no device index answers as the
    /// Dialpad, plus the request errors.
    pub fn find<T: HidTransport>(
        hidpp: &mut Hidpp<T>,
        link: Link,
        timing: &DialpadTiming,
        sleep: impl FnMut(Duration),
    ) -> Result<Self, DialpadError> {
        let slots = search(hidpp, link, timing.slot_probe)?;
        let Some(device_index) = slots
            .0
            .iter()
            .find(|(_, slot)| *slot == Slot::Dialpad)
            .map(|(index, _)| *index)
        else {
            return Err(DialpadError::NotFound(slots));
        };
        hidpp.set_device_index(device_index);
        let walked = reprog::controls(hidpp, &timing.divert, sleep)?;
        let mut buttons: Vec<u16> = walked
            .controls
            .iter()
            .filter(|control| control.divertable())
            .map(|control| control.cid)
            .collect();
        buttons.sort_unstable();
        let rollers = multiroller::rollers(hidpp)?;
        let roller_index = hidpp.feature_index(Plane::Classic, feature::MULTI_ROLLER)?;
        Ok(Self {
            link,
            device_index,
            slots,
            reprog_index: walked.reprog_index,
            roller_index,
            buttons,
            rollers,
            timing: *timing,
            input: Input::dialpad(device_index, walked.reprog_index, roller_index),
            pending: VecDeque::new(),
        })
    }

    /// [`Dialpad::find`], then every divert: the buttons with
    /// `setCidReporting(cid, 0x03)`, the rollers with `setMode(r,
    /// DIVERTED)`, each request after `timing.divert.spacing`.
    ///
    /// # Errors
    ///
    /// The errors of [`Dialpad::find`] and of the diverts.
    pub fn open<T: HidTransport>(
        hidpp: &mut Hidpp<T>,
        link: Link,
        timing: &DialpadTiming,
        mut sleep: impl FnMut(Duration),
    ) -> Result<Self, DialpadError> {
        let dialpad = Self::find(hidpp, link, timing, &mut sleep)?;
        dialpad.apply(hidpp, Mode::Diverted, sleep)?;
        Ok(dialpad)
    }

    /// Re-sends every divert after `timing.divert.settle`, as after the
    /// Dialpad's arrival.
    ///
    /// # Errors
    ///
    /// The divert errors.
    pub fn divert<T: HidTransport>(
        &self,
        hidpp: &mut Hidpp<T>,
        mut sleep: impl FnMut(Duration),
    ) -> Result<(), DialpadError> {
        sleep(self.timing.divert.settle);
        self.apply(hidpp, Mode::Diverted, sleep)
    }

    /// Gives every divert back: the buttons with `setCidReporting(cid,
    /// 0x02)`, the rollers with `setMode(r, NATIVE)`.
    ///
    /// # Errors
    ///
    /// The request errors.
    pub fn restore<T: HidTransport>(
        &self,
        hidpp: &mut Hidpp<T>,
        sleep: impl FnMut(Duration),
    ) -> Result<(), DialpadError> {
        self.apply(hidpp, Mode::Native, sleep)
    }

    fn apply<T: HidTransport>(
        &self,
        hidpp: &mut Hidpp<T>,
        mode: Mode,
        mut sleep: impl FnMut(Duration),
    ) -> Result<(), DialpadError> {
        hidpp.set_device_index(self.device_index);
        let timing = &self.timing.divert;
        match mode {
            Mode::Diverted => {
                reprog::divert(hidpp, self.reprog_index, &self.buttons, timing, &mut sleep)?;
            }
            Mode::Native => {
                reprog::undivert(hidpp, self.reprog_index, &self.buttons, timing, &mut sleep)?;
            }
        }
        for roller in &self.rollers {
            sleep(timing.spacing);
            multiroller::set_mode(hidpp, roller.id, mode)?;
        }
        Ok(())
    }

    /// The next button edge, turn or change of the link, waiting at most
    /// `timeout`; `None` when nothing came. The receiver's arrival with the
    /// link established re-sends every divert ([`Dialpad::divert`]) before
    /// it is reported. Reports of other devices and other software are
    /// skipped.
    ///
    /// # Errors
    ///
    /// The transport errors and the errors of the re-divert.
    pub fn next_event<T: HidTransport>(
        &mut self,
        hidpp: &mut Hidpp<T>,
        timeout: Duration,
        sleep: impl FnMut(Duration),
    ) -> Result<Option<DialpadEvent>, DialpadError> {
        if let Some(event) = self.pending.pop_front() {
            return Ok(Some(DialpadEvent::Input(event)));
        }
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let Some(frame) = hidpp.next_frame(remaining)? else {
                return Ok(None);
            };
            if self.link != Link::Direct
                && let Some(arrival) = arrival(&frame)
                && arrival.slot == self.device_index
            {
                if !arrival.linked {
                    self.input.forget_held();
                    return Ok(Some(DialpadEvent::LinkLost));
                }
                self.divert(hidpp, sleep)?;
                return Ok(Some(DialpadEvent::Rediverted));
            }
            self.pending.extend(self.input.feed(&frame));
            if let Some(event) = self.pending.pop_front() {
                return Ok(Some(DialpadEvent::Input(event)));
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
        }
    }

    /// How the Dialpad is reached.
    #[must_use]
    pub const fn link(&self) -> Link {
        self.link
    }

    /// Its device index: the receiver slot, or `0xff`.
    #[must_use]
    pub const fn device_index(&self) -> u8 {
        self.device_index
    }

    /// What every slot asked answered; empty for [`Link::Direct`].
    #[must_use]
    pub const fn slots(&self) -> &Slots {
        &self.slots
    }

    /// The divertable `0x1B04` controls, in CID order: buttons 1 to 4.
    #[must_use]
    pub fn buttons(&self) -> &[u16] {
        &self.buttons
    }

    /// The button number of `cid`, from 1, in CID order.
    #[must_use]
    pub fn button(&self, cid: u16) -> Option<usize> {
        self.buttons.iter().position(|&b| b == cid).map(|n| n + 1)
    }

    /// The rollers and their increments per rotation.
    #[must_use]
    pub fn rollers(&self) -> &[Roller] {
        &self.rollers
    }

    /// Index of `0x1B04`.
    #[must_use]
    pub const fn reprog_index(&self) -> u8 {
        self.reprog_index
    }

    /// Index of `0x4610`.
    #[must_use]
    pub const fn roller_index(&self) -> u8 {
        self.roller_index
    }
}

/// Asks every device index of `link` for `0x4610`, each waiting at most
/// `slot_probe` on a receiver, and checks a candidate's model id for
/// `BC00`; stops at the first Dialpad.
fn search<T: HidTransport>(
    hidpp: &mut Hidpp<T>,
    link: Link,
    slot_probe: Duration,
) -> Result<Slots, HidppError> {
    let (indices, timeout): (Vec<u8>, Duration) = match link {
        Link::Receiver => (SLOTS.collect(), slot_probe),
        Link::Slot(slot) => (vec![slot], slot_probe),
        Link::Direct => (vec![DEVICE_DIRECT], hidpp.reply_timeout()),
    };
    let mut slots = Vec::new();
    for index in indices {
        hidpp.set_device_index(index);
        let slot = probe_slot(hidpp, timeout)?;
        slots.push((index, slot));
        if slot == Slot::Dialpad {
            break;
        }
    }
    Ok(Slots(slots))
}

/// What the device index `hidpp` talks to answers.
fn probe_slot<T: HidTransport>(
    hidpp: &mut Hidpp<T>,
    timeout: Duration,
) -> Result<Slot, HidppError> {
    let found = match hidpp.get_feature_within(timeout, feature::MULTI_ROLLER) {
        Ok(found) => found,
        Err(error) => return unanswered(error),
    };
    if found.is_none() {
        return Ok(Slot::Absent);
    }
    match firmware::device_info(hidpp) {
        Ok(info) if info.carries(MX_CREATIVE_DIALPAD) => Ok(Slot::Dialpad),
        Ok(info) => Ok(Slot::Other {
            model_id: info.model_id,
        }),
        Err(error) => unanswered(error),
    }
}

/// The slot a request error stands for; the transport errors end the
/// search. The receiver errors the static pass names retryable (connect
/// fail, busy, resource error) leave the slot silent, like a timeout.
fn unanswered(error: HidppError) -> Result<Slot, HidppError> {
    match error {
        HidppError::Receiver {
            code: ReceiverError::ConnectFail | ReceiverError::Busy | ReceiverError::ResourceError,
            ..
        }
        | HidppError::Timeout { .. } => Ok(Slot::Silent),
        HidppError::Receiver { code, .. } => Ok(Slot::Empty(code)),
        HidppError::Device { .. } | HidppError::FeatureNotPresent { .. } => Ok(Slot::Absent),
        other => Err(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::ReportId;
    use crate::hidpp::ErrorCode;

    fn frame(bytes: &[u8]) -> Frame {
        let mut bytes = bytes.to_vec();
        bytes.resize(ReportId::from_byte(bytes[0]).unwrap().size(), 0);
        Frame::parse(&bytes).unwrap()
    }

    #[test]
    fn an_arrival_names_slot_wpid_and_link() {
        assert_eq!(
            arrival(&frame(&[0x10, 0x03, 0x41, 0x10, 0x40, 0x00, 0xbc])),
            Some(Arrival {
                slot: 3,
                wpid: 0xbc00,
                linked: false,
            })
        );
        let linked = arrival(&frame(&[0x10, 0x02, 0x41, 0x10, 0x02, 0x42, 0xb0]));
        assert_eq!(linked.map(|a| (a.wpid, a.linked)), Some((0xb042, true)));
        // Other short reports and long reports are no arrival.
        assert_eq!(arrival(&frame(&[0x10, 0x03, 0x40, 0x10, 0x00])), None);
        assert_eq!(arrival(&frame(&[0x11, 0x03, 0x41, 0x10, 0x00])), None);
    }

    #[test]
    fn request_errors_classify_the_slot() {
        let receiver = |code| HidppError::Receiver {
            device_index: 1,
            feature_index: 0,
            function: 0,
            code,
        };
        assert_eq!(
            unanswered(receiver(ReceiverError::UnknownDevice)),
            Ok(Slot::Empty(ReceiverError::UnknownDevice))
        );
        for retryable in [
            ReceiverError::ConnectFail,
            ReceiverError::Busy,
            ReceiverError::ResourceError,
        ] {
            assert_eq!(unanswered(receiver(retryable)), Ok(Slot::Silent));
        }
        assert_eq!(
            unanswered(receiver(ReceiverError::RequestUnavailable)),
            Ok(Slot::Empty(ReceiverError::RequestUnavailable))
        );
        assert_eq!(
            unanswered(HidppError::Device {
                feature_index: 0,
                function: 0,
                code: ErrorCode::InvalidArgument,
            }),
            Ok(Slot::Absent)
        );
        let lost = || HidppError::Transport(keyvo_core::hid::TransportError::Disconnected);
        assert_eq!(unanswered(lost()), Err(lost()));
    }

    #[test]
    fn slots_list_what_each_index_answered() {
        let slots = Slots(vec![
            (1, Slot::Empty(ReceiverError::UnknownDevice)),
            (2, Slot::Silent),
            (3, Slot::Absent),
            (
                4,
                Slot::Other {
                    model_id: [0xb0, 0x42, 0, 0, 0, 0],
                },
            ),
            (5, Slot::Dialpad),
        ]);
        assert_eq!(
            slots.to_string(),
            "1 empty, 2 silent, 3 no 0x4610, 4 model b0 42 00 00 00 00, 5 dialpad"
        );
        assert_eq!(slots.silent(), [2]);
        assert_eq!(
            Slots(vec![(DEVICE_DIRECT, Slot::Absent)]).to_string(),
            "direct no 0x4610"
        );
        assert_eq!(
            DialpadError::NotFound(Slots(vec![(1, Slot::Silent)])).to_string(),
            "no MX Creative Dialpad found (slots: 1 silent)"
        );
    }
}
