//! The control socket end to end: a daemon on `Pad`s, the socket in a
//! private directory, clients on real Unix stream connections sending the
//! lines of `tests/protocol-fixtures/socket/requests.ndjson`.

#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use keyvo_core::focus::FakeWindowWatcher;
use keyvo_core::inject::FakeInputInjector;
use keyvo_core::socket::{Event, Reply, Response, ServerMessage};
use keyvo_daemon::daemon;
use keyvo_daemon::socket::Server;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::task::JoinHandle;

use support::{Pad, Pads, startup};

const PATIENCE: Duration = Duration::from_secs(10);

const CONFIG: &str = "\
[timing]
keep_alive_period = 3600000
divert_settle = 0
divert_spacing = 0
render_coalesce = 5
reload_settle = 20
focus_hysteresis = 10
";

const GLOBAL: &str = r#"
schema_version = 1
[profile]
id = "global"
name = "Global"

[[pages]]
[pages.keys.1]
label = "Fail"
action = { type = "exec", cmd = "exit 1" }

[pages.keys.5]
label = "Copy"
action = { type = "chord", keys = "ctrl+c" }
"#;

const TERMINAL: &str = r#"
schema_version = 1
[profile]
id = "terminal"
name = "Terminal"
match = { class = ["org.kde.konsole"] }

[[pages]]
[pages.keys.1]
label = "Shell"
"#;

struct Running {
    _dir: tempfile::TempDir,
    socket: PathBuf,
    injector: FakeInputInjector,
    stop: oneshot::Sender<()>,
    daemon: JoinHandle<std::io::Result<()>>,
    server: JoinHandle<()>,
}

impl Running {
    async fn start() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let config_dir = dir.path().join("config");
        std::fs::create_dir_all(config_dir.join("profiles")).unwrap();
        std::fs::write(config_dir.join("config.toml"), CONFIG).unwrap();
        std::fs::write(config_dir.join("profiles/global.toml"), GLOBAL).unwrap();
        std::fs::write(config_dir.join("profiles/terminal.toml"), TERMINAL).unwrap();
        // tempdir is 0755 here; the runtime directory below it is created.
        let socket = dir.path().join("run/keyvo.sock");
        let server = Server::bind(&socket).unwrap();
        let (requests, inbox) = mpsc::unbounded_channel();
        let server = tokio::spawn(async move { server.serve(requests).await });
        let injector = FakeInputInjector::new();
        let (events, mut painted) = broadcast::channel(256);
        let (stop, stopped) = oneshot::channel::<()>();
        let pads = Pads::new([Pad::new(&startup())]);
        let daemon_injector = injector.clone();
        let daemon = tokio::spawn(async move {
            daemon::run(
                &config_dir,
                pads,
                daemon_injector,
                FakeWindowWatcher::new(),
                events,
                inbox,
                async {
                    let _ = stopped.await;
                },
            )
            .await
        });
        // The first paint: the Keypad is claimed.
        tokio::time::timeout(PATIENCE, async {
            while !matches!(painted.recv().await.unwrap(), daemon::Event::Painted { .. }) {}
        })
        .await
        .unwrap();
        Self {
            _dir: dir,
            socket,
            injector,
            stop,
            daemon,
            server,
        }
    }

    async fn client(&self) -> Client {
        let (read, write) = UnixStream::connect(&self.socket)
            .await
            .unwrap()
            .into_split();
        Client {
            lines: BufReader::new(read).lines(),
            write,
        }
    }

    async fn stop(self) {
        self.stop.send(()).unwrap();
        self.daemon.await.unwrap().unwrap();
        self.server.abort();
        let _ = self.server.await;
    }
}

struct Client {
    lines: Lines<BufReader<OwnedReadHalf>>,
    write: OwnedWriteHalf,
}

impl Client {
    async fn send(&mut self, line: &str) {
        self.write
            .write_all(format!("{line}\n").as_bytes())
            .await
            .unwrap();
    }

    async fn recv(&mut self) -> String {
        tokio::time::timeout(PATIENCE, self.lines.next_line())
            .await
            .expect("a line in time")
            .unwrap()
            .expect("an open connection")
    }

    async fn ask(&mut self, line: &str) -> Value {
        self.send(line).await;
        serde_json::from_str(&self.recv().await).unwrap()
    }

    /// The next event; responses in between fail the test.
    async fn event(&mut self) -> Event {
        let line = self.recv().await;
        match ServerMessage::parse(&line).unwrap() {
            ServerMessage::Event(event) => event,
            ServerMessage::Response(_) => panic!("not an event: {line}"),
        }
    }
}

fn mode(path: &Path) -> u32 {
    std::fs::symlink_metadata(path)
        .unwrap()
        .permissions()
        .mode()
        & 0o777
}

#[tokio::test(flavor = "multi_thread")]
async fn the_socket_is_mode_0600_in_a_0700_directory_and_goes_with_the_server() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("run/keyvo.sock");
    let server = Server::bind(&path).unwrap();
    assert_eq!(server.path(), path);
    assert!(
        std::fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_socket()
    );
    assert_eq!(mode(&path), 0o600);
    assert_eq!(mode(path.parent().unwrap()), 0o700);

    // A second daemon does not take over a live socket.
    let error = Server::bind(&path).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::AddrInUse);
    assert!(
        error.to_string().contains("another keyvo daemon"),
        "{error}"
    );

    drop(server);
    assert!(!path.exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stale_socket_is_replaced_and_anything_else_refused() {
    let dir = tempfile::tempdir().unwrap();
    let run = dir.path().join("run");
    let path = run.join("keyvo.sock");
    // A socket nobody listens on, as a crashed daemon leaves it.
    drop(std::os::unix::net::UnixListener::bind({
        std::fs::create_dir(&run).unwrap();
        std::fs::set_permissions(&run, std::fs::Permissions::from_mode(0o700)).unwrap();
        &path
    }));
    assert!(path.exists());
    let server = Server::bind(&path).unwrap();
    assert_eq!(mode(&path), 0o600);
    drop(server);

    std::fs::write(&path, "not a socket").unwrap();
    let error = Server::bind(&path).unwrap_err();
    assert!(error.to_string().contains("is not a socket"), "{error}");

    // A directory others may enter is refused.
    std::fs::remove_file(&path).unwrap();
    std::fs::set_permissions(&run, std::fs::Permissions::from_mode(0o755)).unwrap();
    let error = Server::bind(&path).unwrap_err();
    assert!(error.to_string().contains("not private"), "{error}");
}

#[tokio::test(flavor = "multi_thread")]
async fn every_fixture_request_gets_its_answer() {
    let running = Running::start().await;
    let mut client = running.client().await;
    let fixture = include_str!("../../../tests/protocol-fixtures/socket/requests.ndjson");
    for line in fixture.lines().filter(|line| !line.is_empty()) {
        let request: Value = serde_json::from_str(line).unwrap();
        client.send(line).await;
        // A subscription may stream events before the next answer.
        let response = loop {
            match ServerMessage::parse(&client.recv().await).unwrap() {
                ServerMessage::Response(response) => break response,
                ServerMessage::Event(_) => {}
            }
        };
        assert_eq!(
            response.id.as_ref(),
            request.get("id"),
            "{line}: {response:?}"
        );
        let reply = response.result.unwrap_or_else(|e| panic!("{line}: {e:?}"));
        match (request["cmd"].as_str().unwrap(), reply) {
            ("status", Reply::Status(status)) => {
                assert_eq!(status.device.keys, Some(9));
            }
            ("subscribe", Reply::Subscribed(subscribed)) => {
                assert_eq!(json!(subscribed.events), request["events"]);
            }
            ("press", Reply::Pressed(pressed)) => {
                assert_eq!(json!(pressed.entry.key), request["key"]);
                assert_eq!(json!(pressed.entry.trigger.simulated), request["dry_run"]);
                assert_eq!(json!(pressed.entry.trigger.press), request["kind"]);
            }
            ("reload" | "notify_context", Reply::Done {}) => {}
            (cmd, reply) => panic!("{cmd}: {reply:?}"),
        }
    }
    running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dry_run_press_returns_a_simulated_entry_and_runs_nothing() {
    let running = Running::start().await;
    let mut client = running.client().await;
    let response = client
        .ask(r#"{"v":1,"id":"sim","cmd":"press","key":5,"dry_run":true}"#)
        .await;
    assert_eq!(response["id"], "sim");
    assert_eq!(response["ok"], true);
    let entry = &response["result"]["entry"];
    assert_eq!(
        entry["trigger"],
        json!({"press": "short", "simulated": true})
    );
    assert_eq!(entry["action"], json!({"type": "chord", "keys": "ctrl+c"}));
    assert_eq!(entry["result"], json!({"status": "dry_run"}));
    assert!(running.injector.sent().is_empty());

    // Without dry_run the same key sends its chord.
    let response = client.ask(r#"{"v":1,"cmd":"press","key":5}"#).await;
    assert_eq!(response["result"]["entry"]["result"]["status"], "sent");
    assert_eq!(running.injector.sent().len(), 4);
    running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn bad_lines_get_error_responses_and_the_connection_stays() {
    let running = Running::start().await;
    let mut client = running.client().await;
    let code = |response: &Value| response["error"]["code"].as_str().unwrap().to_owned();
    assert_eq!(code(&client.ask("not json").await), "invalid_json");
    let response = client.ask(r#"{"v":2,"id":7,"cmd":"status"}"#).await;
    assert_eq!(
        (response["id"].clone(), code(&response)),
        (json!(7), "unsupported_version".into())
    );
    let response = client
        .ask(r#"{"v":1,"id":8,"cmd":"press","key":5,"dryrun":true}"#)
        .await;
    assert_eq!(code(&response), "invalid_request");
    assert!(running.injector.sent().is_empty());
    // Blank lines are skipped; the connection still answers.
    client.send("").await;
    let response = client.ask(r#"{"v":1,"id":9,"cmd":"status"}"#).await;
    assert_eq!(
        (response["id"].clone(), response["ok"].clone()),
        (json!(9), json!(true))
    );
    running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_subscriber_gets_the_events_of_its_classes() {
    let running = Running::start().await;
    let mut watcher = running.client().await;
    let response = watcher
        .ask(r#"{"v":1,"cmd":"subscribe","events":["profile","trace","config"]}"#)
        .await;
    assert_eq!(response["result"]["view"]["profile"], "global");

    let mut other = running.client().await;
    other
        .ask(r#"{"v":1,"cmd":"notify_context","class":"org.kde.konsole","title":"~"}"#)
        .await;
    // No context event: not subscribed. The profile switch is.
    let Event::Profile(view) = watcher.event().await else {
        panic!("not a profile event");
    };
    assert_eq!(view.profile, "terminal");

    other
        .ask(r#"{"v":1,"cmd":"press","key":2,"dry_run":true}"#)
        .await;
    let Event::Trace { entry } = watcher.event().await else {
        panic!("not a trace event");
    };
    assert_eq!((entry.profile.as_str(), entry.key.get()), ("terminal", 2));

    other.ask(r#"{"v":1,"cmd":"reload"}"#).await;
    let Event::Config(status) = watcher.event().await else {
        panic!("not a config event");
    };
    assert!(status.reloaded);
    running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn subscribe_replays_trace_entries_before_the_stream() {
    let running = Running::start().await;
    let mut client = running.client().await;
    for key in [3, 4] {
        client
            .ask(&format!(
                r#"{{"v":1,"cmd":"press","key":{key},"dry_run":true}}"#
            ))
            .await;
    }
    let mut subscriber = running.client().await;
    subscriber
        .ask(r#"{"v":1,"cmd":"subscribe","events":["trace","device"],"replay":1}"#)
        .await;
    let Event::Trace { entry } = subscriber.event().await else {
        panic!("not a trace event");
    };
    assert_eq!(entry.key.get(), 4);
    // A subscription without the trace class replays nothing.
    let mut other = running.client().await;
    other
        .ask(r#"{"v":1,"cmd":"subscribe","events":["key"],"replay":5}"#)
        .await;
    let status = other.ask(r#"{"v":1,"id":"s","cmd":"status"}"#).await;
    assert_eq!(
        (status["id"].clone(), status["result"]["trace"].clone()),
        (json!("s"), json!(2))
    );
    running.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_daemon_that_is_gone_answers_unavailable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("run/keyvo.sock");
    let server = Server::bind(&path).unwrap();
    let (requests, inbox) = mpsc::unbounded_channel();
    drop(inbox);
    let server = tokio::spawn(async move { server.serve(requests).await });
    let (read, write) = UnixStream::connect(&path).await.unwrap().into_split();
    let mut client = Client {
        lines: BufReader::new(read).lines(),
        write,
    };
    let response: Response =
        serde_json::from_value(client.ask(r#"{"v":1,"id":1,"cmd":"status"}"#).await).unwrap();
    let error = response.result.unwrap_err();
    assert_eq!(error.message, "the daemon is shutting down");
    server.abort();
}
