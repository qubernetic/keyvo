//! The layer stack (spec §6.1, §7.2): a key the active app profile does not
//! define falls back to the same key of the global profile's context page,
//! on the profiles of `tests/fixtures/dirs/fallback/`.

use std::path::Path;
use std::time::SystemTime;

use keyvo_core::layout::{self, Profiles};
use keyvo_core::press::{self, PressKind};
use keyvo_core::profile::{Action, KeySlot};
use keyvo_core::render::{self, KeySpec, RenderError};
use keyvo_core::stack::{Layer, Stack};
use keyvo_core::trace::{Outcome, Trigger};

fn profiles() -> Profiles {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dirs/fallback");
    layout::load(&dir).unwrap().profiles
}

fn slot(n: u8) -> KeySlot {
    KeySlot::new(n).unwrap()
}

const SHORT: Trigger = Trigger {
    press: PressKind::Short,
    simulated: false,
};

#[test]
fn a_key_the_app_profile_does_not_define_comes_from_the_global_context_page() {
    let profiles = profiles();
    let konsole = profiles.get("konsole").unwrap();
    let stack = Stack::new(&profiles, konsole, 1).unwrap();
    let (own, layer) = stack.key(slot(1)).unwrap();
    assert_eq!((own.label.as_deref(), layer), (Some("New tab"), Layer::App));
    let (fallen, layer) = stack.key(slot(3)).unwrap();
    assert_eq!(
        (fallen.label.as_deref(), layer),
        (Some("Type"), Layer::Global)
    );
    assert_eq!(stack.layer(slot(9)), Some(Layer::Global));
    // Neither layer defines key 4.
    assert!(stack.key(slot(4)).is_none());
    assert_eq!(stack.layer(slot(4)), None);
    // Every page of the app profile sits on the global context page.
    let second = Stack::new(&profiles, konsole, 2).unwrap();
    assert_eq!(
        second.key(slot(1)).unwrap().0.label.as_deref(),
        Some("Split")
    );
    assert_eq!(
        second.key(slot(3)).unwrap().0.label.as_deref(),
        Some("Type")
    );
}

#[test]
fn the_global_profile_has_no_layer_under_it() {
    let profiles = profiles();
    let general = Stack::new(&profiles, profiles.global(), 1).unwrap();
    assert_eq!(
        general
            .key(slot(3))
            .map(|(key, layer)| (key.label.as_deref(), layer)),
        Some((Some("Wrong"), Layer::Global))
    );
    assert!(general.key(slot(1)).is_none());
}

#[test]
fn a_stack_needs_the_page_of_its_profile() {
    let profiles = profiles();
    let konsole = profiles.get("konsole").unwrap();
    assert!(matches!(
        Stack::new(&profiles, konsole, 3),
        Err(RenderError::NoSuchPage {
            page: 3,
            pages: 2,
            ..
        })
    ));
    assert!(Stack::new(&profiles, konsole, 0).is_err());
}

#[test]
fn a_profile_alone_has_no_fallback() {
    let profiles = profiles();
    let konsole = profiles.get("konsole").unwrap();
    let alone = Stack::alone(konsole, 1).unwrap();
    assert!(alone.key(slot(3)).is_none());
    assert_eq!(alone.layer(slot(1)), Some(Layer::App));
}

#[test]
fn the_page_shows_the_global_key_with_the_app_accent() {
    let profiles = profiles();
    let konsole = profiles.get("konsole").unwrap();
    let specs = render::page(&Stack::new(&profiles, konsole, 1).unwrap(), 9);
    assert_eq!(specs.len(), 9);
    let global = profiles.global().pages[1].keys.get(&slot(3));
    // The accent is the context indicator: the app profile's, not the global one.
    assert_eq!(specs[2], KeySpec::from_key(global, konsole.profile.accent));
    assert_eq!(specs[2].label.as_deref(), Some("Type"));
    assert_eq!(specs[3], KeySpec::from_key(None, konsole.profile.accent));
    assert_eq!(specs[8].label.as_deref(), Some("Mute"));
}

#[test]
fn a_press_of_a_key_the_app_profile_lacks_runs_the_global_action() {
    let profiles = profiles();
    let konsole = profiles.get("konsole").unwrap();
    let stack = Stack::new(&profiles, konsole, 1).unwrap();
    let record = press::resolve(&stack, slot(3), SHORT, SystemTime::UNIX_EPOCH);
    assert_eq!(record.profile, "konsole");
    assert_eq!(record.page, 1);
    assert_eq!(record.layer, Some(Layer::Global));
    assert!(matches!(record.action, Some(Action::Chord { .. })));
    assert_eq!(record.result, Outcome::Pending);

    let own = press::resolve(&stack, slot(1), SHORT, SystemTime::UNIX_EPOCH);
    assert_eq!(own.layer, Some(Layer::App));
    let unbound = press::resolve(&stack, slot(4), SHORT, SystemTime::UNIX_EPOCH);
    assert_eq!((unbound.layer, unbound.result), (None, Outcome::Unbound));
}

#[test]
fn layers_have_their_spec_names() {
    assert_eq!(Layer::Global.as_str(), "global");
    assert_eq!(Layer::App.as_str(), "app");
    assert_eq!(serde_json::to_string(&Layer::App).unwrap(), "\"app\"");
}
