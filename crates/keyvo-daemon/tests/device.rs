//! The device actor against `FakeHidTransport` pads: the claim, one
//! composite image per change (the union of the keys on a new page, a crop
//! of the changed keys otherwise), input, reconnect and stop.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use keyvo_core::config::Timing;
use keyvo_core::profile::Color;
use keyvo_core::render::{Canvas, KeyPlace, KeySpec, Renderer};
use keyvo_core::state::{Mode, View};
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

/// The report of a claimed Keypad of the captured geometry, which the
/// test pads publish.
fn connected() -> Event {
    Event::Connected {
        keys: 9,
        panel: display::captured_geometry().panel(),
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
        keys: keys.to_vec(),
        ..Frame::default()
    })
}

/// The canvas of a profile with `accent` on the captured geometry, with
/// `specs` drawn on keys 1 to n.
fn canvas_of(accent: Option<Color>, specs: &[KeySpec]) -> Canvas {
    let renderer = Renderer::new();
    let mut canvas = renderer
        .canvas(accent, &display::captured_geometry().panel())
        .unwrap();
    for (index, spec) in specs.iter().enumerate() {
        renderer.draw(&mut canvas, index, spec).unwrap();
    }
    canvas
}

/// The image of `place` cropped from `canvas`.
fn crop(canvas: &Canvas, place: KeyPlace) -> Image {
    Image {
        rect: (place.x, place.y, place.size.width, place.size.height),
        jpeg: canvas.crop(place).unwrap().jpeg().unwrap(),
    }
}

/// The one image of a page: the union of the key rectangles, 435x434 at
/// 23,6 on the captured geometry.
fn union_of(accent: Option<Color>, specs: &[KeySpec]) -> Image {
    let canvas = canvas_of(accent, specs);
    let union = canvas.panel().bounds(0..9).unwrap();
    assert_eq!(
        (union.x, union.y, union.size.width, union.size.height),
        (23, 6, 435, 434)
    );
    crop(&canvas, union)
}

/// Key `key`'s rectangle cropped from the canvas of `specs`.
fn key_of(accent: Option<Color>, specs: &[KeySpec], key: u8) -> Image {
    let canvas = canvas_of(accent, specs);
    let place = canvas.panel().keys[usize::from(key - 1)];
    crop(&canvas, place)
}

#[test]
fn the_actor_claims_paints_one_union_image_then_only_the_changed_key() {
    let pad = Pad::new(&startup());
    let actor = Actor::start(Pads::new([pad.clone()]));
    assert_eq!(actor.next(), connected());
    let mut specs = labelled(&["1", "2", "3", "4", "5", "6", "7", "8", "9"]);
    actor.commands.send(paint(&specs)).unwrap();
    assert_eq!(
        actor.next(),
        Event::Painted {
            keys: (1..=9).collect()
        }
    );
    assert!(pad.script_done());
    // One image: the background and every key, so nothing of an earlier
    // panel paint stays beside a key and nothing cascades.
    assert_eq!(pad.images(), [union_of(None, &specs)]);

    specs[2].background = Some(Color::rgb(0x2f, 0x6f, 0x3e));
    actor.commands.send(paint(&specs)).unwrap();
    assert_eq!(actor.next(), Event::Painted { keys: vec![3] });
    assert_eq!(pad.images()[1..], [key_of(None, &specs, 3)]);
    actor.stop();
}

#[test]
fn two_changed_keys_go_as_one_crop_of_their_bounds() {
    let pad = Pad::new(&startup());
    let actor = Actor::start(Pads::new([pad.clone()]));
    assert_eq!(actor.next(), connected());
    let mut specs = labelled(&["1", "2", "3", "4", "5", "6", "7", "8", "9"]);
    actor.commands.send(paint(&specs)).unwrap();
    assert!(matches!(actor.next(), Event::Painted { .. }));
    specs[0].label = Some("one".into());
    specs[4].label = Some("five".into());
    actor.commands.send(paint(&specs)).unwrap();
    assert_eq!(actor.next(), Event::Painted { keys: vec![1, 5] });
    let canvas = canvas_of(None, &specs);
    let bounds = canvas.panel().bounds([0, 4]).unwrap();
    assert_eq!(pad.images()[1..], [crop(&canvas, bounds)]);
    actor.stop();
}

#[test]
fn a_new_accent_paints_one_union_image_in_its_background() {
    let pad = Pad::new(&startup());
    let actor = Actor::start(Pads::new([pad.clone()]));
    assert_eq!(actor.next(), connected());
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
            ..Frame::default()
        }))
        .unwrap();
    assert_eq!(actor.next(), Event::Painted { keys: all });
    assert_eq!(pad.images()[1..], [union_of(accent, &tinted)]);
    actor.stop();
}

#[test]
fn a_new_view_paints_the_union_even_where_keys_stay_the_same() {
    let pad = Pad::new(&startup());
    let actor = Actor::start(Pads::new([pad.clone()]));
    assert_eq!(actor.next(), connected());
    let specs = labelled(&["1", "2", "3", "4", "5", "6", "7", "8", "9"]);
    let view = |page| View {
        profile: "global".into(),
        page,
        mode: Mode::Manual,
        index: 1,
        locked: false,
    };
    for page in [1, 2] {
        actor
            .commands
            .send(Command::Paint(Frame {
                view: Some(view(page)),
                keys: specs.clone(),
                ..Frame::default()
            }))
            .unwrap();
        assert_eq!(
            actor.next(),
            Event::Painted {
                keys: (1..=9).collect()
            }
        );
    }
    let union = union_of(None, &specs);
    assert_eq!(pad.images(), [union.clone(), union]);
    actor.stop();
}

#[test]
fn a_display_that_refuses_its_capabilities_is_still_painted() {
    // fn0 answered with a VLP error reply (the shape of
    // `probe-feature-error.txt`) instead of the capabilities.
    let capabilities = support::exchanges(&support::fixture("display-capabilities.txt"), 1..2);
    let request = capabilities.lines().next().unwrap();
    let refused = format!("{request}\n< 13 ff ff 02 0e 02{}\n", " 00".repeat(26));
    let script = startup().replace(&capabilities, &refused);
    assert_ne!(script, startup());
    let pad = Pad::new(&script);
    let actor = Actor::start(Pads::new([pad.clone()]));
    assert_eq!(actor.next(), connected());
    let specs = labelled(&["1", "2", "3"]);
    actor.commands.send(paint(&specs)).unwrap();
    assert_eq!(
        actor.next(),
        Event::Painted {
            keys: vec![1, 2, 3]
        }
    );
    assert!(pad.script_done());
    assert_eq!(pad.images(), [union_of(None, &specs)]);
    actor.stop();
}

#[test]
fn key_presses_and_releases_are_reported() {
    let pad = Pad::new(&startup());
    let actor = Actor::start(Pads::new([pad.clone()]));
    assert_eq!(actor.next(), connected());
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
    assert_eq!(actor.next(), connected());
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
    assert_eq!(actor.next(), connected());
    assert_eq!(
        actor.next(),
        Event::Painted {
            keys: vec![1, 2, 3]
        }
    );
    assert!(second.script_done());
    assert_eq!(second.images(), [union_of(None, &specs)]);
    actor.stop();
}

#[test]
fn a_replug_whose_claim_sees_the_splash_is_no_second_disconnect() {
    let first = Pad::new(&startup());
    let second = Pad::new(&support::startup_in_splash());
    assert_ne!(support::startup_in_splash(), startup());
    let actor = Actor::start(Pads::new([first.clone(), second.clone()]));
    assert_eq!(actor.next(), connected());
    let specs = labelled(&["a", "b", "c"]);
    actor.commands.send(paint(&specs)).unwrap();
    assert!(matches!(actor.next(), Event::Painted { .. }));
    first.unplug();
    assert_eq!(
        actor.next(),
        Event::Disconnected {
            error: "device disconnected".into()
        }
    );
    // The splash is waited out within the claim: no disconnect for it.
    assert_eq!(actor.next(), connected());
    assert_eq!(
        actor.next(),
        Event::Painted {
            keys: vec![1, 2, 3]
        }
    );
    assert!(second.script_done());
    assert_eq!(second.images(), [union_of(None, &specs)]);
    assert!(
        actor
            .events
            .recv_timeout(Duration::from_millis(100))
            .is_err()
    );
    actor.stop();
}

#[test]
fn a_lost_claim_is_reclaimed_once_and_the_union_repainted() {
    let pad = Pad::new(&(startup() + &support::fixture("display-reclaim.txt")));
    let actor = Actor::start(Pads::new([pad.clone()]));
    assert_eq!(actor.next(), connected());
    let specs = labelled(&["1", "2", "3", "4", "5", "6", "7", "8", "9"]);
    actor.commands.send(paint(&specs)).unwrap();
    let all: Vec<u8> = (1..=9).collect();
    assert_eq!(actor.next(), Event::Painted { keys: all.clone() });

    pad.send(&support::reports(&support::fixture("claim-lost.txt")));
    assert_eq!(actor.next(), Event::Reclaimed);
    assert_eq!(actor.next(), Event::Painted { keys: all });
    assert!(pad.script_done());
    // The same one image as on the claim.
    let union = union_of(None, &specs);
    assert_eq!(pad.images(), [union.clone(), union]);
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
    assert_eq!(actor.next(), connected());
    actor.stop();
}

#[test]
fn closing_the_command_channel_ends_the_actor() {
    let actor = Actor::start(Pads::new([Pad::new(&startup())]));
    assert_eq!(actor.next(), connected());
    drop(actor.commands);
    actor.thread.join().unwrap();
}
