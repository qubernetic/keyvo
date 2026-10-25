//! Black-box tests of `keyvo ctl`: a fake daemon on a Unix socket in a
//! temporary directory reads the request line and answers with lines from
//! `tests/protocol-fixtures/socket/`.

#![cfg(unix)]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;

use assert_cmd::Command;
use serde_json::{Value, json};

const STATUS: &str = r#"{"v":1,"id":1,"ok":true,"result":{"version":"0.0.0","device":{"connected":true,"keys":9},"view":{"profile":"global","page":1,"mode":"auto"},"trace":0}}"#;

/// A daemon at `path` that takes one connection, returns the request it
/// read and answers with `lines`.
fn daemon(path: &Path, lines: &[&str]) -> JoinHandle<Value> {
    let listener = UnixListener::bind(path).unwrap();
    let lines: Vec<String> = lines.iter().map(|line| (*line).to_owned()).collect();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut request = String::new();
        reader.read_line(&mut request).unwrap();
        let mut stream = stream;
        for line in lines {
            stream.write_all(format!("{line}\n").as_bytes()).unwrap();
        }
        serde_json::from_str(&request).unwrap()
    })
}

/// A socket path in a temporary directory with `mode`.
fn socket_in(mode: u32) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(mode)).unwrap();
    let path = dir.path().join("keyvo.sock");
    (dir, path)
}

/// A socket path in a private directory, as the daemon creates it.
fn socket() -> (tempfile::TempDir, PathBuf) {
    socket_in(0o700)
}

fn ctl(path: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_keyvo"));
    command.arg("ctl").arg("--socket").arg(path);
    command.env_remove("KEYVO_SOCKET");
    command
}

#[test]
fn ctl_sends_the_command_and_prints_the_response() {
    let (_dir, path) = socket();
    let daemon = daemon(&path, &[STATUS]);
    ctl(&path)
        .arg("status")
        .assert()
        .success()
        .stdout(format!("{STATUS}\n"));
    assert_eq!(daemon.join().unwrap(), json!({"v": 1, "cmd": "status"}));
}

#[test]
fn the_json_argument_carries_the_fields() {
    let (_dir, path) = socket();
    let pressed = r#"{"v":1,"ok":true,"result":{"entry":{"timestamp_ms":1,"trigger":{"press":"short","simulated":true},"profile":"global","page":1,"key":5,"action":null,"result":{"status":"unbound"}}}}"#;
    let daemon = daemon(&path, &[pressed]);
    ctl(&path)
        .args(["press", r#"{"key":5,"dry_run":true}"#])
        .assert()
        .success()
        .stdout(format!("{pressed}\n"));
    assert_eq!(
        daemon.join().unwrap(),
        json!({"v": 1, "cmd": "press", "key": 5, "dry_run": true})
    );
}

#[test]
fn a_refused_command_prints_the_response_and_fails() {
    let (_dir, path) = socket();
    let refused = r#"{"v":1,"ok":false,"error":{"code":"invalid_request","message":"unknown variant `nope`"}}"#;
    let daemon = daemon(&path, &[refused]);
    let output = ctl(&path)
        .arg("nope")
        .assert()
        .failure()
        .get_output()
        .clone();
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("{refused}\n")
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("invalid_request: unknown variant `nope`"),
        "{stderr}"
    );
    daemon.join().unwrap();
}

#[test]
fn subscribe_prints_every_line_until_the_daemon_hangs_up() {
    let (_dir, path) = socket();
    let subscribed = r#"{"v":1,"ok":true,"result":{"events":["config"],"view":{"profile":"global","page":1,"mode":"auto"}}}"#;
    let reloaded = r#"{"v":1,"event":"config","reloaded":true}"#;
    let daemon = daemon(&path, &[subscribed, reloaded, reloaded]);
    ctl(&path)
        .args(["subscribe", r#"{"events":["config"]}"#])
        .assert()
        .success()
        .stdout(format!("{subscribed}\n{reloaded}\n{reloaded}\n"));
    assert_eq!(
        daemon.join().unwrap(),
        json!({"v": 1, "cmd": "subscribe", "events": ["config"]})
    );
}

#[test]
fn the_json_argument_must_be_an_object_without_cmd() {
    let (_dir, path) = socket();
    for (json, message) in [
        ("[1]", "the JSON argument must be an object"),
        ("{", "the JSON argument is not JSON"),
        (
            r#"{"cmd":"reload"}"#,
            "give the command as the first argument",
        ),
    ] {
        let output = ctl(&path)
            .args(["status", json])
            .assert()
            .failure()
            .get_output()
            .clone();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(message), "{json}: {stderr}");
    }
}

#[test]
fn without_a_daemon_ctl_says_so() {
    let (_dir, path) = socket();
    let output = ctl(&path)
        .arg("status")
        .assert()
        .failure()
        .get_output()
        .clone();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("is keyvo-daemon running?"), "{stderr}");
    assert!(stderr.contains(&path.display().to_string()), "{stderr}");
}

#[test]
fn a_socket_directory_others_can_enter_is_refused() {
    let (_dir, path) = socket_in(0o755);
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let output = ctl(&path)
        .arg("status")
        .assert()
        .failure()
        .get_output()
        .clone();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("refusing the keyvo socket"), "{stderr}");
    assert!(stderr.contains("not private to this user"), "{stderr}");
    // Nothing was sent: the connection the refused ctl made is silent.
    let (mut stream, _) = listener.accept().unwrap();
    let mut sent = Vec::new();
    std::io::Read::read_to_end(&mut stream, &mut sent).unwrap();
    assert!(sent.is_empty(), "{}", String::from_utf8_lossy(&sent));
}

#[test]
fn a_daemon_that_hangs_up_without_answering_fails() {
    let (_dir, path) = socket();
    let daemon = daemon(&path, &[]);
    let output = ctl(&path)
        .arg("status")
        .assert()
        .failure()
        .get_output()
        .clone();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("closed the connection"), "{stderr}");
    daemon.join().unwrap();
}

#[test]
fn the_socket_comes_from_keyvo_socket_then_the_runtime_directory() {
    let (dir, path) = socket();
    let daemon_at = daemon(&path, &[STATUS]);
    Command::new(env!("CARGO_BIN_EXE_keyvo"))
        .args(["ctl", "status"])
        .env("KEYVO_SOCKET", &path)
        .assert()
        .success();
    daemon_at.join().unwrap();

    let runtime = dir.path().join("keyvo");
    std::fs::create_dir(&runtime).unwrap();
    std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
    let daemon_at = daemon(&runtime.join("keyvo.sock"), &[STATUS]);
    Command::new(env!("CARGO_BIN_EXE_keyvo"))
        .args(["ctl", "status"])
        .env_remove("KEYVO_SOCKET")
        .env("XDG_RUNTIME_DIR", dir.path())
        .assert()
        .success();
    daemon_at.join().unwrap();
}
