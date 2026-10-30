//! The Dialpad's controls (spec §5.8, §7.2): the dial, the roller and the
//! four buttons, how a turn becomes a number of action runs, and the
//! records of a turn and a button press.
//!
//! A detent is one increment of a roller as the device reports it: the dial
//! has 180 per rotation, the roller 40, neither has a ratchet (#187). A
//! turn's delta counts as that many detents; the binding's `step` detents
//! make one run of its action, the rest carries to the next turn in the
//! same direction, and `[dialpad].max_per_event` caps the runs of one
//! report, so a flick cannot queue a hundred chords.

use std::collections::BTreeMap;
use std::fmt;
use std::time::SystemTime;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::{self, Deserializer, Unexpected, Visitor};
use serde::{Deserialize, Serialize, Serializer};

use crate::layout::Profiles;
use crate::press::PressKind;
use crate::profile::{DialButton, Profile, Rotary};
use crate::stack::{Layer, Stack};
use crate::state::ModifierKey;
use crate::trace::{Outcome, TraceRecord, Trigger};

/// One of the Dialpad's two rollers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Roller {
    /// The large dial, roller id 1 (180 increments per rotation).
    Dial,
    /// The small roller, roller id 0 (40 increments per rotation).
    Roller,
}

impl Roller {
    /// The roller of the device's roller id (spec §5.8: 0 the small
    /// roller, 1 the dial); `None` for another id.
    #[must_use]
    pub const fn from_id(id: u8) -> Option<Self> {
        match id {
            0 => Some(Self::Roller),
            1 => Some(Self::Dial),
            _ => None,
        }
    }

    /// The name in the profile, the socket and the trace.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dial => "dial",
            Self::Roller => "roller",
        }
    }
}

/// The number of a Dialpad button, 1 to 4 in CID order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ButtonSlot(u8);

/// The Dialpad's button count.
pub const BUTTONS: u8 = 4;

impl ButtonSlot {
    /// The button `n`, or `None` outside 1..=4.
    #[must_use]
    pub fn new(n: u8) -> Option<Self> {
        (1..=BUTTONS).contains(&n).then_some(Self(n))
    }

    /// The button number, 1 to 4.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl<'de> Deserialize<'de> for ButtonSlot {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SlotVisitor;

        impl Visitor<'_> for SlotVisitor {
            type Value = ButtonSlot;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a Dialpad button from 1 to 4")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<ButtonSlot, E> {
                let mut bytes = v.bytes();
                match (bytes.next(), bytes.next()) {
                    (Some(b @ b'1'..=b'4'), None) => Ok(ButtonSlot(b - b'0')),
                    _ => Err(E::invalid_value(Unexpected::Str(v), &self)),
                }
            }
        }

        deserializer.deserialize_str(SlotVisitor)
    }
}

/// A control of the Dialpad: what a trace record, a `key` event and a
/// socket `press` name, written `dial`, `roller` or `button:N`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Control {
    /// The dial.
    Dial,
    /// The roller.
    Roller,
    /// A button.
    Button(ButtonSlot),
}

impl Control {
    /// The roller this control is, if it is one.
    #[must_use]
    pub const fn roller(self) -> Option<Roller> {
        match self {
            Self::Dial => Some(Roller::Dial),
            Self::Roller => Some(Roller::Roller),
            Self::Button(_) => None,
        }
    }
}

impl From<Roller> for Control {
    fn from(roller: Roller) -> Self {
        match roller {
            Roller::Dial => Self::Dial,
            Roller::Roller => Self::Roller,
        }
    }
}

impl fmt::Display for Control {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dial => f.write_str("dial"),
            Self::Roller => f.write_str("roller"),
            Self::Button(slot) => write!(f, "button:{}", slot.get()),
        }
    }
}

impl std::str::FromStr for Control {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "dial" => Ok(Self::Dial),
            "roller" => Ok(Self::Roller),
            _ => s
                .strip_prefix("button:")
                .and_then(|n| n.parse().ok())
                .and_then(ButtonSlot::new)
                .map(Self::Button)
                .ok_or_else(|| {
                    format!("unknown control `{s}`, expected `dial`, `roller` or `button:1` to `button:4`")
                }),
        }
    }
}

impl Serialize for Control {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Control {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(de::Error::custom)
    }
}

impl JsonSchema for Control {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Control".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "pattern": "^(dial|roller|button:[1-4])$"
        })
    }
}

/// Which way a roller turned: clockwise is a positive delta (#187; the
/// roller's forward turn).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Clockwise, `cw`.
    Cw,
    /// Counter-clockwise, `ccw`.
    Ccw,
}

impl Direction {
    /// The direction of `delta`; `None` for 0.
    #[must_use]
    pub const fn of(delta: i32) -> Option<Self> {
        if delta > 0 {
            Some(Self::Cw)
        } else if delta < 0 {
            Some(Self::Ccw)
        } else {
            None
        }
    }
}

/// The runs of one turn of `delta` detents with `step` detents per run and
/// `carry` detents left from the turns before in the same direction, at
/// most `max`; and the detents left to carry. A turn the other way starts
/// from nothing.
#[must_use]
pub fn runs(delta: i32, step: u32, carry: i32, max: u32) -> (u32, i32) {
    let step = step.max(1);
    let carry = if carry.signum() == delta.signum() {
        carry
    } else {
        0
    };
    let total = carry.saturating_add(delta);
    let detents = total.unsigned_abs();
    let runs = detents / step;
    let left = i32::try_from(detents % step).unwrap_or(0) * total.signum();
    (runs.min(max), left)
}

/// The detents each roller carries to its next turn.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Turns(BTreeMap<Roller, i32>);

impl Turns {
    /// The runs of a turn of `roller` by `delta` with `step` detents per
    /// run, at most `max`; the rest carries.
    pub fn turn(&mut self, roller: Roller, delta: i32, step: u32, max: u32) -> u32 {
        let carry = self.0.get(&roller).copied().unwrap_or(0);
        let (runs, left) = runs(delta, step, carry, max);
        self.0.insert(roller, left);
        runs
    }

    /// Forgets every carry, as when another binding takes over.
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

/// The Dialpad bindings of one profile over the global ones: a control the
/// profile's `[dialpad]` table leaves out comes from the global profile's
/// (spec §7.2), as a key the app profile lacks comes from the global
/// context page.
#[derive(Debug, Clone, Copy)]
pub struct Bindings<'a> {
    profile: &'a Profile,
    global: Option<&'a Profile>,
}

impl<'a> Bindings<'a> {
    /// The bindings of `profile` over the global profile of `profiles`.
    #[must_use]
    pub fn new(profiles: &'a Profiles, profile: &'a Profile) -> Self {
        let global = (Layer::of(profile) == Layer::App).then(|| profiles.global());
        Self { profile, global }
    }

    /// The profile on top.
    #[must_use]
    pub const fn profile(&self) -> &'a Profile {
        self.profile
    }

    fn layers(&self) -> impl Iterator<Item = &'a Profile> {
        std::iter::once(self.profile).chain(self.global)
    }

    /// The binding of `roller` and the layer it comes from; `None` when no
    /// layer defines it. A roller the profile defines hides the global one,
    /// even without `cw` or `ccw`.
    #[must_use]
    pub fn rotary(&self, roller: Roller) -> Option<(&'a Rotary, Layer)> {
        self.layers().find_map(|profile| {
            let dialpad = profile.dialpad.as_ref()?;
            let rotary = match roller {
                Roller::Dial => dialpad.dial.as_ref(),
                Roller::Roller => dialpad.roller.as_ref(),
            }?;
            Some((rotary, Layer::of(profile)))
        })
    }

    /// Button `slot` and the layer it comes from, like [`Self::rotary`].
    #[must_use]
    pub fn button(&self, slot: ButtonSlot) -> Option<(&'a DialButton, Layer)> {
        self.layers().find_map(|profile| {
            let button = profile.dialpad.as_ref()?.buttons.get(&slot)?;
            Some((button, Layer::of(profile)))
        })
    }

    /// Whether any layer has a `[dialpad]` table.
    #[must_use]
    pub fn any(&self) -> bool {
        self.layers().any(|profile| profile.dialpad.is_some())
    }
}

/// What a turn resolved to: the record of one run and how many runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    /// The record of each run: the action of the turn's direction and the
    /// layer it came from; [`Outcome::Unbound`] without one,
    /// [`Outcome::DryRun`] for a simulated turn, [`Outcome::Pending`] until
    /// the daemon has run it.
    pub record: TraceRecord,
    /// How often the action runs; 0 when the turn stays below `step` or
    /// nothing is bound.
    pub runs: u32,
}

/// Resolves a turn of `roller` by `delta` on `bindings`, the page `page`
/// shown, at `at`. `turns` carries the detents below one step; a simulated
/// turn neither reads nor changes it.
#[must_use]
pub fn turn(
    bindings: &Bindings<'_>,
    page: usize,
    roller: Roller,
    delta: i32,
    turn: (Trigger, SystemTime),
    turns: &mut Turns,
    max: u32,
) -> Turn {
    let (trigger, at) = turn;
    let record = record(
        &bindings.profile().profile.id,
        page,
        roller.into(),
        trigger,
        at,
    );
    resolve(record, bindings.rotary(roller), (roller, delta), turns, max)
}

/// Resolves a turn of `roller` by `delta` on the binding of the modifier
/// key `taker` (spec §7.2), like [`turn`]: the record names the key, the
/// control, the mode and the layer that supplied the key, on the page the
/// key was pressed on. Nothing is bound when the key no longer resolves or
/// has no table for `roller`.
#[must_use]
pub fn modifier_turn(
    profiles: &Profiles,
    taker: &ModifierKey,
    roller: Roller,
    delta: i32,
    turn: (Trigger, SystemTime),
    turns: &mut Turns,
    max: u32,
) -> Turn {
    let (trigger, at) = turn;
    let mut record = record(&taker.profile, taker.page, roller.into(), trigger, at);
    record.key = Some(taker.key);
    record.modifier = Some(taker.mode);
    let found = profiles
        .get(&taker.profile)
        .and_then(|profile| Stack::new(profiles, profile, taker.page).ok())
        .and_then(|stack| stack.key(taker.key))
        .and_then(|(key, layer)| Some((key.rotary(roller)?, layer)));
    resolve(record, found, (roller, delta), turns, max)
}

/// Fills `record` with the binding `found` of a turn of `roller` by
/// `delta`: the action of its direction, how often it runs.
fn resolve(
    mut record: TraceRecord,
    found: Option<(&Rotary, Layer)>,
    (roller, delta): (Roller, i32),
    turns: &mut Turns,
    max: u32,
) -> Turn {
    let trigger = record.trigger;
    record.layer = found.map(|(_, layer)| layer);
    let action = found.and_then(|(rotary, _)| match Direction::of(delta)? {
        Direction::Cw => rotary.cw.clone(),
        Direction::Ccw => rotary.ccw.clone(),
    });
    let step = found.map_or(1, |(rotary, _)| rotary.step.get());
    let runs = match (&action, trigger.simulated) {
        (None, _) => 0,
        (Some(_), true) => runs(delta, step, 0, max).0,
        (Some(_), false) => turns.turn(roller, delta, step, max),
    };
    record.result = match (&action, trigger.simulated) {
        (None, _) => Outcome::Unbound,
        (Some(_), true) => Outcome::DryRun,
        (Some(_), false) => Outcome::Pending,
    };
    record.action = action;
    Turn { record, runs }
}

/// Resolves a press of button `slot` on `bindings` like a key's
/// ([`crate::press::resolve`]): the button's action, else
/// [`Outcome::Unbound`].
#[must_use]
pub fn press(
    bindings: &Bindings<'_>,
    page: usize,
    slot: ButtonSlot,
    trigger: Trigger,
    at: SystemTime,
) -> TraceRecord {
    let found = bindings.button(slot);
    let mut record = record(
        &bindings.profile().profile.id,
        page,
        Control::Button(slot),
        trigger,
        at,
    );
    record.layer = found.map(|(_, layer)| layer);
    record.action = found.and_then(|(button, _)| button.action.clone());
    record.result = match (&record.action, trigger.simulated) {
        (None, _) => Outcome::Unbound,
        (Some(_), true) => Outcome::DryRun,
        (Some(_), false) => Outcome::Pending,
    };
    record
}

/// The record of `control` on the page `page` of `profile`, nothing
/// resolved: no layer, no action, [`Outcome::Unbound`].
#[must_use]
pub fn record(
    profile: &str,
    page: usize,
    control: Control,
    trigger: Trigger,
    at: SystemTime,
) -> TraceRecord {
    TraceRecord {
        control: Some(control),
        ..crate::press::nothing(profile, page, trigger, at)
    }
}

/// The record of a turn or button press under the lock: nothing resolved,
/// [`Outcome::Blocked`] for `reason`.
#[must_use]
pub fn blocked(
    profile: &str,
    page: usize,
    control: Control,
    trigger: Trigger,
    at: SystemTime,
    reason: &str,
) -> TraceRecord {
    let mut record = record(profile, page, control, trigger, at);
    record.result = Outcome::Blocked {
        reason: reason.to_owned(),
    };
    record
}

/// A turn is never a long press: its records carry `short`.
#[must_use]
pub const fn turn_trigger(simulated: bool) -> Trigger {
    Trigger {
        press: PressKind::Short,
        simulated,
    }
}
