//! The client against the shared socket fixtures in
//! `tests/protocol-fixtures/socket/`: the daemon's side of every exchange
//! is a fixture line, and what the client writes is checked against the
//! request fixtures.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{self, BufRead, BufReader, Cursor, Read, Write};
use std::os::unix::net::UnixListener;

use keyvo_client::{Client, ClientError};
use keyvo_core::recipe::Source;
use keyvo_core::socket::{Command, ErrorCode, Event, EventClass, Reply, Response, ServerMessage};
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

/// A daemon that answers with a fixed script and records what it is sent.
#[derive(Default)]
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

#[test]
fn status_sends_the_fixture_request_and_returns_the_fixture_answer() {
    let responses = lines(fixture!("responses.ndjson"));
    let mut client = Client::new(Script::new(&responses[..1]));

    let Reply::Status(status) = client.request(Command::Status {}).unwrap() else {
        panic!("not a status reply");
    };
    assert_eq!(
        status.view,
        View {
            profile: "terminal".into(),
            page: 1,
            mode: Mode::Auto,
            index: 1,
            locked: false,
            modifier: None,
        }
    );
    let written = String::from_utf8(client.into_inner().written).unwrap();
    assert_eq!(written.lines().count(), 1);
    assert_eq!(json(&written), json(lines(fixture!("requests.ndjson"))[0]));
}

#[test]
fn recipes_sends_the_fixture_request_and_returns_the_catalogue() {
    let exchange = lines(fixture!("recipes.ndjson"));
    let answers = [answering(exchange[1], 1), answering(exchange[2], 2)];
    let answers: Vec<&str> = answers.iter().map(String::as_str).collect();
    let mut client = Client::new(Script::new(&answers));

    let recipes = client.recipes().unwrap();
    assert!(recipes.problems.is_empty());
    let screenshot = recipes
        .recipes
        .iter()
        .find(|r| r.id == "screenshot")
        .unwrap();
    assert_eq!(screenshot.source, Source::User);
    assert!(recipes.recipes.iter().any(|r| r.id == "spectacle"));
    assert_eq!(client.recipes().unwrap().problems.len(), 1);

    let written = String::from_utf8(client.into_inner().written).unwrap();
    let mut request = json(written.lines().next().unwrap());
    request["id"] = json(exchange[0])["id"].clone();
    assert_eq!(request, json(exchange[0]));
}

#[test]
fn recipes_refuses_another_reply() {
    let status = answering(lines(fixture!("responses.ndjson"))[0], 1);
    let mut client = Client::new(Script::new(&[&status]));
    assert!(matches!(
        client.recipes(),
        Err(ClientError::Unexpected("recipes"))
    ));
}

/// `line` with its `id` set to `id`, as the answer to the client's request
/// number `id`.
fn answering(line: &str, id: u64) -> String {
    let mut value = json(line);
    value["id"] = id.into();
    value.to_string()
}

#[test]
fn every_response_fixture_answers_a_request() {
    let responses = lines(fixture!("responses.ndjson"));
    let script: Vec<String> = responses
        .iter()
        .zip(1..)
        .map(|(line, id)| answering(line, id))
        .collect();
    let script: Vec<&str> = script.iter().map(String::as_str).collect();
    let mut client = Client::new(Script::new(&script));

    for line in &responses {
        let expected: Response = serde_json::from_str(line).unwrap();
        let got = client.request(Command::Reload {});
        match (expected.result, got) {
            (Ok(reply), Ok(got)) => assert_eq!(got, reply, "{line}"),
            (Err(error), Err(ClientError::Refused(got))) => assert_eq!(got, error, "{line}"),
            (_, got) => panic!("{line}: {got:?}"),
        }
    }
}

#[test]
fn an_answer_without_an_id_answers_the_open_request() {
    let invalid_json = lines(fixture!("responses.ndjson"))
        .into_iter()
        .find(|line| !line.contains("\"id\""))
        .unwrap();
    let mut client = Client::new(Script::new(&[invalid_json]));

    let Err(ClientError::Refused(error)) = client.request(Command::Status {}) else {
        panic!("not refused");
    };
    assert_eq!(error.code, ErrorCode::InvalidJson);
}

#[test]
fn events_before_the_answer_wait_for_next_event() {
    let events = lines(fixture!("events.ndjson"));
    let subscribed = lines(fixture!("responses.ndjson"))[2];
    let mut script = events[..3].to_vec();
    let answer = answering(subscribed, 1);
    script.push(&answer);
    script.extend(&events[3..]);
    let mut client = Client::new(Script::new(&script));

    let reply = client
        .request(Command::Subscribe {
            events: EventClass::ALL.to_vec(),
            replay: 0,
        })
        .unwrap();
    assert!(matches!(reply, Reply::Subscribed(_)));

    let mut got = Vec::new();
    while let Some(event) = client.next_event().unwrap() {
        got.push(event);
    }
    let expected: Vec<Event> = events
        .iter()
        .map(|line| match ServerMessage::parse(line).unwrap() {
            ServerMessage::Event(event) => event,
            ServerMessage::Response(_) => panic!("{line}"),
        })
        .collect();
    assert_eq!(got, expected);
}

#[test]
fn an_answer_to_another_request_is_skipped() {
    let done = lines(fixture!("responses.ndjson"))[5];
    let stale = answering(done, 7);
    let answer = answering(done, 1);
    let mut client = Client::new(Script::new(&[&stale, &answer]));

    assert_eq!(client.request(Command::Reload {}).unwrap(), Reply::Done {});
    assert!(client.next_event().unwrap().is_none());
}

/// A temporary socket directory with `mode`; the daemon creates its own
/// `0700`.
fn socket_dir(mode: u32) -> tempfile::TempDir {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(mode)).unwrap();
    dir
}

#[test]
fn connect_talks_to_a_unix_socket() {
    let dir = socket_dir(0o700);
    let path = dir.path().join("keyvo.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let answer = answering(lines(fixture!("responses.ndjson"))[5], 1);
    let daemon = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut request = String::new();
        BufReader::new(&stream).read_line(&mut request).unwrap();
        (&stream)
            .write_all(format!("{answer}\n").as_bytes())
            .unwrap();
        request
    });

    let mut client = Client::connect(&path).unwrap();
    assert_eq!(client.request(Command::Reload {}).unwrap(), Reply::Done {});
    assert_eq!(
        json(&daemon.join().unwrap()),
        serde_json::json!({"v": 1, "id": 1, "cmd": "reload"})
    );
}

#[test]
fn connect_refuses_a_socket_directory_others_can_enter() {
    for mode in [0o755, 0o770, 0o701] {
        let dir = socket_dir(mode);
        let path = dir.path().join("keyvo.sock");
        let _listener = UnixListener::bind(&path).unwrap();
        let error = Client::connect(&path).unwrap_err();
        assert!(matches!(error, ClientError::Untrusted { .. }), "{error:?}");
        let message = error.to_string();
        assert!(message.contains("not private to this user"), "{message}");
        assert!(message.contains(&*path.to_string_lossy()), "{message}");
    }
}

#[test]
fn connect_without_a_daemon_names_the_socket() {
    let dir = socket_dir(0o700);
    let path = dir.path().join("keyvo.sock");
    let error = Client::connect(&path).unwrap_err();
    assert!(matches!(error, ClientError::Connect { .. }), "{error:?}");
    assert!(
        error.to_string().contains(&*path.to_string_lossy()),
        "{error}"
    );
}

#[test]
fn a_hang_up_before_the_answer_is_closed() {
    let mut client = Client::new(Script::new(&[]));
    assert!(matches!(
        client.request(Command::Status {}),
        Err(ClientError::Closed)
    ));
}

#[test]
fn a_line_over_the_limit_is_refused_without_holding_it() {
    let limit = keyvo_client::client::LINE_LIMIT;
    let long = format!(r#"{{"v":1,"event":"key","pad":"{}"}}"#, "x".repeat(limit));
    let mut client = Client::new(Script::new(&[&long]));
    let error = client.request(Command::Status {}).unwrap_err();
    assert!(
        matches!(error, ClientError::TooLong(n) if n == limit),
        "{error:?}"
    );
    assert!(
        error
            .to_string()
            .contains(&format!("longer than {limit} bytes")),
        "{error}"
    );

    // A line of exactly the limit is read.
    let answer = answering(lines(fixture!("responses.ndjson"))[5], 1);
    let padded = format!("{answer}{}", " ".repeat(limit - answer.len()));
    let mut client = Client::new(Script::new(&[&padded]));
    assert_eq!(client.request(Command::Reload {}).unwrap(), Reply::Done {});
}

#[test]
fn a_line_that_is_not_v1_is_a_protocol_error() {
    let mut client = Client::new(Script::new(&[r#"{"v":2,"ok":true,"result":{}}"#]));
    let error = client.request(Command::Status {}).unwrap_err();
    assert!(matches!(error, ClientError::Protocol(_)), "{error:?}");
    assert!(error.to_string().contains("version 2"), "{error}");
}
