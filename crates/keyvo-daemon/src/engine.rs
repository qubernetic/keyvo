//! The daemon's view of the pad: the loaded configuration, the active
//! profile and page, and the automatic state of every key. Synchronous and
//! free of I/O; the controller feeds it events and asks it what the keys
//! should show.
//!
//! Pre-alpha: the active profile is the global one on page 1. Focus
//! switching and the page buttons come with slice 11.

use std::collections::BTreeMap;
use std::time::{Instant, SystemTime};

use keyvo_core::config::Config;
use keyvo_core::feedback::{Feedback, Look};
use keyvo_core::layout::Loaded;
use keyvo_core::press;
use keyvo_core::profile::{GLOBAL_ID, Key, KeySlot, Profile};
use keyvo_core::render::{self, KeySpec};
use keyvo_core::trace::{Outcome, TraceRecord, Trigger};

/// The pad state behind every paint.
#[derive(Debug, Clone)]
pub struct Engine {
    loaded: Loaded,
    profile: String,
    page: usize,
    keys: u8,
    feedback: BTreeMap<KeySlot, Feedback>,
}

impl Engine {
    /// The global profile on page 1 of `loaded`, before any device told
    /// its key count.
    #[must_use]
    pub fn new(loaded: Loaded) -> Self {
        Self {
            loaded,
            profile: GLOBAL_ID.to_owned(),
            page: 1,
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

    /// Replaces the configuration after a reload. The active profile stays
    /// when it still exists, else the global one is active; a page past the
    /// profile's last falls back to page 1. Key states carry over.
    pub fn reload(&mut self, loaded: Loaded) {
        self.loaded = loaded;
        if self.loaded.profiles.get(&self.profile).is_none() {
            GLOBAL_ID.clone_into(&mut self.profile);
        }
        if self.page > self.active().pages.len() {
            self.page = 1;
        }
    }

    /// The active profile.
    #[must_use]
    pub fn active(&self) -> &Profile {
        self.loaded
            .profiles
            .get(&self.profile)
            .unwrap_or_else(|| self.loaded.profiles.global())
    }

    /// The active page, counted from 1.
    #[must_use]
    pub const fn page(&self) -> usize {
        self.page
    }

    fn key(&self, slot: KeySlot) -> Option<&Key> {
        self.page
            .checked_sub(1)
            .and_then(|index| self.active().pages.get(index))
            .and_then(|page| page.keys.get(&slot))
    }

    /// What every key shows at `now`, keys 1 to the device's key count. A
    /// profile without the active page shows blank keys.
    #[must_use]
    pub fn specs(&self, now: Instant) -> Vec<KeySpec> {
        let profile = self.active();
        let specs = render::page(profile, self.page, self.keys)
            .unwrap_or_else(|_| vec![KeySpec::default(); usize::from(self.keys)]);
        (1..=self.keys)
            .filter_map(KeySlot::new)
            .zip(specs)
            .map(|(slot, spec)| {
                let look = self
                    .feedback
                    .get(&slot)
                    .map_or(Look::Idle, |feedback| feedback.look(now));
                if look == Look::Idle {
                    spec
                } else {
                    spec.with_look(self.key(slot), look)
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

    fn feedback(&mut self, slot: KeySlot) -> &mut Feedback {
        let render = self.loaded.config.render;
        self.feedback
            .entry(slot)
            .or_insert_with(|| Feedback::new(&render))
    }

    /// Resolves a press of `slot` on the active profile and page; an error
    /// or success look of the key ends.
    pub fn press(&mut self, slot: KeySlot, trigger: Trigger, at: SystemTime) -> TraceRecord {
        self.feedback(slot).pressed();
        press::resolve(self.active(), self.page, slot, trigger, at)
    }

    /// The action of `slot` started running at `now`.
    pub fn started(&mut self, slot: KeySlot, now: Instant) {
        self.feedback(slot).started(now);
    }

    /// The action of `slot` ended at `now` with `outcome`.
    pub fn finished(&mut self, slot: KeySlot, outcome: &Outcome, now: Instant) {
        self.feedback(slot).finished(outcome, now);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::time::Duration;

    use keyvo_core::layout::Profiles;
    use keyvo_core::press::PressKind;
    use keyvo_core::profile::Action;

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
"##;

    fn loaded(text: &str) -> Loaded {
        let profile = Profile::from_toml(text, Path::new("global.toml")).unwrap();
        Loaded {
            config: Config::default(),
            profiles: Profiles::new(BTreeMap::from([(profile.profile.id.clone(), profile)])),
        }
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
        assert_eq!(specs, render::page(engine.active(), 1, 9).unwrap());
        assert_eq!(specs[0].label.as_deref(), Some("Build"));
        assert_eq!(engine.page(), 1);
    }

    #[test]
    fn a_failed_exec_shows_the_error_look_with_the_key_state() {
        let mut engine = Engine::new(loaded(GLOBAL));
        engine.set_key_count(9);
        let now = Instant::now();
        let record = engine.press(slot(1), short(), SystemTime::UNIX_EPOCH);
        assert_eq!(record.result, Outcome::Pending);
        assert!(matches!(record.action, Some(Action::Exec { .. })));
        engine.started(slot(1), now);
        assert_eq!(engine.specs(now)[0].look, Look::Running);
        let failed = Outcome::Exited {
            code: Some(1),
            duration_ms: 5,
        };
        engine.finished(slot(1), &failed, now);
        let spec = &engine.specs(now)[0];
        assert_eq!(spec.look, Look::Error);
        assert_eq!(spec.label.as_deref(), Some("Broken"));
        assert_eq!(engine.next_change(now), None);
        // The next press of the key ends the error look.
        engine.press(slot(1), short(), SystemTime::UNIX_EPOCH);
        assert_eq!(engine.specs(now)[0].look, Look::Idle);
    }

    #[test]
    fn success_reverts_after_the_feedback_hold() {
        let mut engine = Engine::new(loaded(GLOBAL));
        engine.set_key_count(9);
        let now = Instant::now();
        let hold = engine.config().render.feedback_hold;
        engine.started(slot(1), now);
        assert_eq!(
            engine.next_change(now),
            Some(now + engine.config().render.long_run_threshold)
        );
        let ok = Outcome::Exited {
            code: Some(0),
            duration_ms: 5,
        };
        engine.finished(slot(1), &ok, now);
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
        engine.finished(slot(1), &Outcome::Failed { error: "x".into() }, now);
        engine.reload(loaded(&GLOBAL.replace("\"Copy\"", "\"Paste\"")));
        let specs = engine.specs(now);
        assert_eq!(specs[1].label.as_deref(), Some("Paste"));
        assert_eq!(specs[0].look, Look::Error);
        assert_eq!(engine.active().profile.id, "global");
    }

    #[test]
    fn a_reload_without_the_active_profile_or_page_falls_back() {
        let mut engine = Engine::new(loaded(GLOBAL));
        engine.profile = "gone".into();
        engine.page = 4;
        engine.reload(Loaded {
            config: Config::default(),
            profiles: Profiles::new(BTreeMap::new()),
        });
        assert_eq!(engine.active().profile.id, GLOBAL_ID);
        assert_eq!(engine.page(), 1);
        assert_eq!(
            engine.next_change(Instant::now() + Duration::from_secs(1)),
            None
        );
    }
}
