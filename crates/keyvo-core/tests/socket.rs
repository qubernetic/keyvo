//! The socket protocol types against the shared fixtures in
//! `tests/protocol-fixtures/socket/`: every line parses and writes back to
//! the same JSON.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use keyvo_core::press::PressKind;
use keyvo_core::profile::KeySlot;
use keyvo_core::socket::{
    Command, ErrorCode, Event, EventClass, Reply, Request, Response, ServerMessage,
};
use keyvo_core::state::{Mode, View};
use serde_json::Value;

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!("../../../tests/protocol-fixtures/socket/", $name))
    };
}

fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines().filter(|line| !line.trim().is_empty())
}

fn json(line: &str) -> Value {
    serde_json::from_str(line).unwrap()
}

#[test]
fn every_request_round_trips_its_fixture() {
    for line in lines(fixture!("requests.ndjson")) {
        let request = Request::parse(line).unwrap_or_else(|e| panic!("{line}: {e:?}"));
        assert_eq!(json(&request.to_line()), json(line), "{line}");
        assert!(!request.to_line().contains('\n'));
    }
}

/// The variant name of a response, `error` for a failed one.
fn kind(response: &Response) -> &'static str {
    match &response.result {
        Ok(Reply::Status(_)) => "status",
        Ok(Reply::Subscribed(_)) => "subscribed",
        Ok(Reply::Pressed(_)) => "pressed",
        Ok(Reply::Done {}) => "done",
        Err(_) => "error",
    }
}

#[test]
fn every_response_round_trips_its_fixture() {
    let mut kinds = Vec::new();
    for line in lines(fixture!("responses.ndjson")) {
        let response: Response =
            serde_json::from_str(line).unwrap_or_else(|e| panic!("{line}: {e}"));
        assert_eq!(json(&response.to_line()), json(line), "{line}");
        assert_eq!(
            ServerMessage::parse(line).unwrap(),
            ServerMessage::Response(response.clone())
        );
        kinds.push(kind(&response));
    }
    assert_eq!(
        kinds,
        [
            "status",
            "status",
            "subscribed",
            "pressed",
            "pressed",
            "done",
            "error",
            "done",
            "error",
            "error",
            "error",
            "error"
        ]
    );
}

/// The line with a field `later` added to every object in it, as a later
/// daemon might send it.
fn with_unknown_fields(line: &str) -> String {
    fn add(value: &mut Value) {
        match value {
            Value::Object(object) => {
                object.values_mut().for_each(add);
                object.insert("later".into(), 1.into());
            }
            Value::Array(items) => items.iter_mut().for_each(add),
            _ => {}
        }
    }
    let mut value = json(line);
    add(&mut value);
    value.to_string()
}

/// A later daemon may add fields to a response; a v1 client ignores them.
#[test]
fn a_response_with_unknown_fields_still_parses_to_its_reply() {
    for line in lines(fixture!("responses.ndjson")) {
        let known: Response = serde_json::from_str(line).unwrap();
        let line = with_unknown_fields(line);
        let response: Response =
            serde_json::from_str(&line).unwrap_or_else(|e| panic!("{line}: {e}"));
        assert_eq!(response, known, "{line}");
        assert_eq!(
            ServerMessage::parse(&line).unwrap(),
            ServerMessage::Response(known)
        );
    }
}

/// The same holds for events.
#[test]
fn an_event_with_unknown_fields_still_parses() {
    for line in lines(fixture!("events.ndjson")) {
        let known = ServerMessage::parse(line).unwrap();
        let line = with_unknown_fields(line);
        let event = ServerMessage::parse(&line).unwrap_or_else(|e| panic!("{line}: {e}"));
        assert_eq!(event, known, "{line}");
    }
}

#[test]
fn a_response_needs_the_version_and_one_of_result_and_error() {
    assert!(serde_json::from_str::<Response>(r#"{"v":2,"ok":true,"result":{}}"#).is_err());
    assert!(serde_json::from_str::<Response>(r#"{"v":1,"ok":true}"#).is_err());
    assert!(
        serde_json::from_str::<Response>(
            r#"{"v":1,"ok":false,"result":{},"error":{"code":"invalid_json","message":"x"}}"#
        )
        .is_err()
    );
    assert_eq!(
        ServerMessage::parse(r#"{"v":2,"event":"config","reloaded":true}"#)
            .unwrap_err()
            .code,
        ErrorCode::UnsupportedVersion
    );
    assert!(ServerMessage::parse(r#"{"v":1,"event":"nope"}"#).is_err());
}

#[test]
fn a_switch_is_one_event_per_changed_class() {
    let view = |profile: &str, page, mode| View {
        profile: profile.into(),
        page,
        mode,
    };
    let before = view("global", 1, Mode::Auto);
    assert!(Event::switched(&before, &before).is_empty());
    let after = view("terminal", 2, Mode::Manual);
    assert_eq!(
        Event::switched(&before, &after),
        [
            Event::Profile(after.clone()),
            Event::Page(after.clone()),
            Event::Mode(after.clone())
        ]
    );
    let classes: Vec<_> = Event::switched(&before, &view("global", 2, Mode::Auto))
        .iter()
        .map(Event::class)
        .collect();
    assert_eq!(classes, [EventClass::Page]);
}

#[test]
fn every_event_round_trips_its_fixture() {
    let mut classes = Vec::new();
    for line in lines(fixture!("events.ndjson")) {
        let event = match ServerMessage::parse(line).unwrap_or_else(|e| panic!("{line}: {e}")) {
            ServerMessage::Event(event) => event,
            ServerMessage::Response(_) => panic!("{line}: not an event"),
        };
        assert_eq!(json(&event.to_line()), json(line), "{line}");
        classes.push(event.class());
    }
    // The fixtures hold every event class.
    for class in EventClass::ALL {
        assert!(classes.contains(&class), "{class:?}");
    }
}

#[test]
fn left_out_fields_take_their_defaults() {
    let request = Request::parse(r#"{"v":1,"cmd":"press","key":3}"#).unwrap();
    assert_eq!(request.id, None);
    assert_eq!(
        request.command,
        Command::Press {
            key: KeySlot::new(3).unwrap(),
            kind: PressKind::Short,
            dry_run: false,
        }
    );
    let request = Request::parse(r#"{"v":1,"cmd":"subscribe"}"#).unwrap();
    assert_eq!(
        request.command,
        Command::Subscribe {
            events: EventClass::ALL.to_vec(),
            replay: 0,
        }
    );
    let request = Request::parse(r#"{"v":1,"cmd":"notify_context","class":"kitty"}"#).unwrap();
    assert_eq!(
        request.command,
        Command::NotifyContext {
            class: "kitty".into(),
            title: String::new(),
        }
    );
}

/// The error code and message `line` is rejected with, and the id it
/// carried.
fn rejected(line: &str) -> (Option<Value>, ErrorCode, String) {
    let error = Request::parse(line).unwrap_err();
    (error.id, error.code, error.message)
}

#[test]
fn a_bad_request_is_rejected_with_its_id_and_a_code() {
    assert_eq!(rejected("not json").1, ErrorCode::InvalidJson);
    assert_eq!(rejected("[1]").1, ErrorCode::InvalidJson);
    assert_eq!(
        rejected(r#"{"v":2,"id":7,"cmd":"status"}"#),
        (
            Some(Value::from(7)),
            ErrorCode::UnsupportedVersion,
            "protocol version 2 is not supported; this daemon speaks 1".into()
        )
    );
    assert_eq!(
        rejected(r#"{"id":7,"cmd":"status"}"#).1,
        ErrorCode::UnsupportedVersion
    );
    assert_eq!(
        rejected(r#"{"v":1,"id":8,"cmd":"press","key":5,"dryrun":true}"#),
        (
            Some(Value::from(8)),
            ErrorCode::InvalidRequest,
            "unknown field `dryrun`, expected one of `key`, `kind`, `dry_run`".into()
        )
    );
    // A typo must never turn a dry run into a real press, nor reach a
    // command without fields.
    assert_eq!(
        rejected(r#"{"v":1,"cmd":"status","verbose":true}"#).1,
        ErrorCode::InvalidRequest
    );
    assert_eq!(
        rejected(r#"{"v":1,"cmd":"press","key":10}"#).1,
        ErrorCode::InvalidRequest
    );
    assert_eq!(
        rejected(r#"{"v":1,"cmd":"press","key":"5"}"#).1,
        ErrorCode::InvalidRequest
    );
    assert_eq!(
        rejected(r#"{"v":1,"cmd":"set_key"}"#).1,
        ErrorCode::InvalidRequest
    );
    assert_eq!(rejected(r#"{"v":1}"#).1, ErrorCode::InvalidRequest);
}

#[test]
fn an_error_response_carries_the_rejected_id() {
    let error = Request::parse(r#"{"v":1,"id":"a","cmd":"nope"}"#).unwrap_err();
    let response = Response::error(error.id.clone(), error.code, &error.message);
    let value = json(&response.to_line());
    assert_eq!(value["id"], "a");
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "invalid_request");
}
