//! The device actor against `FakeHidTransport` pads: the claim, the panel
//! background before the keys, the paint of only the changed keys, input,
//! reconnect and stop.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use keyvo_core::config::Timing;
use keyvo_core::profile::Color;
use keyvo_core::render::{KeySpec, Renderer, TileSize};
use keyvo_daemon::device::{self, Command, Connector, Event, Frame, Settings};
use keyvo_hid::display;
use keyvo_hid::input::Control;
use keyvo_hid::input::InputEvent;

use support::{Image, Pad, Pads, startup};

/// How long a test waits for one event before it fails.
const PATIENCE: Duration = Duration::from_secs(10);

fn settings() -> Settings {
    Settings::from_timing(&Timing {
        divert_settle: Duration::ZERO,
        divert_spacing: Duration::ZERO,
        render_coalesce: Duration::from_millis(5),
        reconnect_interval: Duration::from_millis(20),
        ..Timing::default()
    })
}

struct Actor {
    commands: Sender<Command>,
    events: Receiver<Event>,
    thread: JoinHandle<()>,
}

impl Actor {
    fn start(connector: impl Connector) -> Self {
        let (commands, inbox) = mpsc::channel();
        let (outbox, events) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            device::run(connector, &settings(), &inbox, |event| {
                let _ = outbox.send(event);
            });
        });
        Self {
            commands,
            events,
            thread,
        }
    }

    fn next(&self) -> Event {
        self.events.recv_timeout(PATIENCE).expect("an event")
    }

    fn stop(self) {
        self.commands.send(Command::Stop).unwrap();
        self.thread.join().unwrap();
    }
}

fn labelled(labels: &[&str]) -> Vec<KeySpec> {
    labels
        .iter()
        .map(|label| KeySpec {
            label: Some((*label).to_owned()),
            ..KeySpec::default()
        })
        .collect()
}

/// A paint request for `keys` of a profile without an accent.
fn paint(keys: &[KeySpec]) -> Command {
    Command::Paint(Frame {
        accent: None,
        keys: keys.to_vec(),
    })
}

/// The image of `spec` on key `key` of the captured geometry, on the panel
/// background of the spec's accent.
fn image_of(key: u8, spec: &KeySpec) -> Image {
    let geometry = display::captured_geometry();
    let rect = geometry.keys[usize::from(key - 1)];
    let size = TileSize {
        width: rect.width,
        height: rect.height,
    };
    let renderer = Renderer::new();
    let under = renderer.panel_color(spec.accent);
    Image {
        rect: (rect.x, rect.y, rect.width, rect.height),
        jpeg: renderer.tile(spec, size).unwrap().jpeg_on(under).unwrap(),
    }
}

/// The panel background of a profile with `accent` on the captured
/// geometry: the bounding box of the keys, 435x434 at 23,6.
fn backdrop_of(accent: Option<Color>) -> Image {
    let size = TileSize {
        width: 435,
        height: 434,
    };
    Image {
        rect: (23, 6, 435, 434),
        jpeg: Renderer::new()
            .backdrop(accent, size)
            .unwrap()
            .jpeg()
            .unwrap(),
    }
}

#[test]
fn the_actor_claims_paints_the_panel_every_key_then_only_the_changed_one() {
    let pad = Pad::new(&startup());
    let actor = Actor::start(Pads::new([pad.clone()]));
    assert_eq!(actor.next(), Event::Connected { keys: 9 });
    let mut specs = labelled(&["1", "2", "3", "4", "5", "6", "7", "8", "9"]);
    actor.commands.send(paint(&specs)).unwrap();
    assert_eq!(
        actor.next(),
        Event::Painted {
            keys: (1..=9).collect()
        }
    );
    assert!(pad.script_done());
    let images = pad.images();
    assert_eq!(images.len(), 10);
    // The panel background goes first, so no earlier panel content stays
    // beside a key.
    assert_eq!(images[0], backdrop_of(None));
    assert_eq!(images[5], image_of(5, &specs[4]));

    specs[2].background = Some(Color::rgb(0x2f, 0x6f, 0x3e));
    actor.commands.send(paint(&specs)).unwrap();
    assert_eq!(actor.next(), Event::Painted { keys: vec![3] });
    assert_eq!(pad.images()[10..], [image_of(3, &specs[2])]);
    actor.stop();
}

#[test]
fn a_new_accent_paints_its_panel_background_then_every_key() {
    let pad = Pad::new(&startup());
    let actor = Actor::start(Pads::new([pad.clone()]));
    assert_eq!(actor.next(), Event::Connected { keys: 9 });
    let specs = labelled(&["1", "2", "3", "4", "5", "6", "7", "8", "9"]);
    actor.commands.send(paint(&specs)).unwrap();
    let all: Vec<u8> = (1..=9).collect();
    assert_eq!(actor.next(), Event::Painted { keys: all.clone() });

    // Another profile: the same keys, another accent.
    let accent = Some(Color::rgb(0x4e, 0x8b, 0xd6));
    let tinted: Vec<KeySpec> = specs
        .iter()
        .map(|spec| KeySpec {
            accent,
            ..spec.clone()
        })
        .collect();
    actor
        .commands
        .send(Command::Paint(Frame {
            accent,
            keys: tinted.clone(),
        }))
        .unwrap();
    assert_eq!(actor.next(), Event::Painted { keys: all });
    let images = pad.images();
    assert_eq!(images.len(), 20);
    assert_eq!(images[10], backdrop_of(accent));
    assert_eq!(images[11], image_of(1, &tinted[0]));
    assert_eq!(images[19], image_of(9, &tinted[8]));
    actor.stop();
}

#[test]
fn key_presses_and_releases_are_reported() {
    let pad = Pad::new(&startup());
    let actor = Actor::start(Pads::new([pad.clone()]));
    assert_eq!(actor.next(), Event::Connected { keys: 9 });
    pad.hold(&[5]);
    assert_eq!(
        actor.next(),
        Event::Input(InputEvent::press(Control::Key(5)))
    );
    pad.hold(&[]);
    assert_eq!(
        actor.next(),
        Event::Input(InputEvent::release(Control::Key(5)))
    );
    actor.stop();
}

#[test]
fn a_lost_device_is_reopened_reclaimed_and_repainted() {
    let first = Pad::new(&startup());
    let second = Pad::new(&startup());
    let actor = Actor::start(Pads::new([first.clone(), second.clone()]));
    assert_eq!(actor.next(), Event::Connected { keys: 9 });
    let specs = labelled(&["a", "b", "c"]);
    actor.commands.send(paint(&specs)).unwrap();
    assert_eq!(
        actor.next(),
        Event::Painted {
            keys: vec![1, 2, 3]
        }
    );
    first.unplug();
    assert_eq!(
        actor.next(),
        Event::Disconnected {
            error: "device disconnected".into()
        }
    );
    assert_eq!(actor.next(), Event::Connected { keys: 9 });
    assert_eq!(
        actor.next(),
        Event::Painted {
            keys: vec![1, 2, 3]
        }
    );
    assert!(second.script_done());
    let images = second.images();
    assert_eq!(images[0], backdrop_of(None));
    assert_eq!(images[1], image_of(1, &specs[0]));
    actor.stop();
}

#[test]
fn a_lost_claim_is_reclaimed_once_and_the_panel_and_every_key_repainted() {
    let pad = Pad::new(&(startup() + &support::fixture("display-reclaim.txt")));
    let actor = Actor::start(Pads::new([pad.clone()]));
    assert_eq!(actor.next(), Event::Connected { keys: 9 });
    let specs = labelled(&["1", "2", "3", "4", "5", "6", "7", "8", "9"]);
    actor.commands.send(paint(&specs)).unwrap();
    let all: Vec<u8> = (1..=9).collect();
    assert_eq!(actor.next(), Event::Painted { keys: all.clone() });

    pad.send(&support::reports(&support::fixture("claim-lost.txt")));
    assert_eq!(actor.next(), Event::Reclaimed);
    assert_eq!(actor.next(), Event::Painted { keys: all });
    assert!(pad.script_done());
    let images = pad.images();
    assert_eq!(images.len(), 20);
    // The panel background first, then the keys, as on the claim.
    assert_eq!(images[10], backdrop_of(None));
    assert_eq!(images[10..], images[..10]);
    // The twin event gives no second reclaim.
    assert!(
        actor
            .events
            .recv_timeout(Duration::from_millis(100))
            .is_err()
    );
    actor.stop();
}

#[test]
fn a_missing_device_is_reported_once_and_paints_wait_for_it() {
    let actor = Actor::start(Pads::new([]));
    assert_eq!(
        actor.next(),
        Event::Disconnected {
            error: "cannot open the keypad: hid i/o error: no keypad".into()
        }
    );
    actor.commands.send(paint(&labelled(&["x"]))).unwrap();
    // Further attempts fail the same way and report nothing new.
    assert!(
        actor
            .events
            .recv_timeout(Duration::from_millis(100))
            .is_err()
    );
    actor.stop();
}

#[test]
fn a_failed_claim_is_reported_and_retried() {
    // The keep-alive start only; the claim's first request is unexpected.
    let broken = Pad::new(&support::exchanges(
        &support::fixture("keep-alive-ping.txt"),
        0..1,
    ));
    let good = Pad::new(&startup());
    let actor = Actor::start(Pads::new([broken, good]));
    match actor.next() {
        Event::Disconnected { error } => assert!(error.contains("unexpected write"), "{error}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(actor.next(), Event::Connected { keys: 9 });
    actor.stop();
}

#[test]
fn closing_the_command_channel_ends_the_actor() {
    let actor = Actor::start(Pads::new([Pad::new(&startup())]));
    assert_eq!(actor.next(), Event::Connected { keys: 9 });
    drop(actor.commands);
    actor.thread.join().unwrap();
}
