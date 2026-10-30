//! The daemon's view of the pad: the loaded configuration, the active
//! profile and page, and the automatic state of every key. Synchronous and
//! free of I/O; the controller feeds it events and asks it what the keys
//! should show.
//!
//! The profile and page come from the context state machine
//! (`keyvo_core::state`): the focused window and the page buttons. Key
//! states belong to a key of one profile page, so a running `exec` keeps
//! its look on its own page and never shows on another profile.
//!
//! The Dialpad's turns and button presses resolve on the active profile's
//! `[dialpad]` table over the global one (`keyvo_core::dialpad`); its
//! controls have no display and no look. While a modifier key is held or
//! armed (`keyvo_core::state`), a turn resolves on that key's `dial` or
//! `roller` table instead, and the key shows the `modifier` look.

use std::collections::BTreeMap;
use std::time::{Instant, SystemTime};

use keyvo_core::config::Config;
use keyvo_core::dialpad::{self, Bindings, ButtonSlot, Control, Roller, Turn, Turns};
use keyvo_core::feedback::{Feedback, Look};
use keyvo_core::focus::WindowContext;
use keyvo_core::layout::Loaded;
use keyvo_core::lineage::Lineage;
use keyvo_core::press;
use keyvo_core::press::PressKind;
use keyvo_core::profile::{Action, Key, KeySlot, ModifierMode, Profile};
use keyvo_core::render::{self, KeySpec};
use keyvo_core::session::Session;
use keyvo_core::stack::{Layer, Stack};
use keyvo_core::state::{
    ActiveModifier, KeyDown, KeyPress, LaunchId, ModifierKey, PageButton, State, View,
};
use keyvo_core::trace::{Outcome, TraceRecord, Trigger};

/// The reason of a press blocked while the session is locked.
pub const BLOCKED_LOCKED: &str = "session locked";

/// A key of one page (counted from 1) of one profile.
type KeyId = (String, usize, KeySlot);

/// The pad state behind every paint.
#[derive(Debug, Clone)]
pub struct Engine {
    loaded: Loaded,
    state: State,
    keys: u8,
    feedback: BTreeMap<KeyId, Feedback>,
    /// The detents below one `step` each roller carries.
    turns: Turns,
    /// The modifier key whose binding the carry belongs to; `None` for the
    /// profile's `[dialpad]` table.
    turns_for: Option<ActiveModifier>,
}

impl Engine {
    /// The global profile's context page in auto mode, before any device
    /// told its key count. The state machine's `[timing]` values
    /// (`focus_hysteresis`, `long_press`) and `[focus].hold_classes` are
    /// read here; `page_return_global` and `page_return_own` here and on
    /// every reload.
    #[must_use]
    pub fn new(loaded: Loaded) -> Self {
        let state = State::new(&loaded.config, &loaded.profiles);
        Self {
            loaded,
            state,
            keys: 0,
            feedback: BTreeMap::new(),
            turns: Turns::default(),
            turns_for: None,
        }
    }

    /// The configuration in use.
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.loaded.config
    }

    /// The number of keys the device publishes; 0 until one is connected.
    pub fn set_key_count(&mut self, keys: u8) {
        self.keys = keys;
    }

    /// Replaces the configuration after a reload: in auto mode the focused
    /// window selects its profile again, as the rules may have changed;
    /// manual mode keeps its profile while it exists. Key states carry
    /// over.
    pub fn reload(&mut self, loaded: Loaded) {
        self.loaded = loaded;
        self.state
            .reload(&self.loaded.config, &self.loaded.profiles);
        // The bindings may have changed their step.
        self.turns.clear();
    }

    /// The profile, page and mode the pad shows.
    #[must_use]
    pub fn view(&self) -> View {
        self.state.view(&self.loaded.profiles)
    }

    /// The active profile.
    #[must_use]
    pub fn active(&self) -> &Profile {
        let view = self.view();
        self.loaded
            .profiles
            .get(&view.profile)
            .unwrap_or_else(|| self.loaded.profiles.global())
    }

    /// The active page, counted from 1.
    #[must_use]
    pub fn page(&self) -> usize {
        self.view().page
    }

    /// The focus source reports `window` at `now`; `lineage` tells which
    /// launch its process belongs to.
    pub fn focus(&mut self, window: WindowContext, now: Instant, lineage: &dyn Lineage) {
        self.state
            .focus(window, now, &self.loaded.profiles, lineage);
    }

    /// `action` was pressed at `now`: a `launch` with `wait_focus` set
    /// expects its window ([`State::launched`]); `None` for every other
    /// action, and when `[timing].launch_focus` is 0.
    pub fn launched(&mut self, action: &Action, now: Instant) -> Option<LaunchId> {
        match action {
            Action::Launch {
                wait_focus: true, ..
            } => self.state.launched(action.launch_class(), now),
            _ => None,
        }
    }

    /// The shell of launch `id` runs as process `pid`.
    pub fn launch_started(&mut self, id: LaunchId, pid: u32) {
        self.state.launch_started(id, pid);
    }

    /// Launch `id` did not start or exited with an error.
    pub fn launch_failed(&mut self, id: LaunchId) {
        self.state.launch_failed(id);
    }

    /// The pending launches, oldest first.
    #[must_use]
    pub fn launches(&self) -> Vec<LaunchId> {
        self.state.launches()
    }

    /// Whether a window of `class` holds the context
    /// (`[focus].hold_classes`).
    #[must_use]
    pub fn holds(&self, class: &str) -> bool {
        self.state.holds(class)
    }

    /// A page button went down at `now`.
    pub fn page_press(&mut self, button: PageButton, now: Instant) {
        self.state.press(button, now, &self.loaded.profiles);
    }

    /// A page button came up at `now`.
    pub fn page_release(&mut self, button: PageButton, now: Instant) {
        self.state.release(button, now, &self.loaded.profiles);
    }

    /// The session source reports `session`; `true` when the lock state
    /// changed. While locked the keys show the lock page and every press
    /// is blocked.
    pub fn session(&mut self, session: Session) -> bool {
        self.state.session(session)
    }

    /// Key `slot` went down at `now`: the page return starts over, the
    /// armed toggle ends, a held modifier key takes the dial and the roller
    /// ([`State::key_down`]).
    pub fn key_down(&mut self, slot: KeySlot, now: Instant) -> KeyDown {
        self.state.key_down(slot, now, &self.loaded.profiles)
    }

    /// Key `slot` came up at `now`: for a held modifier key whether its
    /// action runs ([`State::key_up`]).
    pub fn key_up(&mut self, slot: KeySlot, now: Instant) -> Option<bool> {
        self.state.key_up(slot, now)
    }

    /// The modifier mode of key `slot` on the active page; `None` for an
    /// ordinary key.
    #[must_use]
    pub fn modifier_mode(&self, slot: KeySlot) -> Option<ModifierMode> {
        self.key(slot).and_then(|(key, _)| key.modifier_mode())
    }

    /// A Dialpad button went down at `now`: pad activity, and the end of
    /// the armed toggle.
    pub fn button_down(&mut self, now: Instant) {
        self.state.touch(now);
        self.state.button(now);
    }

    /// Applies a focus change, a page-button long press or the page return
    /// due at `now`.
    pub fn poll(&mut self, now: Instant) {
        self.state.poll(now, &self.loaded.profiles);
    }

    /// When [`Engine::poll`] has something to do next.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        self.state.next_deadline()
    }

    /// The active page over the global layer (spec §6.1); `None` when the
    /// active profile does not have the page.
    fn stack(&self) -> Option<Stack<'_>> {
        Stack::new(&self.loaded.profiles, self.active(), self.page()).ok()
    }

    /// The key at `slot` on the active page and the layer it comes from.
    fn key(&self, slot: KeySlot) -> Option<(&Key, Layer)> {
        self.stack().and_then(|stack| stack.key(slot))
    }

    /// What every key shows at `now`, keys 1 to the device's key count: the
    /// active profile's key, else the global one. A profile without the
    /// active page shows blank keys; a locked session the lock page.
    #[must_use]
    pub fn specs(&self, now: Instant) -> Vec<KeySpec> {
        let profile = self.active();
        if self.state.locked() {
            return render::lock_page(profile.profile.accent, self.keys);
        }
        let page = self.page();
        let specs = self.stack().map_or_else(
            || vec![KeySpec::default(); usize::from(self.keys)],
            |stack| render::page(&stack, self.keys),
        );
        // The modifier key shows its look on the page it was pressed on.
        let modifier = self
            .state
            .modifier()
            .filter(|at| at.profile == profile.profile.id && at.page == page)
            .map(|at| at.key);
        (1..=self.keys)
            .filter_map(KeySlot::new)
            .zip(specs)
            .map(|(slot, spec)| {
                let id = (profile.profile.id.clone(), page, slot);
                let look = if modifier == Some(slot) {
                    Look::Modifier
                } else {
                    self.feedback
                        .get(&id)
                        .map_or(Look::Idle, |feedback| feedback.look(now))
                };
                if look == Look::Idle {
                    spec
                } else {
                    spec.with_look(self.key(slot).map(|(key, _)| key), look)
                }
            })
            .collect()
    }

    /// When a key's look next changes on its own (running to long run,
    /// success to idle), so the controller repaints then.
    #[must_use]
    pub fn next_change(&self, now: Instant) -> Option<Instant> {
        self.feedback
            .values()
            .filter_map(|feedback| feedback.next_change(now))
            .min()
    }

    /// The look of the key `record` pressed; `None` for a Dialpad control,
    /// which has no display.
    fn feedback(&mut self, record: &TraceRecord) -> Option<&mut Feedback> {
        let render = self.loaded.config.render;
        let key = record.key?;
        Some(
            self.feedback
                .entry((record.profile.clone(), record.page, key))
                .or_insert_with(|| Feedback::new(&render)),
        )
    }

    /// The Dialpad bindings of the active profile over the global ones.
    fn bindings(&self) -> Bindings<'_> {
        Bindings::new(&self.loaded.profiles, self.active())
    }

    /// Resolves a turn of `roller` by `delta` detents at `now` on the
    /// binding of the held or armed modifier key, else on the active
    /// profile, a roller the profile leaves out on the global one: the
    /// action of the direction and how often it runs (`step` detents a run,
    /// the rest carried, at most `[dialpad].max_per_event`). A turn is no
    /// pad activity: the page return keeps running. While the session is
    /// locked nothing is resolved: [`BLOCKED_LOCKED`], no run.
    pub fn turn(
        &mut self,
        roller: Roller,
        delta: i32,
        simulated: bool,
        (at, now): (SystemTime, Instant),
    ) -> Turn {
        let trigger = dialpad::turn_trigger(simulated);
        let page = self.page();
        if self.state.locked() {
            return Turn {
                record: dialpad::blocked(
                    &self.active().profile.id,
                    page,
                    roller.into(),
                    trigger,
                    at,
                    BLOCKED_LOCKED,
                ),
                runs: 0,
            };
        }
        let max = self.loaded.config.dialpad.max_per_event.get();
        let taker = self
            .state
            .modifier_turn(roller, simulated, now, &self.loaded.profiles);
        let owner = taker.as_ref().map(ModifierKey::active);
        if !simulated && owner != self.turns_for {
            // Another binding takes the turns: nothing carries over.
            self.turns.clear();
            self.turns_for = owner;
        }
        let profiles = &self.loaded.profiles;
        if let Some(taker) = taker {
            return dialpad::modifier_turn(
                profiles,
                &taker,
                roller,
                delta,
                (trigger, at),
                &mut self.turns,
                max,
            );
        }
        let view = self.state.view(profiles);
        let active = profiles
            .get(&view.profile)
            .unwrap_or_else(|| profiles.global());
        dialpad::turn(
            &Bindings::new(profiles, active),
            page,
            roller,
            delta,
            (trigger, at),
            &mut self.turns,
            max,
        )
    }

    /// Resolves a press of Dialpad button `slot` like a key's: the active
    /// profile's button, else the global one; [`BLOCKED_LOCKED`] under the
    /// lock. A client's press ends the armed toggle at `now` as the
    /// device's does at the button's press ([`Engine::button_down`]),
    /// unless it is simulated.
    pub fn button(
        &mut self,
        slot: ButtonSlot,
        trigger: Trigger,
        (at, now): (SystemTime, Instant),
    ) -> TraceRecord {
        if !trigger.simulated {
            self.state.button(now);
        }
        let page = self.page();
        if self.state.locked() {
            return dialpad::blocked(
                &self.active().profile.id,
                page,
                Control::Button(slot),
                trigger,
                at,
                BLOCKED_LOCKED,
            );
        }
        dialpad::press(&self.bindings(), page, slot, trigger, at)
    }

    /// The `on_fail` of Dialpad button `slot`, from the layer that supplies
    /// the button.
    #[must_use]
    pub fn button_on_fail(&self, slot: ButtonSlot) -> Option<Action> {
        self.bindings()
            .button(slot)
            .and_then(|(button, _)| button.on_fail.clone())
    }

    /// Resolves a press of `slot` at `now` on the active profile and page,
    /// a key the profile lacks on the global layer; an error or success
    /// look of the key ends, unless the press is simulated. A short press
    /// of a toggle key arms or ends the toggle and runs nothing
    /// ([`Outcome::Modifier`]); any other press ends the armed toggle of
    /// another key. While the session is locked nothing is resolved: the
    /// press is [`BLOCKED_LOCKED`] and the look stays.
    pub fn press(
        &mut self,
        slot: KeySlot,
        trigger: Trigger,
        (at, now): (SystemTime, Instant),
    ) -> TraceRecord {
        if self.state.locked() {
            return press::blocked(
                &self.active().profile.id,
                self.page(),
                slot,
                trigger,
                at,
                BLOCKED_LOCKED,
            );
        }
        // The record names the page the press was made on, before a toggle
        // changes anything.
        let mut record = self.stack().map_or_else(
            || press::unbound(&self.active().profile.id, self.page(), slot, trigger, at),
            |stack| press::resolve(&stack, slot, trigger, at),
        );
        let decision = self.state.key_press(
            slot,
            trigger.press,
            trigger.simulated,
            now,
            &self.loaded.profiles,
        );
        if let KeyPress::Toggle(active) = decision {
            record.action = None;
            record.modifier = Some(ModifierMode::Toggle);
            record.result = if trigger.simulated {
                Outcome::DryRun
            } else {
                Outcome::Modifier { active }
            };
            return record;
        }
        if !trigger.simulated
            && let Some(feedback) = self.feedback(&record)
        {
            feedback.pressed();
        }
        record
    }

    /// A client holds key `slot` down at `now` (`press` with `hold`): a
    /// held modifier key takes the dial and the roller until
    /// [`Engine::release`]. A simulated hold changes nothing. The record of
    /// the hold, [`Outcome::Modifier`]; an error for a key that is not a
    /// held modifier key on the active page.
    ///
    /// # Errors
    ///
    /// What is wrong, for the client.
    pub fn hold(
        &mut self,
        slot: KeySlot,
        simulated: bool,
        (at, now): (SystemTime, Instant),
    ) -> Result<TraceRecord, String> {
        let trigger = Trigger {
            press: PressKind::Short,
            simulated,
        };
        if self.state.locked() {
            return Ok(self.press(slot, trigger, (at, now)));
        }
        if self.modifier_mode(slot) != Some(ModifierMode::Held) {
            return Err(format!(
                "key {} is not a held modifier key on this page",
                slot.get()
            ));
        }
        let mut record = self.modifier_record(slot, trigger, at);
        record.result = if simulated {
            Outcome::DryRun
        } else {
            self.state.key_down(slot, now, &self.loaded.profiles);
            Outcome::Modifier { active: true }
        };
        Ok(record)
    }

    /// A client lets key `slot` go at `now` (`press` with `release`): the
    /// key's action runs when no turn took its binding, else the record of
    /// the release, [`Outcome::Modifier`]. A simulated release changes
    /// nothing; an error for a key not held.
    ///
    /// # Errors
    ///
    /// What is wrong, for the client.
    pub fn release(
        &mut self,
        slot: KeySlot,
        simulated: bool,
        (at, now): (SystemTime, Instant),
    ) -> Result<TraceRecord, String> {
        let trigger = Trigger {
            press: PressKind::Short,
            simulated,
        };
        if self.state.locked() {
            return Ok(self.press(slot, trigger, (at, now)));
        }
        let held = self
            .state
            .modifier_keys()
            .any(|at| at.key == slot && at.mode == ModifierMode::Held);
        if !held {
            return Err(format!("key {} is not held", slot.get()));
        }
        if simulated {
            let mut record = self.modifier_record(slot, trigger, at);
            record.result = Outcome::DryRun;
            return Ok(record);
        }
        if self.state.key_up(slot, now) == Some(true) {
            return Ok(self.press(slot, trigger, (at, now)));
        }
        let mut record = self.modifier_record(slot, trigger, at);
        record.result = Outcome::Modifier { active: false };
        Ok(record)
    }

    /// The record of a hold or release of the held modifier key `slot`:
    /// the key resolved, no action.
    fn modifier_record(&self, slot: KeySlot, trigger: Trigger, at: SystemTime) -> TraceRecord {
        let mut record = self.stack().map_or_else(
            || press::unbound(&self.active().profile.id, self.page(), slot, trigger, at),
            |stack| press::resolve(&stack, slot, trigger, at),
        );
        record.action = None;
        record.modifier = Some(ModifierMode::Held);
        record
    }

    /// The `on_fail` action of the key at `slot` on the active page, a key
    /// the profile lacks on the global layer, as [`Engine::press`] resolves
    /// its action.
    #[must_use]
    pub fn on_fail(&self, slot: KeySlot) -> Option<Action> {
        self.key(slot).and_then(|(key, _)| key.on_fail.clone())
    }

    /// The action of the press `record` started running at `now`.
    pub fn started(&mut self, record: &TraceRecord, now: Instant) {
        if let Some(feedback) = self.feedback(record) {
            feedback.started(now);
        }
    }

    /// The action of the press `record` ended at `now` with its result.
    pub fn finished(&mut self, record: &TraceRecord, now: Instant) {
        if let Some(feedback) = self.feedback(record) {
            feedback.finished(&record.result, now);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::time::Duration;

    use keyvo_core::layout::Profiles;
    use keyvo_core::lineage::NoLineage;
    use keyvo_core::press::PressKind;
    use keyvo_core::profile::{Action, GLOBAL_ID};
    use keyvo_core::session::Session;
    use keyvo_core::state::Mode;
    use keyvo_core::trace::Outcome;

    use super::*;

    const GLOBAL: &str = r##"
schema_version = 1
[profile]
id = "global"
name = "Global"
accent = "#4e8bd6"

[[pages]]
[pages.keys.1]
label = "Build"
action = { type = "exec", cmd = "true" }
states.error = { label = "Broken" }

[pages.keys.2]
label = "Copy"
action = { type = "chord", keys = "ctrl+c" }

[[pages]]
name = "general"
[pages.keys.1]
label = "Play"
"##;

    const CODE: &str = r#"
schema_version = 1
[profile]
id = "code"
name = "Code"
match = { class = ["code"] }

[[pages]]
[pages.keys.1]
label = "Run"
action = { type = "exec", cmd = "true" }
"#;

    fn loaded_with(texts: &[&str]) -> Loaded {
        let profiles = texts
            .iter()
            .map(|text| {
                let profile = Profile::from_toml(text, Path::new("p.toml")).unwrap();
                (profile.profile.id.clone(), profile)
            })
            .collect();
        Loaded {
            config: Config::default(),
            profiles: Profiles::new(profiles),
        }
    }

    fn loaded(text: &str) -> Loaded {
        loaded_with(&[text])
    }

    fn slot(n: u8) -> KeySlot {
        KeySlot::new(n).unwrap()
    }

    fn short() -> Trigger {
        Trigger {
            press: PressKind::Short,
            simulated: false,
        }
    }

    /// `engine` shows the profile for a `code` window from `now` on.
    fn focus_code(engine: &mut Engine, now: Instant) {
        engine.focus(WindowContext::new("code", "main.rs"), now, &NoLineage);
        let at = engine.next_deadline().unwrap();
        assert_eq!(at, now + engine.config().timing.focus_hysteresis);
        engine.poll(at);
        assert_eq!(engine.active().profile.id, "code");
    }

    #[test]
    fn only_a_launch_that_waits_for_its_window_expects_one() {
        let mut engine = Engine::new(loaded(GLOBAL));
        let now = Instant::now();
        let flatpak = Action::launch("flatpak run org.kde.kcalc");
        let id = engine.launched(&flatpak, now).unwrap();
        let shy = Action::Launch {
            cmd: "kcalc".into(),
            class: Some("org.kde.kcalc".into()),
            wait_focus: false,
        };
        assert_eq!(engine.launched(&shy, now), None);
        assert_eq!(
            engine.launched(&Action::Exec { cmd: "x".into() }, now),
            None
        );
        assert_eq!(engine.launches(), [id]);
        engine.launch_started(id, 4242);
        // The derived class matches KCalc's window.
        engine.focus(
            WindowContext::new("org.kde.kcalc", "KCalc"),
            now,
            &NoLineage,
        );
        engine.poll(now + engine.config().timing.focus_hysteresis);
        assert_eq!(engine.launches(), []);
        let failed = engine.launched(&flatpak, now).unwrap();
        engine.launch_failed(failed);
        assert_eq!(engine.launches(), []);
    }

    #[test]
    fn no_key_shows_before_a_device_tells_the_key_count() {
        let engine = Engine::new(loaded(GLOBAL));
        assert!(engine.specs(Instant::now()).is_empty());
    }

    #[test]
    fn the_specs_are_the_active_page_of_the_global_profile() {
        let mut engine = Engine::new(loaded(GLOBAL));
        engine.set_key_count(9);
        let specs = engine.specs(Instant::now());
        assert_eq!(specs.len(), 9);
        let stack = Stack::new(&engine.loaded.profiles, engine.active(), 1).unwrap();
        assert_eq!(specs, render::page(&stack, 9));
        assert_eq!(specs[0].label.as_deref(), Some("Build"));
        assert_eq!(engine.page(), 1);
    }

    #[test]
    fn a_focused_window_and_the_page_buttons_choose_the_specs() {
        let mut engine = Engine::new(loaded_with(&[GLOBAL, CODE]));
        engine.set_key_count(9);
        let now = Instant::now();
        focus_code(&mut engine, now);
        assert_eq!(engine.specs(now)[0].label.as_deref(), Some("Run"));
        engine.page_press(PageButton::Left, now);
        engine.page_release(PageButton::Left, now);
        assert_eq!(
            engine.view(),
            View {
                profile: GLOBAL_ID.into(),
                page: 2,
                mode: Mode::Auto,
                index: 0,
                locked: false,
                modifier: None
            }
        );
        assert_eq!(engine.specs(now)[0].label.as_deref(), Some("Play"));
    }

    #[test]
    fn a_key_the_app_profile_lacks_shows_and_runs_the_global_key() {
        let mut engine = Engine::new(loaded_with(&[GLOBAL, CODE]));
        engine.set_key_count(9);
        let now = Instant::now();
        focus_code(&mut engine, now);
        let specs = engine.specs(now);
        assert_eq!(specs[0].label.as_deref(), Some("Run"));
        // `code` has no key 2: the global "Copy", with the code accent.
        assert_eq!(specs[1].label.as_deref(), Some("Copy"));
        assert_eq!(specs[1].accent, engine.active().profile.accent);
        let record = engine.press(slot(2), short(), (SystemTime::UNIX_EPOCH, Instant::now()));
        assert_eq!(record.profile, "code");
        assert_eq!(record.layer, Some(Layer::Global));
        assert!(matches!(record.action, Some(Action::Chord { .. })));
        let own = engine.press(slot(1), short(), (SystemTime::UNIX_EPOCH, Instant::now()));
        assert_eq!(own.layer, Some(Layer::App));
    }

    #[test]
    fn a_key_brings_its_on_fail_from_the_layer_that_supplies_it() {
        let notify = "on_fail = { type = \"exec\", cmd = \"notify-send copy\" }\n";
        let global = GLOBAL.replace(
            "action = { type = \"chord\", keys = \"ctrl+c\" }\n",
            &format!("action = {{ type = \"chord\", keys = \"ctrl+c\" }}\n{notify}"),
        );
        let mut engine = Engine::new(loaded_with(&[&global, CODE]));
        engine.set_key_count(9);
        focus_code(&mut engine, Instant::now());
        // `code` has no key 2: the global key and its on_fail.
        assert_eq!(
            engine.on_fail(slot(2)),
            Some(Action::Exec {
                cmd: "notify-send copy".into()
            })
        );
        assert_eq!(engine.on_fail(slot(1)), None);
        assert_eq!(engine.on_fail(slot(9)), None);
    }

    #[test]
    fn a_global_key_shows_its_own_states_on_the_app_page() {
        let mut engine = Engine::new(loaded_with(&[
            &GLOBAL.replace(
                "[[pages]]\nname = \"general\"",
                "[pages.keys.3]\nlabel = \"Lint\"\naction = { type = \"exec\", cmd = \"true\" }\n\
                 states.error = { label = \"Dirty\" }\n\n[[pages]]\nname = \"general\"",
            ),
            CODE,
        ]));
        engine.set_key_count(9);
        let now = Instant::now();
        focus_code(&mut engine, now);
        let mut record = engine.press(slot(3), short(), (SystemTime::UNIX_EPOCH, Instant::now()));
        assert_eq!(record.layer, Some(Layer::Global));
        record.finish(Outcome::Failed { error: "x".into() });
        engine.finished(&record, now);
        let spec = &engine.specs(now)[2];
        assert_eq!(
            (spec.look, spec.label.as_deref()),
            (Look::Error, Some("Dirty"))
        );
    }

    #[test]
    fn a_key_state_stays_with_the_page_it_was_pressed_on() {
        let mut engine = Engine::new(loaded_with(&[GLOBAL, CODE]));
        engine.set_key_count(9);
        let now = Instant::now();
        let record = engine.press(slot(1), short(), (SystemTime::UNIX_EPOCH, Instant::now()));
        engine.started(&record, now);
        assert_eq!(engine.specs(now)[0].look, Look::Running);
        focus_code(&mut engine, now);
        assert_eq!(engine.specs(now)[0].look, Look::Idle);
        engine.page_press(PageButton::Left, now);
        engine.page_release(PageButton::Left, now);
        // The general page is another page of the global profile.
        assert_eq!(engine.specs(now)[0].look, Look::Idle);
    }

    #[test]
    fn a_failed_exec_shows_the_error_look_with_the_key_state() {
        let mut engine = Engine::new(loaded(GLOBAL));
        engine.set_key_count(9);
        let now = Instant::now();
        let mut record = engine.press(slot(1), short(), (SystemTime::UNIX_EPOCH, Instant::now()));
        assert_eq!(record.result, Outcome::Pending);
        assert!(matches!(record.action, Some(Action::Exec { .. })));
        engine.started(&record, now);
        assert_eq!(engine.specs(now)[0].look, Look::Running);
        record.finish(Outcome::Exited {
            code: Some(1),
            duration_ms: 5,
        });
        engine.finished(&record, now);
        let spec = &engine.specs(now)[0];
        assert_eq!(spec.look, Look::Error);
        assert_eq!(spec.label.as_deref(), Some("Broken"));
        assert_eq!(engine.next_change(now), None);
        // The next press of the key ends the error look.
        engine.press(slot(1), short(), (SystemTime::UNIX_EPOCH, Instant::now()));
        assert_eq!(engine.specs(now)[0].look, Look::Idle);
    }

    #[test]
    fn a_simulated_press_keeps_the_error_look() {
        let mut engine = Engine::new(loaded(GLOBAL));
        engine.set_key_count(9);
        let now = Instant::now();
        let mut record = engine.press(slot(1), short(), (SystemTime::UNIX_EPOCH, Instant::now()));
        record.finish(Outcome::Failed { error: "x".into() });
        engine.finished(&record, now);
        let simulated = Trigger {
            press: PressKind::Short,
            simulated: true,
        };
        let dry = engine.press(slot(1), simulated, (SystemTime::UNIX_EPOCH, Instant::now()));
        assert_eq!(dry.result, Outcome::DryRun);
        assert_eq!(engine.specs(now)[0].look, Look::Error);
    }

    #[test]
    fn success_reverts_after_the_feedback_hold() {
        let mut engine = Engine::new(loaded(GLOBAL));
        engine.set_key_count(9);
        let now = Instant::now();
        let hold = engine.config().render.feedback_hold;
        let mut record = engine.press(slot(1), short(), (SystemTime::UNIX_EPOCH, Instant::now()));
        engine.started(&record, now);
        assert_eq!(
            engine.next_change(now),
            Some(now + engine.config().render.long_run_threshold)
        );
        record.finish(Outcome::Exited {
            code: Some(0),
            duration_ms: 5,
        });
        engine.finished(&record, now);
        assert_eq!(engine.specs(now)[0].look, Look::Success);
        assert_eq!(engine.next_change(now), Some(now + hold));
        assert_eq!(engine.specs(now + hold)[0].look, Look::Idle);
    }

    #[test]
    fn a_locked_session_shows_the_lock_page_with_the_profiles_accent() {
        let mut engine = Engine::new(loaded_with(&[GLOBAL, CODE]));
        engine.set_key_count(9);
        let now = Instant::now();
        focus_code(&mut engine, now);
        assert!(engine.session(Session::Locked));
        assert!(!engine.session(Session::Locked));
        assert!(engine.view().locked);
        assert_eq!(
            engine.specs(now),
            render::lock_page(engine.active().profile.accent, 9)
        );
        assert!(engine.session(Session::Unlocked));
        assert_eq!(engine.specs(now)[0].label.as_deref(), Some("Run"));
    }

    #[test]
    fn a_press_under_the_lock_is_blocked_and_leaves_the_look_alone() {
        let mut engine = Engine::new(loaded(GLOBAL));
        engine.set_key_count(9);
        let now = Instant::now();
        let mut record = engine.press(slot(1), short(), (SystemTime::UNIX_EPOCH, Instant::now()));
        record.finish(Outcome::Failed { error: "x".into() });
        engine.finished(&record, now);
        engine.session(Session::Locked);
        for trigger in [
            short(),
            Trigger {
                simulated: true,
                ..short()
            },
        ] {
            let blocked = engine.press(slot(1), trigger, (SystemTime::UNIX_EPOCH, Instant::now()));
            assert_eq!(
                blocked.result,
                Outcome::Blocked {
                    reason: "session locked".into()
                }
            );
            assert_eq!((blocked.action, blocked.layer), (None, None));
            assert_eq!((blocked.profile.as_str(), blocked.page), ("global", 1));
        }
        engine.session(Session::Unlocked);
        // The error look of before the lock is still there.
        assert_eq!(engine.specs(now)[0].look, Look::Error);
    }

    const DIALPAD: &str = "\n[dialpad]\ndial = { cw = \"KEY_VOLUMEUP\", ccw = \"KEY_VOLUMEDOWN\", step = 2 }\n\
                           [dialpad.buttons.1]\naction = { type = \"exec\", cmd = \"false\" }\n\
                           on_fail = { type = \"exec\", cmd = \"true\" }\n";

    #[test]
    fn a_turn_and_a_button_resolve_on_the_global_table_under_an_app_profile() {
        let global = format!("{GLOBAL}{DIALPAD}");
        let mut engine = Engine::new(loaded_with(&[&global, CODE]));
        engine.set_key_count(9);
        let now = Instant::now();
        focus_code(&mut engine, now);
        let at = SystemTime::UNIX_EPOCH;
        let turn = engine.turn(Roller::Dial, 5, false, (at, Instant::now()));
        assert_eq!((turn.runs, turn.record.layer), (2, Some(Layer::Global)));
        assert_eq!(turn.record.profile, "code");
        // The detent left over carries.
        assert_eq!(
            engine
                .turn(Roller::Dial, 1, false, (at, Instant::now()))
                .runs,
            1
        );
        assert_eq!(
            engine
                .turn(Roller::Roller, 3, false, (at, Instant::now()))
                .record
                .result,
            Outcome::Unbound
        );
        let button = ButtonSlot::new(1).unwrap();
        let press = engine.button(button, short(), (at, Instant::now()));
        assert!(matches!(press.action, Some(Action::Exec { .. })));
        assert!(engine.button_on_fail(button).is_some());
        // A button has no look; its run leaves the keys alone.
        engine.started(&press, now);
        assert_eq!(engine.next_change(now), None);
        assert!(engine.specs(now).iter().all(|spec| spec.look == Look::Idle));
    }

    #[test]
    fn a_turn_or_a_button_under_the_lock_is_blocked() {
        let mut engine = Engine::new(loaded(&format!("{GLOBAL}{DIALPAD}")));
        engine.session(Session::Locked);
        let at = SystemTime::UNIX_EPOCH;
        let turn = engine.turn(Roller::Dial, 4, false, (at, Instant::now()));
        assert_eq!(turn.runs, 0);
        assert_eq!(
            turn.record.result,
            Outcome::Blocked {
                reason: BLOCKED_LOCKED.into()
            }
        );
        let button = engine.button(ButtonSlot::new(1).unwrap(), short(), (at, Instant::now()));
        assert_eq!((button.action, button.layer), (None, None));
        engine.session(Session::Unlocked);
        // Nothing carried from the turn under the lock.
        assert_eq!(
            engine
                .turn(Roller::Dial, 1, false, (at, Instant::now()))
                .runs,
            0
        );
    }

    #[test]
    fn a_missing_page_shows_blank_keys() {
        let mut engine = Engine::new(loaded(
            "schema_version = 1\n[profile]\nid = \"global\"\nname = \"G\"\n",
        ));
        engine.set_key_count(9);
        assert_eq!(engine.specs(Instant::now()), vec![KeySpec::default(); 9]);
        let record = engine.press(slot(3), short(), (SystemTime::UNIX_EPOCH, Instant::now()));
        assert_eq!(record.result, Outcome::Unbound);
    }

    #[test]
    fn a_reload_replaces_the_keys_and_keeps_key_states() {
        let mut engine = Engine::new(loaded(GLOBAL));
        engine.set_key_count(9);
        let now = Instant::now();
        let mut record = engine.press(slot(1), short(), (SystemTime::UNIX_EPOCH, Instant::now()));
        record.finish(Outcome::Failed { error: "x".into() });
        engine.finished(&record, now);
        engine.reload(loaded(&GLOBAL.replace("\"Copy\"", "\"Paste\"")));
        let specs = engine.specs(now);
        assert_eq!(specs[1].label.as_deref(), Some("Paste"));
        assert_eq!(specs[0].look, Look::Error);
        assert_eq!(engine.active().profile.id, "global");
    }

    #[test]
    fn a_reload_without_the_active_profile_falls_back() {
        let mut engine = Engine::new(loaded_with(&[GLOBAL, CODE]));
        focus_code(&mut engine, Instant::now());
        engine.reload(Loaded {
            config: Config::default(),
            profiles: Profiles::new(BTreeMap::new()),
        });
        assert_eq!(engine.active().profile.id, GLOBAL_ID);
        assert_eq!(engine.page(), 1);
        assert_eq!(engine.next_deadline(), None);
        assert_eq!(
            engine.next_change(Instant::now() + Duration::from_secs(1)),
            None
        );
    }
}
