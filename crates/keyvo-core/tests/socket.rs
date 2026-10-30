//! The socket protocol types against the shared fixtures in
//! `tests/protocol-fixtures/socket/`: every line parses and writes back to
//! the same JSON.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use keyvo_core::dialpad::Control;
use keyvo_core::press::PressKind;
use keyvo_core::profile::{KeySlot, ModifierMode};
use keyvo_core::render::{KeyPlace, Panel, TileSize};
use keyvo_core::socket::{
    Command, ErrorCode, Event, EventClass, Geometry, Rect, Reply, Request, Response, ServerMessage,
};
use keyvo_core::state::{ActiveModifier, Mode, View};
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
        Ok(Reply::Recipes(_)) => "recipes",
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
            "error",
            "status",
            "pressed"
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

/// `recipes.ndjson`: the request, then the answers to it. The file stays
/// apart from `requests.ndjson` until the app speaks `recipes` (#129).
#[test]
fn the_recipes_request_and_its_answers_round_trip() {
    let mut lines = lines(fixture!("recipes.ndjson"));
    let request = Request::parse(lines.next().unwrap()).unwrap();
    assert_eq!(request.command, Command::Recipes {});
    assert_eq!(
        json(&request.to_line()),
        json(r#"{"v":1,"id":9,"cmd":"recipes"}"#)
    );
    let mut problems = Vec::new();
    for line in lines {
        let response: Response =
            serde_json::from_str(line).unwrap_or_else(|e| panic!("{line}: {e}"));
        assert_eq!(json(&response.to_line()), json(line), "{line}");
        let Ok(Reply::Recipes(recipes)) = &response.result else {
            panic!("{line}: {response:?}");
        };
        assert!(!recipes.recipes.is_empty(), "{line}");
        problems.extend(recipes.problems.clone());
        let later: Response = serde_json::from_str(&with_unknown_fields(line)).unwrap();
        assert_eq!(later, response, "{line}");
    }
    assert_eq!(problems.len(), 1, "one answer names a broken pack");
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
        index: 1,
        locked: false,
        modifier: None,
    };
    let before = view("global", 1, Mode::Auto);
    assert!(Event::switched(&before, &before).is_empty());
    // Another list position is another page, even on the same page.
    let moved = View {
        index: 0,
        locked: false,
        ..before.clone()
    };
    assert_eq!(
        Event::switched(&before, &moved),
        [Event::Page(moved.clone())]
    );
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
fn a_lock_or_unlock_is_a_session_event() {
    let before = View {
        profile: "global".into(),
        page: 1,
        mode: Mode::Auto,
        index: 1,
        locked: false,
        modifier: None,
    };
    let locked = View {
        locked: true,
        ..before.clone()
    };
    assert_eq!(
        Event::switched(&before, &locked),
        [Event::Session { locked: true }]
    );
    assert_eq!(
        Event::switched(&locked, &before),
        [Event::Session { locked: false }]
    );
    // A profile shown at the unlock: its event carries the whole view.
    let other = View {
        profile: "terminal".into(),
        ..before.clone()
    };
    assert_eq!(
        Event::switched(&locked, &other),
        [
            Event::Profile(other.clone()),
            Event::Session { locked: false }
        ]
    );
    assert_eq!(Event::Session { locked: true }.class(), EventClass::Session);
    assert_eq!(
        json(&Event::Session { locked: true }.to_line()),
        json(r#"{"v":1,"event":"session","locked":true}"#)
    );
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

/// The Keypad's panel as captured (spec §5.4).
fn captured_panel() -> Panel {
    let origins = [
        (24, 6),
        (182, 7),
        (340, 6),
        (23, 164),
        (181, 163),
        (339, 164),
        (23, 322),
        (181, 321),
        (340, 321),
    ];
    let size = TileSize {
        width: 118,
        height: 118,
    };
    Panel {
        size: TileSize {
            width: 480,
            height: 480,
        },
        keys: origins
            .iter()
            .map(|&(x, y)| KeyPlace { x, y, size })
            .collect(),
    }
}

#[test]
fn status_carries_the_geometry_and_the_configuration_directory() {
    let mut statuses = lines(fixture!("responses.ndjson")).filter_map(|line| {
        match serde_json::from_str::<Response>(line).unwrap().result {
            Ok(Reply::Status(status)) => Some(status),
            _ => None,
        }
    });
    let with = statuses.next().unwrap();
    assert_eq!(with.config_dir, "/home/u/.config/keyvo");
    let geometry = with.device.geometry.unwrap();
    assert_eq!(geometry, Geometry::from(&captured_panel()));
    assert_eq!(geometry.key_count(), with.device.keys);
    assert_eq!(
        geometry.tile_size(),
        TileSize {
            width: 118,
            height: 118
        }
    );
    assert_eq!(
        geometry.rects[1],
        Rect {
            x: 182,
            y: 7,
            width: 118,
            height: 118
        }
    );
    // Without a Keypad there is no geometry; the directory is always there.
    let without = statuses.next().unwrap();
    assert_eq!(without.config_dir, "/home/u/.config/keyvo");
    assert_eq!(without.device.geometry, None);
}

#[test]
fn a_panel_without_keys_has_no_tile() {
    let panel = Panel {
        size: TileSize {
            width: 10,
            height: 10,
        },
        keys: Vec::new(),
    };
    let geometry = Geometry::from(&panel);
    assert_eq!(
        geometry.tile_size(),
        TileSize {
            width: 0,
            height: 0
        }
    );
    assert_eq!(geometry.key_count(), Some(0));
}

#[test]
fn left_out_fields_take_their_defaults() {
    let request = Request::parse(r#"{"v":1,"cmd":"press","key":3}"#).unwrap();
    assert_eq!(request.id, None);
    assert_eq!(
        request.command,
        Command::press(KeySlot::new(3).unwrap(), PressKind::Short, false)
    );
    let request = Request::parse(r#"{"v":1,"cmd":"press","control":"roller","delta":-2}"#).unwrap();
    assert_eq!(
        request.command,
        Command::Press {
            key: None,
            control: Some(Control::Roller),
            delta: Some(-2),
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
            "unknown field `dryrun`, expected one of `key`, `control`, `delta`, `kind`, `dry_run`"
                .into()
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
fn a_press_names_a_key_or_a_control_and_a_turn_its_delta() {
    for (line, message) in [
        (
            r#"{"v":1,"id":1,"cmd":"press","key":1,"control":"dial","delta":1}"#,
            "a press names `key` or `control`, not both",
        ),
        (
            r#"{"v":1,"id":1,"cmd":"press"}"#,
            "a press needs `key` or `control`",
        ),
        (
            r#"{"v":1,"id":1,"cmd":"press","key":2,"delta":1}"#,
            "`delta` belongs to a turn of `dial` or `roller`",
        ),
        (
            r#"{"v":1,"id":1,"cmd":"press","control":"dial"}"#,
            "a turn of `dial` needs a nonzero `delta`",
        ),
        (
            r#"{"v":1,"id":1,"cmd":"press","control":"roller","delta":0}"#,
            "a turn of `roller` needs a nonzero `delta`",
        ),
        (
            r#"{"v":1,"id":1,"cmd":"press","control":"button:2","delta":1}"#,
            "`button:2` takes no `delta`",
        ),
    ] {
        assert_eq!(
            rejected(line),
            (
                Some(Value::from(1)),
                ErrorCode::InvalidRequest,
                message.into()
            ),
            "{line}"
        );
    }
    assert_eq!(
        rejected(r#"{"v":1,"cmd":"press","control":"button:5"}"#).1,
        ErrorCode::InvalidRequest
    );
    assert!(Request::parse(r#"{"v":1,"cmd":"press","control":"button:4"}"#).is_ok());
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

#[test]
fn a_modifier_change_is_a_modifier_event() {
    let before = View {
        profile: "global".into(),
        page: 1,
        mode: Mode::Auto,
        index: 1,
        locked: false,
        modifier: None,
    };
    let held = View {
        modifier: Some(ActiveModifier {
            key: KeySlot::new(7).unwrap(),
            mode: ModifierMode::Held,
        }),
        ..before.clone()
    };
    assert_eq!(
        Event::switched(&before, &held),
        [Event::Modifier {
            modifier: held.modifier
        }]
    );
    assert_eq!(
        Event::switched(&held, &before),
        [Event::Modifier { modifier: None }]
    );
    assert_eq!(
        Event::Modifier { modifier: None }.class(),
        EventClass::Modifier
    );
    assert_eq!(
        json(&Event::Modifier { modifier: None }.to_line()),
        json(r#"{"v":1,"event":"modifier","modifier":null}"#)
    );
    // The view carries it while one is active, and leaves it out else.
    assert_eq!(
        json(&Event::Page(held).to_line()),
        json(
            r#"{"v":1,"event":"page","profile":"global","page":1,"mode":"auto","index":1,"locked":false,"modifier":{"key":7,"mode":"held"}}"#
        )
    );
    assert!(!Event::Page(before).to_line().contains("modifier"));
}
