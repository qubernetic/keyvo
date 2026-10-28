//! Black-box tests of `keyvo doctor` on the fake roots under
//! `tests/fixtures/doctor/` (their README says what each one holds and what
//! the test adds): the text and `--json` output as insta snapshots, with the
//! temporary root shown as `[root]`, and the exit status.

#![cfg(unix)]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use serde_json::Value;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixtures() -> PathBuf {
    manifest_dir().join("tests/fixtures/doctor")
}

fn hex(text: &str) -> Vec<u8> {
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .flat_map(str::split_whitespace)
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect()
}

/// Copies `from` into `to`: `report_descriptor.hex` as the binary
/// `report_descriptor`, a `*.shipped` placeholder as the shipped rule.
fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        let source = entry.path();
        if source.is_dir() {
            copy(&source, &to.join(&name));
        } else if name == "report_descriptor.hex" {
            let text = std::fs::read_to_string(&source).unwrap();
            std::fs::write(to.join("report_descriptor"), hex(&text)).unwrap();
        } else if let Some(installed) = name.strip_suffix(".shipped") {
            let rule = manifest_dir().join("../../contrib/udev/70-keyvo.rules");
            std::fs::copy(rule, to.join(installed)).unwrap();
        } else {
            std::fs::copy(&source, to.join(&name)).unwrap();
        }
    }
}

/// The first line of the shared socket fixture: the answer to `status`.
fn status_line() -> String {
    let path = manifest_dir().join("../../tests/protocol-fixtures/socket/responses.ndjson");
    let text = std::fs::read_to_string(path).unwrap();
    text.lines().next().unwrap().to_owned()
}

/// A daemon on `root/run/keyvo/keyvo.sock` that answers one `status`.
fn daemon(root: &Path) {
    let dir = root.join("run/keyvo");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(root.join("run"), std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = dir.join("keyvo.sock");
    let listener = UnixListener::bind(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let answer = status_line();
    // Never joined: a doctor that does not connect leaves it waiting until
    // the test process ends.
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut request = String::new();
        BufReader::new(stream.try_clone().unwrap())
            .read_line(&mut request)
            .unwrap();
        let request: Value = serde_json::from_str(&request).unwrap();
        assert_eq!(request["cmd"], "status");
        let mut stream = stream;
        stream.write_all(format!("{answer}\n").as_bytes()).unwrap();
    });
}

/// The fake root of `case`, with the daemon listening.
fn root(case: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    copy(
        &manifest_dir().join("../keyvo-hid/tests/fixtures/sysfs/keypad-and-receiver"),
        &root.path().join("sys"),
    );
    copy(&fixtures().join(case), root.path());
    let bin = root.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    for name in ["systemctl", "flatpak"] {
        std::os::unix::fs::symlink(fixtures().join("fake-command"), bin.join(name)).unwrap();
    }
    daemon(root.path());
    root
}

/// `keyvo doctor` on `root` with `args`: stdout with the root as `[root]`,
/// and the exit code.
fn doctor(root: &Path, args: &[&str]) -> (String, i32) {
    let output = Command::new(env!("CARGO_BIN_EXE_keyvo"))
        .arg("doctor")
        .arg("--root")
        .arg(root)
        .args(args)
        .env_clear()
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", root.join("bin").display()),
        )
        .env("HOME", root.join("home"))
        .env("XDG_RUNTIME_DIR", root.join("run"))
        .env("XDG_SESSION_TYPE", "wayland")
        .env("XDG_CURRENT_DESKTOP", "KDE")
        .env("FAKE_COMMANDS", root.join("commands"))
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr, "");
    let shown = root.display().to_string();
    (
        stdout.replace(&shown, "[root]"),
        output.status.code().unwrap(),
    )
}

/// The status of each check in the `--json` lines, by name.
fn statuses(json: &str) -> Vec<(String, String)> {
    json.lines()
        .map(|line| {
            let value: Value = serde_json::from_str(line).unwrap();
            (
                value["check"].as_str().unwrap().to_owned(),
                value["status"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// The nine checks in order, each `ok` except those in `differ`.
fn expected(differ: &[(&str, &str)]) -> Vec<(String, String)> {
    [
        "keypad", "uinput", "rule", "session", "focus", "unit", "daemon", "solaar", "flatpak",
    ]
    .iter()
    .map(|name| {
        let status = differ
            .iter()
            .find(|(check, _)| check == name)
            .map_or("ok", |(_, status)| status);
        ((*name).to_owned(), status.to_owned())
    })
    .collect()
}

fn case(name: &str, differ: &[(&str, &str)], code: i32) {
    let root = root(name);
    let (text, text_code) = doctor(root.path(), &[]);
    assert_eq!(text_code, code, "{text}");
    insta::assert_snapshot!(format!("{name}-text"), text);
    let root = self::root(name);
    let (json, json_code) = doctor(root.path(), &["--json"]);
    assert_eq!(json_code, code, "{json}");
    assert_eq!(statuses(&json), expected(differ), "{json}");
    insta::assert_snapshot!(format!("{name}-json"), json);
}

#[test]
fn all_good_is_nine_ok_lines_and_exit_0() {
    case("all-good", &[], 0);
}

#[test]
fn a_missing_rule_with_working_access_is_a_warning() {
    case("no-rule", &[("rule", "warn")], 0);
}

#[test]
fn a_missing_uinput_fails_and_exits_1() {
    case("no-uinput", &[("uinput", "fail")], 1);
}

#[test]
fn focus_stealing_prevention_medium_is_a_warning() {
    case("level-2", &[("focus", "warn")], 0);
}

#[test]
fn a_running_solaar_is_a_warning() {
    case("solaar-running", &[("solaar", "warn")], 0);
}
