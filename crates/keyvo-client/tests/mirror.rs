//! The mirror folds the daemon's answers and the fixture events of
//! `tests/protocol-fixtures/socket/` into what the pad shows.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use keyvo_client::Mirror;
use keyvo_core::profile::{KeySlot, ModifierMode};
use keyvo_core::socket::{Event, Reply, Response, ServerMessage};
use keyvo_core::state::{ActiveModifier, Mode, View};
use keyvo_core::trace::TraceRecord;

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!("../../../tests/protocol-fixtures/socket/", $name))
    };
}

fn lines(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .collect()
}

fn events() -> Vec<Event> {
    lines(fixture!("events.ndjson"))
        .into_iter()
        .map(|line| match ServerMessage::parse(line).unwrap() {
            ServerMessage::Event(event) => event,
            ServerMessage::Response(_) => panic!("{line}"),
        })
        .collect()
}

fn reply(index: usize) -> Reply {
    let line = lines(fixture!("responses.ndjson"))[index];
    serde_json::from_str::<Response>(line)
        .unwrap()
        .result
        .unwrap()
}

fn view(profile: &str, page: usize, index: isize, mode: Mode) -> View {
    View {
        profile: profile.into(),
        page,
        mode,
        index,
        locked: false,
        modifier: None,
    }
}

#[test]
fn an_empty_mirror_knows_nothing() {
    let mirror = Mirror::new(10);
    assert_eq!(mirror.view(), None);
    assert_eq!(mirror.device(), None);
    assert_eq!(mirror.context(), None);
    assert!(mirror.trace().is_empty());
    assert!(mirror.held().is_empty());
    assert_eq!(mirror.config_error(), None);
}

#[test]
fn status_fills_the_view_the_device_and_the_context() {
    let mut mirror = Mirror::new(10);
    mirror.answer(&reply(0));
    assert_eq!(mirror.view(), Some(&view("terminal", 1, 1, Mode::Auto)));
    assert_eq!(mirror.device().unwrap().keys, Some(9));
    assert_eq!(mirror.context().unwrap().class, "org.kde.konsole");

    mirror.answer(&reply(1));
    assert!(!mirror.device().unwrap().connected);
    assert_eq!(mirror.context(), None, "a status without context clears it");
}

#[test]
fn subscribed_sets_the_view_and_a_press_adds_its_entry() {
    let mut mirror = Mirror::new(10);
    mirror.answer(&reply(2));
    assert_eq!(mirror.view(), Some(&view("global", 1, 1, Mode::Auto)));

    mirror.answer(&reply(3));
    assert_eq!(mirror.trace().len(), 1);
    assert!(mirror.trace()[0].trigger.simulated);

    mirror.answer(&reply(5));
    assert_eq!(mirror.view(), Some(&view("global", 1, 1, Mode::Auto)));
}

#[test]
fn every_fixture_event_lands_in_the_mirror() {
    let mut mirror = Mirror::new(10);
    let events = events();
    let mut changed = Vec::new();
    for event in &events {
        changed.push(mirror.apply(event));
    }
    assert!(
        changed.iter().all(|c| *c),
        "every fixture event changes something"
    );

    assert_eq!(mirror.view(), Some(&view("global", 1, 1, Mode::Manual)));
    let device = mirror.device().unwrap();
    assert!(!device.connected);
    assert_eq!(device.error.as_deref(), Some("device disconnected"));
    // The last context is the keyvo app's, holding the window before it.
    let context = mirror.context().unwrap();
    assert_eq!((context.class.as_str(), context.held), ("keyvo-app", true));
    assert_eq!(mirror.holding().unwrap().class, "org.gnome.Ptyxis");
    let trace_events = events
        .iter()
        .filter(|e| matches!(e, Event::Trace { .. }))
        .count();
    assert_eq!(mirror.trace().len(), trace_events);
    assert!(mirror.held().is_empty(), "key 5 went down and up");
    assert!(
        mirror
            .config_error()
            .unwrap()
            .starts_with("profiles/global.toml:8:9")
    );
}

#[test]
fn a_session_event_locks_and_unlocks_the_known_view() {
    let mut mirror = Mirror::new(10);
    let locked = Event::Session { locked: true };
    assert!(!mirror.apply(&locked), "no view to lock yet");
    assert_eq!(mirror.view(), None);
    mirror.answer(&reply(2));
    assert!(mirror.apply(&locked));
    assert!(mirror.view().unwrap().locked);
    assert!(!mirror.apply(&locked));
    assert!(mirror.apply(&Event::Session { locked: false }));
    assert_eq!(mirror.view(), Some(&view("global", 1, 1, Mode::Auto)));
}

#[test]
fn a_held_context_holds_the_window_reported_before_it() {
    let mut mirror = Mirror::new(10);
    let events = events();
    let context = |class: &str| {
        events
            .iter()
            .find(|e| matches!(e, Event::Context(c) if c.class == class))
            .unwrap()
    };
    let app = context("keyvo-app");
    // Nothing reported before the keyvo app: it holds no known window.
    assert!(mirror.apply(app));
    assert_eq!(mirror.holding(), None);
    assert!(mirror.apply(context("org.kde.konsole")));
    assert_eq!(mirror.holding(), None);
    assert!(mirror.apply(app));
    assert_eq!(mirror.holding().unwrap().class, "org.kde.konsole");
    // A status during the hold keeps the window held over.
    mirror.answer(&reply(0));
    assert_eq!(mirror.holding(), None, "the status context is not held");
    assert!(mirror.apply(app));
    let Reply::Status(mut status) = reply(0) else {
        panic!("not a status")
    };
    status.context = Some(match app {
        Event::Context(c) => c.clone(),
        _ => unreachable!(),
    });
    mirror.answer(&Reply::Status(status));
    assert_eq!(mirror.holding().unwrap().class, "org.kde.konsole");
    assert!(mirror.apply(context("org.gnome.Ptyxis")));
    assert_eq!(mirror.holding(), None);
}

#[test]
fn a_held_key_shows_until_it_is_released() {
    let mut mirror = Mirror::new(10);
    let events = events();
    mirror.apply(&events[0]);
    assert_eq!(mirror.held(), [KeySlot::new(5).unwrap()]);
    assert!(!mirror.apply(&events[0]), "a second press changes nothing");
    mirror.apply(&events[1]);
    assert!(mirror.held().is_empty());
}

#[test]
fn a_good_reload_clears_the_config_error() {
    let mut mirror = Mirror::new(10);
    let events = events();
    let (good, bad) = (&events[events.len() - 2], &events[events.len() - 1]);
    mirror.apply(bad);
    assert!(mirror.config_error().is_some());
    mirror.apply(good);
    assert_eq!(mirror.config_error(), None);
}

#[test]
fn the_trace_keeps_the_newest_entries() {
    let mut mirror = Mirror::new(2);
    let traces: Vec<Event> = events()
        .into_iter()
        .filter(|e| matches!(e, Event::Trace { .. }))
        .collect();
    for event in &traces {
        mirror.apply(event);
    }
    let kept: Vec<&TraceRecord> = mirror.trace().iter().collect();
    let expected: Vec<&TraceRecord> = traces[traces.len() - 2..]
        .iter()
        .map(|e| match e {
            Event::Trace { entry } => entry,
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(kept, expected);
}

#[test]
fn a_modifier_event_sets_the_known_views_modifier() {
    let mut mirror = Mirror::new(10);
    let held = Event::Modifier {
        modifier: Some(ActiveModifier {
            key: KeySlot::new(7).unwrap(),
            mode: ModifierMode::Held,
        }),
    };
    assert!(!mirror.apply(&held), "no view yet");
    mirror.answer(&reply(2));
    assert!(mirror.apply(&held));
    assert_eq!(
        mirror
            .view()
            .unwrap()
            .modifier
            .map(|modifier| modifier.key.get()),
        Some(7)
    );
    assert!(!mirror.apply(&held));
    assert!(mirror.apply(&Event::Modifier { modifier: None }));
    assert_eq!(mirror.view(), Some(&view("global", 1, 1, Mode::Auto)));
}
