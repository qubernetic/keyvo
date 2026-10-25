//! A subscription and dry-run presses against the shared socket fixtures
//! in `tests/protocol-fixtures/socket/`: what the desktop app's Home sees.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{self, Cursor, Read, Write};

use keyvo_client::{Client, ClientError, Update, follow};
use keyvo_core::press::PressKind;
use keyvo_core::socket::{Event, ServerMessage};
use keyvo_core::state::{Mode, View};
use serde_json::Value;

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

fn json(line: &str) -> Value {
    serde_json::from_str(line).unwrap()
}

/// `line` with its `id` set to `id`.
fn answering(line: &str, id: u64) -> String {
    let mut value = json(line);
    value["id"] = id.into();
    value.to_string()
}

fn event(line: &str) -> Event {
    match ServerMessage::parse(line).unwrap() {
        ServerMessage::Event(event) => event,
        ServerMessage::Response(_) => panic!("{line}"),
    }
}

/// A daemon that answers with a fixed script and records what it is sent.
struct Script {
    input: Cursor<Vec<u8>>,
    written: Vec<u8>,
}

impl Script {
    fn new(server_lines: &[&str]) -> Self {
        let mut text = server_lines.join("\n");
        text.push('\n');
        Self {
            input: Cursor::new(text.into_bytes()),
            written: Vec::new(),
        }
    }
}

impl Read for Script {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.input.read(buf)
    }
}

impl Write for Script {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.written.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

const SUBSCRIBED: usize = 2;
const DRY_RUN: usize = 3;
const DONE: usize = 5;

#[test]
fn follow_hands_on_the_view_then_every_event_then_why_it_ended() {
    let events = lines(fixture!("events.ndjson"));
    let answer = answering(lines(fixture!("responses.ndjson"))[SUBSCRIBED], 1);
    let mut script = vec![answer.as_str()];
    script.extend(&events);

    let mut got = Vec::new();
    follow(Ok(Client::new(Script::new(&script))), 20, |update| {
        got.push(update);
        true
    });

    let mut expected = vec![Update::Subscribed {
        view: View {
            profile: "global".into(),
            page: 1,
            mode: Mode::Auto,
        },
    }];
    expected.extend(
        events
            .iter()
            .map(|line| Update::Event { event: event(line) }),
    );
    expected.push(Update::Closed {
        reason: ClientError::Closed.to_string(),
    });
    assert_eq!(got, expected);
}

#[test]
fn follow_subscribes_to_every_class_with_the_replay() {
    let answer = answering(lines(fixture!("responses.ndjson"))[SUBSCRIBED], 1);
    let mut script = Script::new(&[&answer]);
    let client = Client::new(&mut script);
    follow(Ok(client), 20, |_| true);

    let written = String::from_utf8(script.written).unwrap();
    assert_eq!(
        json(&written),
        serde_json::json!({
            "v": 1,
            "id": 1,
            "cmd": "subscribe",
            "events": ["key", "profile", "page", "mode", "device", "context", "trace", "config"],
            "replay": 20,
        })
    );
}

#[test]
fn follow_stops_when_the_sink_says_so() {
    let events = lines(fixture!("events.ndjson"));
    let answer = answering(lines(fixture!("responses.ndjson"))[SUBSCRIBED], 1);
    let mut script = vec![answer.as_str()];
    script.extend(&events);

    let mut got = Vec::new();
    follow(Ok(Client::new(Script::new(&script))), 0, |update| {
        got.push(update);
        got.len() < 2
    });
    assert_eq!(got.len(), 2);
    assert_eq!(
        got[1],
        Update::Event {
            event: event(events[0])
        }
    );
}

#[test]
fn follow_without_a_daemon_says_why() {
    let dir = tempfile::tempdir().unwrap();
    let error = Client::connect(&dir.path().join("keyvo.sock")).unwrap_err();
    let reason = error.to_string();

    let mut got = Vec::new();
    follow(Err::<Client<Script>, _>(error), 20, |update| {
        got.push(update);
        true
    });
    assert_eq!(got, vec![Update::Closed { reason }]);
}

#[test]
fn follow_says_why_a_refused_subscribe_ended() {
    let refused = lines(fixture!("responses.ndjson"))
        .into_iter()
        .find(|line| !line.contains("\"id\""))
        .unwrap();

    let mut got = Vec::new();
    follow(Ok(Client::new(Script::new(&[refused]))), 20, |update| {
        got.push(update);
        true
    });
    let [Update::Closed { reason }] = got.as_slice() else {
        panic!("{got:?}");
    };
    assert!(reason.contains("refused"), "{reason}");
}

#[test]
fn follow_says_why_another_reply_to_subscribe_ended() {
    let answer = answering(lines(fixture!("responses.ndjson"))[DONE], 1);
    let mut got = Vec::new();
    follow(Ok(Client::new(Script::new(&[&answer]))), 20, |update| {
        got.push(update);
        true
    });
    assert_eq!(
        got,
        vec![Update::Closed {
            reason: ClientError::Unexpected("subscribe").to_string()
        }]
    );
}

#[test]
fn follow_stops_at_the_view_when_the_sink_says_so() {
    let answer = answering(lines(fixture!("responses.ndjson"))[SUBSCRIBED], 1);
    let mut got = Vec::new();
    follow(Ok(Client::new(Script::new(&[&answer]))), 20, |update| {
        got.push(update);
        false
    });
    assert!(
        matches!(got.as_slice(), [Update::Subscribed { .. }]),
        "{got:?}"
    );
}

#[test]
fn an_update_is_tagged_by_kind_with_the_wire_event_inside() {
    let line = lines(fixture!("events.ndjson"))[2];
    let update = serde_json::to_value(Update::Event { event: event(line) }).unwrap();
    let mut expected = json(line);
    expected.as_object_mut().unwrap().remove("v");
    assert_eq!(
        update,
        serde_json::json!({"kind": "event", "event": expected})
    );
    assert_eq!(
        serde_json::to_value(Update::Closed {
            reason: "gone".into()
        })
        .unwrap(),
        serde_json::json!({"kind": "closed", "reason": "gone"})
    );
}

#[test]
fn dry_run_sends_a_simulated_press_and_returns_the_entry() {
    let answer = answering(lines(fixture!("responses.ndjson"))[DRY_RUN], 1);
    let mut script = Script::new(&[&answer]);
    let mut client = Client::new(&mut script);

    let entry = client
        .dry_run(
            keyvo_core::profile::KeySlot::new(5).unwrap(),
            PressKind::Short,
        )
        .unwrap();
    assert!(entry.trigger.simulated);
    drop(client);
    assert_eq!(
        json(&String::from_utf8(script.written).unwrap()),
        serde_json::json!({"v": 1, "id": 1, "cmd": "press", "key": 5, "kind": "short", "dry_run": true})
    );
}

#[test]
fn dry_run_with_another_reply_is_unexpected() {
    let answer = answering(lines(fixture!("responses.ndjson"))[DONE], 1);
    let mut client = Client::new(Script::new(&[&answer]));
    let error = client
        .dry_run(
            keyvo_core::profile::KeySlot::new(5).unwrap(),
            PressKind::Long,
        )
        .unwrap_err();
    assert!(
        matches!(error, ClientError::Unexpected("press")),
        "{error:?}"
    );
}
