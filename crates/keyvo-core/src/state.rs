//! The context state machine (spec §6.1, §6.3): which profile and page the
//! pad shows, from the focused window and the two page buttons.
//!
//! - The page list: for the shown profile the pad walks one ordered list.
//!   Index 0 is the global profile's page named `general`, the global pages
//!   after it in file order take -1, -2, ...; index 1 is the shown
//!   profile's context page, its other pages follow from 2 in file order.
//!   A short press of the left button shows index - 1, of the right one
//!   index + 1; the list does not wrap.
//! - `auto` mode: the focused window selects the profile
//!   ([`Profiles::select`]) once its report has stood for
//!   `[timing].focus_hysteresis`; an app switch (another window class)
//!   shows index 1 of the new profile. A title change in the same app keeps
//!   the index.
//! - Pending launches: a `launch` key ([`State::launched`]) expects its
//!   window for `[timing].launch_focus`. An app switch in auto mode without
//!   the lock is judged against them, the first hit consuming its entry:
//!   the window's process descends from the launch ([`Lineage`]), else a
//!   launch of the window's class, else the oldest launch without a class.
//!   A hit keeps an index of 0 or below (a global page exists under every
//!   profile) and starts the page return over; otherwise, or for an index
//!   of 2 or above, the switch shows index 1.
//! - The page return: in auto mode, an index of 0 or below shows index 1
//!   again after `[timing].page_return_global` without a page-button press
//!   or release or a key press, an index of 2 or above after
//!   `[timing].page_return_own`; 0 turns a side off.
//! - `manual` mode is a focus lock: the shown profile and index stay
//!   whatever the focus does, the buttons walk the same list. Leaving it
//!   shows index 1 of the profile the focused window selects.
//! - A page button held for `[timing].long_press` toggles the mode; both
//!   held together escape to the global profile's context page in auto
//!   mode.
//! - A window whose class is in `[focus].hold_classes` (the keyvo app)
//!   holds the context: its report cancels a pending one and the window
//!   applied before stays, so the profile and page stay too.
//! - A locked session ([`Session`]) is a veil over the state, not a mode:
//!   the view keeps its profile, index and mode and says `locked`; the page
//!   buttons do nothing; focus reports and the page return apply as
//!   without the lock, so the unlock shows what the state shows anyway.
//!
//! Synchronous and clock-free: every call takes `now`, and the caller
//! wakes the machine with [`State::poll`] at [`State::next_deadline`].

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::config::{Config, Focus};
use crate::dialpad::Roller;
use crate::focus::WindowContext;
use crate::layout::Profiles;
use crate::lineage::Lineage;
use crate::press::{PressDetector, PressKind};
use crate::profile::{GLOBAL_ID, KeySlot, ModifierMode, Profile, class_matches};
use crate::session::Session;
use crate::stack::Stack;

/// Names one pending launch ([`State::launched`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LaunchId(u64);

/// A `launch` whose window may still come.
#[derive(Debug, Clone)]
struct PendingLaunch {
    id: LaunchId,
    /// The window class the launch opens, when known.
    class: Option<String>,
    /// The process id of the launch's shell, once it started.
    pid: Option<u32>,
    /// The press plus `[timing].launch_focus`.
    expires: Instant,
}

/// A reported window waiting out the focus hysteresis.
#[derive(Debug, Clone)]
struct Pending {
    window: WindowContext,
    /// When it applies.
    at: Instant,
    /// The launches its process descends from, as the report found them.
    kin: Vec<LaunchId>,
}

/// Who chooses the profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// The focused window chooses.
    Auto,
    /// The profile is locked; focus changes are recorded, not shown.
    Manual,
}

impl Mode {
    /// The name in the spec and the socket protocol.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Manual => "manual",
        }
    }
}

/// One of the Keypad's two page buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PageButton {
    /// The left button (CID `0x01A1`).
    Left,
    /// The right button (CID `0x01A2`).
    Right,
}

/// What the pad shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct View {
    /// The id of the profile whose page is painted: the global one at index
    /// 0 and below.
    pub profile: String,
    /// The page, counted from 1 inside `profile`.
    pub page: usize,
    /// The mode.
    pub mode: Mode,
    /// The position in the shown profile's page list: 1 its context page,
    /// 0 the global `general` page.
    pub index: isize,
    /// The session is locked: the pad shows the lock page over this view
    /// and blocks every press. Read as `false` when absent, as from a
    /// daemon before the session lock.
    #[serde(default)]
    pub locked: bool,
    /// The key that has the dial and the roller, held or toggled; absent
    /// when none has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifier: Option<ActiveModifier>,
}

/// The key that has the dial and the roller (spec §6.1): a held modifier
/// key, the last pressed of several, else the armed toggle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveModifier {
    /// The key, 1 to 9, on the page the view shows.
    #[serde(deserialize_with = "crate::profile::slot_number")]
    pub key: KeySlot,
    /// Held or toggled.
    pub mode: ModifierMode,
}

/// The active modifier and where its key is: the page of the profile it
/// was pressed on, which resolves it over the global layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModifierKey {
    /// The key.
    pub key: KeySlot,
    /// Held or toggled.
    pub mode: ModifierMode,
    /// The id of the profile shown at the press.
    pub profile: String,
    /// The page shown at the press, counted from 1.
    pub page: usize,
}

impl ModifierKey {
    /// The key and the mode.
    #[must_use]
    pub const fn active(&self) -> ActiveModifier {
        ActiveModifier {
            key: self.key,
            mode: self.mode,
        }
    }

    /// Whether it is still a modifier key of `mode` in `profiles`.
    fn resolves(&self, profiles: &Profiles) -> bool {
        modifier_of(profiles, &self.profile, self.page, self.key) == Some(self.mode)
    }
}

/// What a key going down is ([`State::key_down`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyDown {
    /// An ordinary key or a toggle key: its release or its long press makes
    /// a press ([`State::key_press`]).
    Press,
    /// A held modifier key: it has no long press, its release tells
    /// whether its action runs ([`State::key_up`]).
    Held,
}

/// What a press of a key does ([`State::key_press`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPress {
    /// The key's action runs, as of an ordinary key.
    Run,
    /// A short press of a toggle key: the toggle is on (`true`) or off now,
    /// or would be for a simulated press; no action runs.
    Toggle(bool),
}

/// The modifier mode of key `slot` on page `page` of the profile `profile`
/// over the global layer; `None` for an ordinary key or none at all.
fn modifier_of(
    profiles: &Profiles,
    profile: &str,
    page: usize,
    slot: KeySlot,
) -> Option<ModifierMode> {
    let profile = profiles.get(profile)?;
    Stack::new(profiles, profile, page)
        .ok()?
        .key(slot)?
        .0
        .modifier_mode()
}

/// A held modifier key.
#[derive(Debug, Clone)]
struct Holding {
    at: ModifierKey,
    /// A turn took its binding or another's during the hold.
    used: bool,
}

/// The armed toggle.
#[derive(Debug, Clone)]
struct Armed {
    at: ModifierKey,
    /// The arm or the last turn, for `[timing].modifier_timeout`.
    since: Instant,
}

/// The mode and page state of the pad.
#[derive(Debug, Clone)]
pub struct State {
    hysteresis: Duration,
    page_return_global: Duration,
    page_return_own: Duration,
    long_press: Duration,
    launch_focus: Duration,
    mode: Mode,
    /// The profile the focused window selects.
    selected: String,
    /// The profile whose page list the pad walks: the selected one in auto
    /// mode, the locked one in manual mode.
    shown: String,
    /// The position in the shown profile's page list.
    index: isize,
    /// The focused window as last applied.
    window: Option<WindowContext>,
    /// A newer window and when it applies.
    pending: Option<Pending>,
    /// The launches whose windows may still come, oldest first.
    launches: Vec<PendingLaunch>,
    /// The id of the next launch.
    next_launch: u64,
    /// The classes that hold the context.
    hold: Focus,
    /// The focused window while it holds the context.
    in_front: Option<WindowContext>,
    /// The last page-button press or release or key press.
    last_use: Option<Instant>,
    held: BTreeSet<PageButton>,
    buttons: PressDetector<PageButton>,
    /// The session is locked.
    locked: bool,
    modifier_timeout: Duration,
    /// The held modifier keys, the first pressed first.
    holding: Vec<Holding>,
    /// The armed toggle.
    armed: Option<Armed>,
    /// The armed toggle key went down again and ended it: its short
    /// release does not arm it again.
    disarming: Option<KeySlot>,
}

impl State {
    /// Auto mode on the global profile's context page, before any focus
    /// report, with the `[timing]` and `[focus]` of `config`.
    #[must_use]
    pub fn new(config: &Config, _profiles: &Profiles) -> Self {
        let timing = &config.timing;
        Self {
            hysteresis: timing.focus_hysteresis,
            page_return_global: timing.page_return_global,
            page_return_own: timing.page_return_own,
            long_press: timing.long_press,
            launch_focus: timing.launch_focus,
            mode: Mode::Auto,
            selected: GLOBAL_ID.to_owned(),
            shown: GLOBAL_ID.to_owned(),
            index: 1,
            window: None,
            pending: None,
            launches: Vec::new(),
            next_launch: 0,
            hold: config.focus.clone(),
            in_front: None,
            last_use: None,
            held: BTreeSet::new(),
            buttons: PressDetector::new(timing.long_press),
            locked: false,
            modifier_timeout: timing.modifier_timeout,
            holding: Vec::new(),
            armed: None,
            disarming: None,
        }
    }

    /// The mode.
    #[must_use]
    pub const fn mode(&self) -> Mode {
        self.mode
    }

    /// What the pad shows with `profiles`. A profile that no longer exists
    /// shows as the global one.
    #[must_use]
    pub fn view(&self, profiles: &Profiles) -> View {
        let shown = self.shown(profiles);
        let list = PageList::of(profiles, shown);
        let index = list.clamp(self.index);
        let (profile, page) = list.get(index).unwrap_or((shown, 1));
        View {
            profile: profile.profile.id.clone(),
            page,
            mode: self.mode,
            index,
            locked: self.locked,
            modifier: self.modifier().map(ModifierKey::active),
        }
    }

    /// The key that has the dial and the roller: the last pressed held
    /// modifier key, else the armed toggle.
    #[must_use]
    pub fn modifier(&self) -> Option<&ModifierKey> {
        self.holding
            .last()
            .map(|holding| &holding.at)
            .or_else(|| self.armed.as_ref().map(|armed| &armed.at))
    }

    /// Every held modifier key, the first pressed first, then the armed
    /// toggle.
    pub fn modifier_keys(&self) -> impl Iterator<Item = &ModifierKey> {
        self.holding
            .iter()
            .map(|holding| &holding.at)
            .chain(self.armed.as_ref().map(|armed| &armed.at))
    }

    /// The modifier key at `slot` of the page shown, if it is one.
    fn modifier_key(&self, slot: KeySlot, profiles: &Profiles) -> Option<ModifierKey> {
        let view = self.view(profiles);
        let mode = modifier_of(profiles, &view.profile, view.page, slot)?;
        Some(ModifierKey {
            key: slot,
            mode,
            profile: view.profile,
            page: view.page,
        })
    }

    /// Ends the armed toggle at `now`, which starts the page return over.
    fn disarm(&mut self, now: Instant) {
        if self.armed.take().is_some() {
            self.last_use = Some(now);
        }
    }

    /// Key `slot` went down at `now`: the page return starts over and the
    /// armed toggle ends, this press otherwise acting as usual; the armed
    /// key's own press only ends it. A held modifier key takes the dial and
    /// the roller until its release. Nothing changes while the session is
    /// locked.
    pub fn key_down(&mut self, slot: KeySlot, now: Instant, profiles: &Profiles) -> KeyDown {
        if self.locked {
            return KeyDown::Press;
        }
        self.touch(now);
        if let Some(armed) = &self.armed {
            if armed.at.key == slot {
                self.disarming = Some(slot);
            }
            self.disarm(now);
        }
        match self.modifier_key(slot, profiles) {
            Some(at) if at.mode == ModifierMode::Held => {
                self.holding.retain(|holding| holding.at.key != slot);
                self.holding.push(Holding { at, used: false });
                KeyDown::Held
            }
            _ => KeyDown::Press,
        }
    }

    /// Key `slot` came up at `now`: for a held modifier key whether its
    /// action runs (no turn took a binding during the hold); `None` for any
    /// other key, whose press the caller's detector makes.
    pub fn key_up(&mut self, slot: KeySlot, now: Instant) -> Option<bool> {
        let at = self
            .holding
            .iter()
            .position(|holding| holding.at.key == slot)?;
        let holding = self.holding.remove(at);
        self.last_use = Some(now);
        Some(!holding.used)
    }

    /// A press of key `slot` of `kind` at `now`, from the device or a
    /// client: a short press of a toggle key arms it, or ends it when it is
    /// armed; any other press ends the armed toggle of another key and runs
    /// as usual, a toggle key's long press its action. A `simulated` press
    /// changes nothing and tells what it would do.
    pub fn key_press(
        &mut self,
        slot: KeySlot,
        kind: PressKind,
        simulated: bool,
        now: Instant,
        profiles: &Profiles,
    ) -> KeyPress {
        if self.locked {
            return KeyPress::Run;
        }
        let at = self
            .modifier_key(slot, profiles)
            .filter(|at| at.mode == ModifierMode::Toggle);
        let armed = self
            .armed
            .as_ref()
            .is_some_and(|armed| armed.at.key == slot);
        if simulated {
            return match (at, kind) {
                (Some(_), PressKind::Short) => KeyPress::Toggle(!armed),
                _ => KeyPress::Run,
            };
        }
        let disarming = self.disarming == Some(slot);
        if disarming {
            self.disarming = None;
        }
        if let (Some(at), PressKind::Short) = (at, kind)
            && !disarming
            && !armed
        {
            self.armed = Some(Armed { at, since: now });
            return KeyPress::Toggle(true);
        }
        self.disarm(now);
        if kind == PressKind::Short && (disarming || armed) {
            KeyPress::Toggle(false)
        } else {
            KeyPress::Run
        }
    }

    /// A button of the Dialpad was pressed at `now`: the armed toggle ends.
    pub fn button(&mut self, now: Instant) {
        self.disarm(now);
    }

    /// A turn of `roller` at `now`: the active modifier key, when it binds
    /// `roller`, else the held one before it that does; `None` leaves the
    /// turn to the profile's `[dialpad]` table. A turn taken marks the
    /// held keys used and starts `[timing].modifier_timeout` over, unless
    /// it is `simulated`.
    pub fn modifier_turn(
        &mut self,
        roller: Roller,
        simulated: bool,
        now: Instant,
        profiles: &Profiles,
    ) -> Option<ModifierKey> {
        let binds = |at: &ModifierKey| {
            profiles
                .get(&at.profile)
                .and_then(|profile| Stack::new(profiles, profile, at.page).ok())
                .and_then(|stack| stack.key(at.key))
                .and_then(|(key, _)| key.rotary(roller))
                .is_some()
        };
        let taker = self
            .holding
            .iter()
            .rev()
            .map(|holding| &holding.at)
            .chain(self.armed.as_ref().map(|armed| &armed.at))
            .find(|at| binds(at))
            .cloned()?;
        if !simulated {
            for holding in &mut self.holding {
                holding.used = true;
            }
            if let Some(armed) = &mut self.armed {
                armed.since = now;
            }
        }
        Some(taker)
    }

    /// Ends the toggle when the page shown is no longer the one it was
    /// armed on, at `now`.
    fn settle(&mut self, now: Instant, profiles: &Profiles) {
        if self.off_page(profiles) {
            self.disarm(now);
        }
    }

    /// Whether the armed toggle's page no longer shows.
    fn off_page(&self, profiles: &Profiles) -> bool {
        let view = self.view(profiles);
        self.armed
            .as_ref()
            .is_some_and(|armed| armed.at.profile != view.profile || armed.at.page != view.page)
    }

    /// When the armed toggle ends without a turn.
    fn timeout_at(&self) -> Option<Instant> {
        if self.modifier_timeout.is_zero() {
            return None;
        }
        self.armed
            .as_ref()
            .map(|armed| armed.since + self.modifier_timeout)
    }

    /// Whether the session is locked.
    #[must_use]
    pub const fn locked(&self) -> bool {
        self.locked
    }

    /// The session source reports `session`; `true` when the lock state
    /// changed. A page button held at the lock or the unlock is forgotten,
    /// so neither its long press nor its release acts.
    pub fn session(&mut self, session: Session) -> bool {
        let locked = session.is_locked();
        if locked == self.locked {
            return false;
        }
        self.locked = locked;
        self.held.clear();
        self.buttons = PressDetector::new(self.long_press);
        // The lock ends every modifier; a key held through it is forgotten.
        self.holding.clear();
        self.armed = None;
        self.disarming = None;
        true
    }

    /// Whether a window of `class` holds the context.
    #[must_use]
    pub fn holds(&self, class: &str) -> bool {
        self.hold.holds(class)
    }

    /// The focused window, while it is one that holds the context.
    #[must_use]
    pub const fn held(&self) -> Option<&WindowContext> {
        self.in_front.as_ref()
    }

    /// The focus source reports `window` at `now`. It applies once no other
    /// report came for `[timing].focus_hysteresis`; a report of the window
    /// already applied cancels a pending one, and so does a report of a
    /// window that holds the context, which never applies. `lineage` tells
    /// the pending launches the window's process descends from.
    pub fn focus(
        &mut self,
        window: WindowContext,
        now: Instant,
        profiles: &Profiles,
        lineage: &dyn Lineage,
    ) {
        if self.holds(&window.class) {
            self.pending = None;
            self.in_front = Some(window);
        } else {
            self.in_front = None;
            if self.window.as_ref() == Some(&window) {
                self.pending = None;
            } else {
                let kin = window.pid.map_or_else(Vec::new, |pid| {
                    self.launches
                        .iter()
                        .filter(|launch| {
                            launch.pid.is_some_and(|shell| lineage.descends(pid, shell))
                        })
                        .map(|launch| launch.id)
                        .collect()
                });
                self.pending = Some(Pending {
                    window,
                    at: now + self.hysteresis,
                    kin,
                });
            }
        }
        self.poll(now, profiles);
    }

    /// A `launch` key whose window carries `class`, when known, was pressed
    /// at `now`: its window is expected until `[timing].launch_focus` from
    /// now. `None` when that is 0, so no window is expected.
    pub fn launched(&mut self, class: Option<String>, now: Instant) -> Option<LaunchId> {
        if self.launch_focus.is_zero() {
            return None;
        }
        let id = LaunchId(self.next_launch);
        self.next_launch += 1;
        self.launches.push(PendingLaunch {
            id,
            class,
            pid: None,
            expires: now + self.launch_focus,
        });
        Some(id)
    }

    /// The shell of launch `id` runs as process `pid`.
    pub fn launch_started(&mut self, id: LaunchId, pid: u32) {
        if let Some(launch) = self.launches.iter_mut().find(|launch| launch.id == id) {
            launch.pid = Some(pid);
        }
    }

    /// Launch `id` did not start or exited with an error: no window of it
    /// comes.
    pub fn launch_failed(&mut self, id: LaunchId) {
        self.launches.retain(|launch| launch.id != id);
    }

    /// The pending launches, oldest first.
    #[must_use]
    pub fn launches(&self) -> Vec<LaunchId> {
        self.launches.iter().map(|launch| launch.id).collect()
    }

    /// `button` went down at `now`. The second button held escapes. Nothing
    /// while the session is locked.
    pub fn press(&mut self, button: PageButton, now: Instant, profiles: &Profiles) {
        if self.locked {
            return;
        }
        self.last_use = Some(now);
        self.held.insert(button);
        if self.held.len() > 1 {
            self.escape();
        } else {
            self.buttons.press(button, now);
        }
        self.settle(now, profiles);
    }

    /// `button` came up at `now`: a short press pages, a long one not yet
    /// fired toggles the mode. Nothing while the session is locked.
    pub fn release(&mut self, button: PageButton, now: Instant, profiles: &Profiles) {
        if self.locked {
            return;
        }
        self.last_use = Some(now);
        self.held.remove(&button);
        match self.buttons.release(button, now) {
            Some(PressKind::Short) => self.step(button, profiles),
            Some(PressKind::Long) => self.toggle_mode(),
            None => {}
        }
        self.settle(now, profiles);
    }

    /// A key on the pad went down at `now`: the page return starts over.
    pub const fn touch(&mut self, now: Instant) {
        self.last_use = Some(now);
    }

    /// Applies what is due at `now`: the page return, the end of the
    /// launches past `[timing].launch_focus`, a pending window, a long
    /// press.
    pub fn poll(&mut self, now: Instant, profiles: &Profiles) {
        if self.return_at().is_some_and(|at| at <= now) {
            self.index = 1;
        }
        self.launches.retain(|launch| launch.expires > now);
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.at <= now)
            && let Some(pending) = self.pending.take()
        {
            self.apply(pending, now, profiles);
        }
        for _ in self.buttons.poll(now) {
            self.toggle_mode();
        }
        if self.timeout_at().is_some_and(|at| at <= now) {
            self.disarm(now);
        }
        self.settle(now, profiles);
    }

    /// When [`State::poll`] has something to do next.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        let focus = self.pending.as_ref().map(|pending| pending.at);
        [
            focus,
            self.buttons.next_deadline(),
            self.return_at(),
            self.timeout_at(),
        ]
        .into_iter()
        .flatten()
        .min()
    }

    /// The configuration and profiles were reloaded: the page return times
    /// and `launch_focus` of `config` apply to what comes next, the
    /// pending launches stay; the focused window (while one that holds the
    /// context is in front, the one before it) selects again, as its rules
    /// may have changed; manual mode keeps its profile unless it is gone
    /// (then the global context page). An index the list no longer has
    /// becomes the nearest end.
    pub fn reload(&mut self, config: &Config, profiles: &Profiles) {
        self.page_return_global = config.timing.page_return_global;
        self.page_return_own = config.timing.page_return_own;
        self.launch_focus = config.timing.launch_focus;
        self.modifier_timeout = config.timing.modifier_timeout;
        self.selected = self.window.as_ref().map_or_else(
            || GLOBAL_ID.to_owned(),
            |window| profiles.select(window).profile.id.clone(),
        );
        match self.mode {
            Mode::Auto => self.shown.clone_from(&self.selected),
            Mode::Manual => {
                if profiles.get(&self.shown).is_none() {
                    GLOBAL_ID.clone_into(&mut self.shown);
                    self.index = 1;
                }
            }
        }
        self.index = PageList::of(profiles, self.shown(profiles)).clamp(self.index);
        // A modifier key the reload dropped or changed ends, and so does a
        // toggle whose page no longer shows.
        self.holding.retain(|holding| holding.at.resolves(profiles));
        if self
            .armed
            .as_ref()
            .is_some_and(|armed| !armed.at.resolves(profiles))
            || self.off_page(profiles)
        {
            self.armed = None;
        }
    }

    /// The shown profile in `profiles`, the global one when it is gone.
    fn shown<'a>(&self, profiles: &'a Profiles) -> &'a Profile {
        profiles
            .get(&self.shown)
            .unwrap_or_else(|| profiles.global())
    }

    /// When the page return is due: auto mode on a page other than the
    /// context page, with the return time of its side set.
    fn return_at(&self) -> Option<Instant> {
        let after = match self.index {
            1 => return None,
            index if index < 1 => self.page_return_global,
            _ => self.page_return_own,
        };
        // A held or armed modifier is pad activity.
        if self.mode != Mode::Auto || after.is_zero() || self.modifier().is_some() {
            return None;
        }
        self.last_use.map(|used| used + after)
    }

    /// The window of `pending` applies at `now`. An app switch in auto mode
    /// shows index 1, unless the window is a launch's and the index is 0
    /// or below.
    fn apply(&mut self, pending: Pending, now: Instant, profiles: &Profiles) {
        let Pending { window, kin, .. } = pending;
        let app_switch = self
            .window
            .as_ref()
            .is_none_or(|old| old.class != window.class);
        profiles
            .select(&window)
            .profile
            .id
            .clone_into(&mut self.selected);
        let launched = app_switch
            && self.mode == Mode::Auto
            && !self.locked
            && self.claim(&window.class, &kin);
        self.window = Some(window);
        if self.mode == Mode::Auto {
            self.shown.clone_from(&self.selected);
            let list = PageList::of(profiles, self.shown(profiles));
            self.index = if !app_switch {
                list.clamp(self.index)
            } else if launched && self.index <= 0 {
                self.last_use = Some(now);
                list.clamp(self.index)
            } else {
                1
            };
        }
    }

    /// Consumes the pending launch a window of `class` belongs to: the
    /// first of `kin` (by lineage), else the first of its class, else the
    /// oldest without a class; `false` when there is none.
    fn claim(&mut self, class: &str, kin: &[LaunchId]) -> bool {
        let at = self
            .launches
            .iter()
            .position(|launch| kin.contains(&launch.id))
            .or_else(|| {
                self.launches.iter().position(|launch| {
                    launch
                        .class
                        .as_deref()
                        .is_some_and(|rule| class_matches(rule, class))
                })
            })
            .or_else(|| {
                self.launches
                    .iter()
                    .position(|launch| launch.class.is_none())
            });
        at.map(|at| self.launches.remove(at)).is_some()
    }

    /// A short press: the next index towards `button`, nothing past an end.
    fn step(&mut self, button: PageButton, profiles: &Profiles) {
        let list = PageList::of(profiles, self.shown(profiles));
        let next = match button {
            PageButton::Left => self.index - 1,
            PageButton::Right => self.index + 1,
        };
        if list.clamp(next) == next {
            self.index = next;
        }
    }

    /// A long press: auto mode locks the shown profile and index; the lock
    /// released shows index 1 of the profile the focused window selects.
    fn toggle_mode(&mut self) {
        match self.mode {
            Mode::Auto => self.mode = Mode::Manual,
            Mode::Manual => {
                self.mode = Mode::Auto;
                self.shown.clone_from(&self.selected);
                self.index = 1;
            }
        }
    }

    /// Both page buttons: the global context page in auto mode; the held
    /// buttons do nothing more until they are pressed again.
    fn escape(&mut self) {
        self.mode = Mode::Auto;
        GLOBAL_ID.clone_into(&mut self.selected);
        GLOBAL_ID.clone_into(&mut self.shown);
        self.index = 1;
        self.buttons = PressDetector::new(self.long_press);
    }
}

/// The page list of one shown profile (spec §6.3), lowest index first.
struct PageList<'a> {
    /// The index of the first entry: 1 minus the global pages from
    /// `general` on, 1 when the global profile has no `general` page.
    lowest: isize,
    /// The profile and page (counted from 1) of each index.
    pages: Vec<(&'a Profile, usize)>,
}

impl<'a> PageList<'a> {
    fn of(profiles: &'a Profiles, shown: &'a Profile) -> Self {
        let global = profiles.global();
        let general = named_page(global, "general");
        let global_context = context_page(global);
        let is_global = shown.profile.id == GLOBAL_ID;
        // The enabled global pages from the last one down to `general`:
        // indices ..., -1, 0; none for a profile that leaves them out.
        let mut pages: Vec<_> = general
            .filter(|_| is_global || shown.profile.global_pages)
            .map(|general| (general..=global.pages.len()).rev())
            .into_iter()
            .flatten()
            .filter(|&page| page == global_context || enabled(global, page))
            .map(|page| (global, page))
            .collect();
        let lowest = 1 - isize::try_from(pages.len()).unwrap_or(0);
        let context = context_page(shown);
        pages.push((shown, context));
        pages.extend(
            (1..=shown.pages.len())
                .filter(|&page| page != context && enabled(shown, page))
                // The global profile's pages from `general` on are left of
                // its context page already.
                .filter(|&page| !is_global || general.is_none_or(|general| page < general))
                .map(|page| (shown, page)),
        );
        Self { lowest, pages }
    }

    /// The highest index.
    fn highest(&self) -> isize {
        self.lowest + isize::try_from(self.pages.len()).unwrap_or(1) - 1
    }

    /// `index` moved into the list: the nearest end when it is outside.
    fn clamp(&self, index: isize) -> isize {
        index.clamp(self.lowest, self.highest())
    }

    /// The profile and page at `index`.
    fn get(&self, index: isize) -> Option<(&'a Profile, usize)> {
        let at = usize::try_from(index - self.lowest).ok()?;
        self.pages.get(at).copied()
    }
}

/// The page named `context`, else the first enabled one, else the first;
/// counted from 1. It is in the page list whether it is enabled or not.
pub(crate) fn context_page(profile: &Profile) -> usize {
    named_page(profile, "context")
        .or_else(|| {
            profile
                .pages
                .iter()
                .position(|page| page.enabled)
                .map(|index| index + 1)
        })
        .unwrap_or(1)
}

/// Whether page `page` (from 1) of `profile` is enabled.
fn enabled(profile: &Profile, page: usize) -> bool {
    page.checked_sub(1)
        .and_then(|index| profile.pages.get(index))
        .is_some_and(|page| page.enabled)
}

fn named_page(profile: &Profile, name: &str) -> Option<usize> {
    profile
        .pages
        .iter()
        .position(|page| page.name.as_deref() == Some(name))
        .map(|index| index + 1)
}
