//! The device actor against `FakeHidTransport` pads: the claim, the paint
//! of only the changed keys, input, reconnect and stop.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use keyvo_core::config::Timing;
use keyvo_core::profile::Color;
use keyvo_core::render::{KeySpec, Renderer, TileSize};
use keyvo_daemon::device::{self, Command, Connector, Event, Settings};
use keyvo_hid::display;
use keyvo_hid::input::Control;
use keyvo_hid::input::InputEvent;

use support::{Image, Pad, Pads, startup};

/// How long a test waits for one event before it fails.
const PATIENCE: Duration = Duration::from_secs(10);

fn settings() -> Settings {
    Settings::from_timing(&Timing {
        keep_alive_period: Duration::from_secs(3600),
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

/// The image of `spec` on key `key` of the captured geometry.
fn image_of(key: u8, spec: &KeySpec) -> Image {
    let geometry = display::captured_geometry();
    let rect = geometry.keys[usize::from(key - 1)];
    let size = TileSize {
        width: rect.width,
        height: rect.height,
    };
    Image {
        rect: (rect.x, rect.y, rect.width, rect.height),
        jpeg: Renderer::new().tile(spec, size).unwrap().jpeg().unwrap(),
    }
}

#[test]
fn the_actor_claims_paints_every_key_then_only_the_changed_one() {
    let pad = Pad::new(&startup());
    let actor = Actor::start(Pads::new([pad.clone()]));
    assert_eq!(actor.next(), Event::Connected { keys: 9 });
    let mut specs = labelled(&["1", "2", "3", "4", "5", "6", "7", "8", "9"]);
    actor.commands.send(Command::Paint(specs.clone())).unwrap();
    assert_eq!(
        actor.next(),
        Event::Painted {
            keys: (1..=9).collect()
        }
    );
    assert!(pad.script_done());
    let images = pad.images();
    assert_eq!(images.len(), 9);
    assert_eq!(images[4], image_of(5, &specs[4]));

    specs[2].background = Some(Color::rgb(0x2f, 0x6f, 0x3e));
    actor.commands.send(Command::Paint(specs.clone())).unwrap();
    assert_eq!(actor.next(), Event::Painted { keys: vec![3] });
    assert_eq!(pad.images()[9..], [image_of(3, &specs[2])]);
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
    actor.commands.send(Command::Paint(specs.clone())).unwrap();
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
    assert_eq!(second.images()[0], image_of(1, &specs[0]));
    actor.stop();
}

#[test]
fn a_lost_claim_is_reclaimed_once_and_every_key_repainted() {
    let pad = Pad::new(&(startup() + &support::fixture("display-reclaim.txt")));
    let actor = Actor::start(Pads::new([pad.clone()]));
    assert_eq!(actor.next(), Event::Connected { keys: 9 });
    let specs = labelled(&["1", "2", "3", "4", "5", "6", "7", "8", "9"]);
    actor.commands.send(Command::Paint(specs.clone())).unwrap();
    let all: Vec<u8> = (1..=9).collect();
    assert_eq!(actor.next(), Event::Painted { keys: all.clone() });

    pad.send(&support::reports(&support::fixture("claim-lost.txt")));
    assert_eq!(actor.next(), Event::Reclaimed);
    assert_eq!(actor.next(), Event::Painted { keys: all });
    assert!(pad.script_done());
    let images = pad.images();
    assert_eq!(images[9..], images[..9]);
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
    actor
        .commands
        .send(Command::Paint(labelled(&["x"])))
        .unwrap();
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
        0..2,
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
