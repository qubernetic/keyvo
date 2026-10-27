//! The daemon's view of the pad: the loaded configuration, the active
//! profile and page, and the automatic state of every key. Synchronous and
//! free of I/O; the controller feeds it events and asks it what the keys
//! should show.
//!
//! The profile and page come from the context state machine
//! (`keyvo_core::state`): the focused window and the page buttons. Key
//! states belong to a key of one profile page, so a running `exec` keeps
//! its look on its own page and never shows on another profile.

use std::collections::BTreeMap;
use std::time::{Instant, SystemTime};

use keyvo_core::config::Config;
use keyvo_core::feedback::{Feedback, Look};
use keyvo_core::focus::WindowContext;
use keyvo_core::layout::Loaded;
use keyvo_core::press;
use keyvo_core::profile::{Key, KeySlot, Profile};
use keyvo_core::render::{self, KeySpec};
use keyvo_core::stack::{Layer, Stack};
use keyvo_core::state::{PageButton, State, View};
use keyvo_core::trace::{TraceRecord, Trigger};

/// A key of one page (counted from 1) of one profile.
type KeyId = (String, usize, KeySlot);

/// The pad state behind every paint.
#[derive(Debug, Clone)]
pub struct Engine {
    loaded: Loaded,
    state: State,
    keys: u8,
    feedback: BTreeMap<KeyId, Feedback>,
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

    /// The focus source reports `window` at `now`.
    pub fn focus(&mut self, window: WindowContext, now: Instant) {
        self.state.focus(window, now, &self.loaded.profiles);
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

    /// A key on the pad went down at `now`: the page return starts over.
    pub const fn touch(&mut self, now: Instant) {
        self.state.touch(now);
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
    /// active page shows blank keys.
    #[must_use]
    pub fn specs(&self, now: Instant) -> Vec<KeySpec> {
        let profile = self.active();
        let page = self.page();
        let specs = self.stack().map_or_else(
            || vec![KeySpec::default(); usize::from(self.keys)],
            |stack| render::page(&stack, self.keys),
        );
        (1..=self.keys)
            .filter_map(KeySlot::new)
            .zip(specs)
            .map(|(slot, spec)| {
                let id = (profile.profile.id.clone(), page, slot);
                let look = self
                    .feedback
                    .get(&id)
                    .map_or(Look::Idle, |feedback| feedback.look(now));
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

    fn feedback(&mut self, record: &TraceRecord) -> &mut Feedback {
        let render = self.loaded.config.render;
        self.feedback
            .entry((record.profile.clone(), record.page, record.key))
            .or_insert_with(|| Feedback::new(&render))
    }

    /// Resolves a press of `slot` on the active profile and page, a key the
    /// profile lacks on the global layer; an error or success look of the
    /// key ends, unless the press is simulated.
    pub fn press(&mut self, slot: KeySlot, trigger: Trigger, at: SystemTime) -> TraceRecord {
        let record = self.stack().map_or_else(
            || press::unbound(&self.active().profile.id, self.page(), slot, trigger, at),
            |stack| press::resolve(&stack, slot, trigger, at),
        );
        if !trigger.simulated {
            self.feedback(&record).pressed();
        }
        record
    }

    /// The action of the press `record` started running at `now`.
    pub fn started(&mut self, record: &TraceRecord, now: Instant) {
        self.feedback(record).started(now);
    }

    /// The action of the press `record` ended at `now` with its result.
    pub fn finished(&mut self, record: &TraceRecord, now: Instant) {
        self.feedback(record).finished(&record.result, now);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::time::Duration;

    use keyvo_core::layout::Profiles;
    use keyvo_core::press::PressKind;
    use keyvo_core::profile::{Action, GLOBAL_ID};
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
        engine.focus(WindowContext::new("code", "main.rs"), now);
        let at = engine.next_deadline().unwrap();
        assert_eq!(at, now + engine.config().timing.focus_hysteresis);
        engine.poll(at);
        assert_eq!(engine.active().profile.id, "code");
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
                index: 0
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
        let record = engine.press(slot(2), short(), SystemTime::UNIX_EPOCH);
        assert_eq!(record.profile, "code");
        assert_eq!(record.layer, Some(Layer::Global));
        assert!(matches!(record.action, Some(Action::Chord { .. })));
        let own = engine.press(slot(1), short(), SystemTime::UNIX_EPOCH);
        assert_eq!(own.layer, Some(Layer::App));
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
        let mut record = engine.press(slot(3), short(), SystemTime::UNIX_EPOCH);
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
        let record = engine.press(slot(1), short(), SystemTime::UNIX_EPOCH);
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
        let mut record = engine.press(slot(1), short(), SystemTime::UNIX_EPOCH);
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
        engine.press(slot(1), short(), SystemTime::UNIX_EPOCH);
        assert_eq!(engine.specs(now)[0].look, Look::Idle);
    }

    #[test]
    fn a_simulated_press_keeps_the_error_look() {
        let mut engine = Engine::new(loaded(GLOBAL));
        engine.set_key_count(9);
        let now = Instant::now();
        let mut record = engine.press(slot(1), short(), SystemTime::UNIX_EPOCH);
        record.finish(Outcome::Failed { error: "x".into() });
        engine.finished(&record, now);
        let simulated = Trigger {
            press: PressKind::Short,
            simulated: true,
        };
        let dry = engine.press(slot(1), simulated, SystemTime::UNIX_EPOCH);
        assert_eq!(dry.result, Outcome::DryRun);
        assert_eq!(engine.specs(now)[0].look, Look::Error);
    }

    #[test]
    fn success_reverts_after_the_feedback_hold() {
        let mut engine = Engine::new(loaded(GLOBAL));
        engine.set_key_count(9);
        let now = Instant::now();
        let hold = engine.config().render.feedback_hold;
        let mut record = engine.press(slot(1), short(), SystemTime::UNIX_EPOCH);
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
    fn a_missing_page_shows_blank_keys() {
        let mut engine = Engine::new(loaded(
            "schema_version = 1\n[profile]\nid = \"global\"\nname = \"G\"\n",
        ));
        engine.set_key_count(9);
        assert_eq!(engine.specs(Instant::now()), vec![KeySpec::default(); 9]);
        let record = engine.press(slot(3), short(), SystemTime::UNIX_EPOCH);
        assert_eq!(record.result, Outcome::Unbound);
    }

    #[test]
    fn a_reload_replaces_the_keys_and_keeps_key_states() {
        let mut engine = Engine::new(loaded(GLOBAL));
        engine.set_key_count(9);
        let now = Instant::now();
        let mut record = engine.press(slot(1), short(), SystemTime::UNIX_EPOCH);
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
