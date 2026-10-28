//! `keyvo doctor [--json]`: nine checks of the setup on this machine (spec
//! §12), one line each with `ok`, `warn` or `fail`, a sentence and the fix
//! command. Every check is a predicate over inputs a test can fake: a root
//! directory for sysfs, `/dev`, `/etc` and `/proc`, the environment, and a
//! [`Runner`] for `systemctl` and `flatpak`.
//!
//! What it touches: it opens the Keypad's hidraw node and `/dev/uinput`
//! read-write and closes them again without a byte written or an ioctl
//! sent, reads files, sends `status` on the daemon's socket, and runs
//! `systemctl --user is-active|is-enabled keyvo-daemon`, `flatpak ps` and
//! `flatpak info` with fixed arguments. It changes nothing.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use keyvo_client::{Client, ClientError};
use keyvo_core::socket::{Command as Request, Reply};
use keyvo_hid::sysfs::{self, SysfsError};
use serde_json::json;

/// The rule keyvo ships; check 3 compares the installed one with it.
pub const RULE: &str = include_str!("../../../contrib/udev/70-keyvo.rules");
/// Where the rule is installed, below the root.
const RULE_PATH: &str = "etc/udev/rules.d/70-keyvo.rules";
/// The rule's install lines, run from the repository checkout.
const RULE_INSTALL: &str = "sudo install -m 644 contrib/udev/70-keyvo.rules /etc/udev/rules.d/\n\
     sudo udevadm control --reload && sudo udevadm trigger";
/// The user unit's install lines, run from the repository checkout.
const UNIT_INSTALL: &str = "install -Dm755 bin/keyvo-daemon ~/.local/bin/keyvo-daemon\n\
     install -Dm644 contrib/systemd/keyvo-daemon.service ~/.config/systemd/user/\n\
     systemctl --user daemon-reload && systemctl --user enable --now keyvo-daemon";
const UNIT: &str = "keyvo-daemon";
const SOLAAR_FLATPAK: &str = "io.github.pwr_solaar.solaar";
const APP_FLATPAK: &str = "io.github.qubernetic.keyvo";
/// The names of `KWin`'s `FocusStealingPreventionLevel`, 0 to 4.
const LEVELS: [&str; 5] = ["None", "Low", "Medium", "High", "Extreme"];

/// A check's verdict. Only `fail` makes `keyvo doctor` exit 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

impl Status {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Fail => "fail",
        }
    }
}

/// One check's line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// The check's name: `keypad`, `uinput`, `rule`, `session`, `focus`,
    /// `unit`, `daemon`, `solaar` or `flatpak`.
    pub name: &'static str,
    pub status: Status,
    /// One sentence.
    pub message: String,
    /// The commands that fix it, one per line.
    pub fix: Option<String>,
}

impl Check {
    fn new(name: &'static str, status: Status, message: impl Into<String>) -> Self {
        Self {
            name,
            status,
            message: message.into(),
            fix: None,
        }
    }

    fn fix(mut self, fix: impl Into<String>) -> Self {
        self.fix = Some(fix.into());
        self
    }
}

/// What a command printed and whether it exited 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    pub success: bool,
    pub stdout: String,
}

/// Runs a program with arguments; `None` when it cannot be started (not
/// installed).
pub trait Runner {
    fn run(&mut self, program: &str, args: &[&str]) -> Option<Ran>;
}

/// The real [`Runner`]: the program from `PATH`, stdin and stderr on
/// `/dev/null`, stdout captured.
pub struct System;

impl Runner for System {
    fn run(&mut self, program: &str, args: &[&str]) -> Option<Ran> {
        let output = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        Some(Ran {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        })
    }
}

/// The inputs besides the [`Runner`].
pub struct Inputs<'a> {
    /// `/` on a real machine; sysfs, `/dev`, `/etc` and `/proc` are below it.
    pub root: &'a Path,
    /// The process environment.
    pub env: &'a BTreeMap<String, OsString>,
    /// This user's uid.
    pub uid: u32,
    /// `[timing].doctor_reply`.
    pub reply: Duration,
}

impl Inputs<'_> {
    fn var(&self, name: &str) -> Option<&std::ffi::OsStr> {
        self.env.get(name).map(OsString::as_os_str)
    }
}

/// The nine checks, in order.
pub fn doctor(inputs: &Inputs<'_>, runner: &mut dyn Runner) -> Vec<Check> {
    let keypad = keypad(inputs.root);
    let uinput = uinput(inputs.root);
    let access = keypad.status == Status::Ok && uinput.status == Status::Ok;
    let rule = rule(inputs.root, access);
    vec![
        keypad,
        uinput,
        rule,
        session(inputs),
        focus(inputs),
        unit(runner),
        daemon(inputs),
        solaar(inputs.root, runner),
        flatpak(runner),
    ]
}

/// `true` when a check failed.
pub fn failed(checks: &[Check]) -> bool {
    checks.iter().any(|check| check.status == Status::Fail)
}

/// One line per check, `status  name  message`, and the fix lines below
/// it, indented.
///
/// # Errors
///
/// When `out` fails.
pub fn write_text(checks: &[Check], out: &mut impl Write) -> std::io::Result<()> {
    for check in checks {
        writeln!(
            out,
            "{:<4}  {:<7}  {}",
            check.status.as_str(),
            check.name,
            check.message
        )?;
        for line in check.fix.iter().flat_map(|fix| fix.lines()) {
            writeln!(out, "      fix: {line}")?;
        }
    }
    Ok(())
}

/// One JSON object per check and line: `check`, `status`, `message` and
/// `fix` (a string, the lines joined by `\n`, or `null`).
///
/// # Errors
///
/// When `out` fails.
pub fn write_json(checks: &[Check], out: &mut impl Write) -> std::io::Result<()> {
    for check in checks {
        let line = json!({
            "check": check.name,
            "status": check.status.as_str(),
            "message": check.message,
            "fix": check.fix,
        });
        writeln!(out, "{line}")?;
    }
    Ok(())
}

/// Opens `path` read-write and closes it again.
fn open_rw(path: &Path) -> std::io::Result<()> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map(drop)
}

/// Check 1: the Keypad's node, found through sysfs and readable and
/// writable.
fn keypad(root: &Path) -> Check {
    const NAME: &str = "keypad";
    let node = match sysfs::find_keypad(&root.join("sys/class/hidraw"), &root.join("dev")) {
        Ok(node) => node,
        Err(SysfsError::NotFound { .. }) => {
            return Check::new(
                NAME,
                Status::Fail,
                "no MX Creative Keypad (046d:c354) found",
            )
            .fix("plug the Keypad in by USB");
        }
        Err(error) => {
            return Check::new(
                NAME,
                Status::Fail,
                format!("the Keypad was not found: {error}"),
            );
        }
    };
    let path = node.path.display();
    match open_rw(&node.path) {
        Ok(()) => Check::new(
            NAME,
            Status::Ok,
            format!("the Keypad is {path}, readable and writable"),
        ),
        Err(error) if error.kind() == ErrorKind::PermissionDenied => Check::new(
            NAME,
            Status::Fail,
            format!("the Keypad is {path}, but not readable and writable: {error}"),
        )
        .fix(RULE_INSTALL),
        Err(error) => Check::new(
            NAME,
            Status::Fail,
            format!("the Keypad is {path}, but cannot be opened: {error}"),
        ),
    }
}

/// Check 2: `/dev/uinput` writable.
fn uinput(root: &Path) -> Check {
    const NAME: &str = "uinput";
    let path = root.join("dev/uinput");
    let shown = path.display();
    match open_rw(&path) {
        Ok(()) => Check::new(
            NAME,
            Status::Ok,
            format!("{shown} is writable, chord keys can type"),
        ),
        Err(error) if error.kind() == ErrorKind::NotFound => Check::new(
            NAME,
            Status::Fail,
            format!("{shown} does not exist, chord keys will fail"),
        )
        .fix("sudo modprobe uinput"),
        Err(error) if error.kind() == ErrorKind::PermissionDenied => Check::new(
            NAME,
            Status::Fail,
            format!("{shown} is not writable, chord keys will fail: {error}"),
        )
        .fix(RULE_INSTALL),
        Err(error) => Check::new(
            NAME,
            Status::Fail,
            format!("{shown} cannot be opened: {error}"),
        ),
    }
}

/// Check 3: the rule installed and identical to the shipped one; a
/// warning only while checks 1 and 2 pass (`access`).
fn rule(root: &Path, access: bool) -> Check {
    const NAME: &str = "rule";
    let path = root.join(RULE_PATH);
    let shown = path.display();
    let problem = match std::fs::read_to_string(&path) {
        Ok(text) if text == RULE => {
            return Check::new(NAME, Status::Ok, format!("{shown} is the shipped rule"));
        }
        Ok(_) => format!("{shown} differs from the shipped rule"),
        Err(error) if error.kind() == ErrorKind::NotFound => format!("{shown} is not installed"),
        Err(error) => format!("{shown} cannot be read: {error}"),
    };
    let (status, message) = if access {
        (
            Status::Warn,
            format!("{problem}; the device access above comes from another setup"),
        )
    } else {
        (Status::Fail, problem)
    };
    Check::new(NAME, status, message).fix(RULE_INSTALL)
}

/// Check 4: a KDE Plasma session on Wayland.
fn session(inputs: &Inputs<'_>) -> Check {
    const NAME: &str = "session";
    let text = |name| {
        inputs
            .var(name)
            .map(|value| value.to_string_lossy().into_owned())
            .filter(|value| !value.is_empty())
    };
    let kind = text("XDG_SESSION_TYPE");
    let desktop = text("XDG_CURRENT_DESKTOP");
    let wayland = kind.as_deref() == Some("wayland");
    let kde = desktop
        .as_deref()
        .is_some_and(|desktop| desktop.split(':').any(|name| name == "KDE"));
    if wayland && kde {
        return Check::new(NAME, Status::Ok, "a KDE Plasma session on Wayland");
    }
    Check::new(
        NAME,
        Status::Warn,
        format!(
            "a {} session on {}: profiles follow the focused window on KWin only",
            kind.as_deref().unwrap_or("unknown"),
            desktop.as_deref().unwrap_or("an unknown desktop"),
        ),
    )
}

/// `~/.config`: `$XDG_CONFIG_HOME`, else `$HOME/.config`.
fn config_home(inputs: &Inputs<'_>) -> Option<PathBuf> {
    keyvo_core::layout::config_dir(inputs.var("XDG_CONFIG_HOME"), inputs.var("HOME"))
        .and_then(|dir| dir.parent().map(Path::to_path_buf))
}

/// The value of `key` in the `[group]` of a KDE config file.
fn kde_value<'a>(text: &'a str, group: &str, key: &str) -> Option<&'a str> {
    let header = format!("[{group}]");
    let mut inside = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            inside = line == header;
        } else if inside
            && let Some((name, value)) = line.split_once('=')
            && name.trim() == key
        {
            return Some(value.trim());
        }
    }
    None
}

/// Check 5: `KWin`'s focus stealing prevention at None or Low, so a launched
/// application's window takes the focus and switches the pad (#179).
fn focus(inputs: &Inputs<'_>) -> Check {
    const NAME: &str = "focus";
    const FIX: &str = "kwriteconfig6 --file kwinrc --group Windows --key FocusStealingPreventionLevel 1\n\
         qdbus6 org.kde.KWin /KWin reconfigure";
    let Some(path) = config_home(inputs).map(|dir| dir.join("kwinrc")) else {
        return Check::new(
            NAME,
            Status::Warn,
            "neither XDG_CONFIG_HOME nor HOME is set, kwinrc not read",
        );
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Check::new(
                NAME,
                Status::Warn,
                format!("{} cannot be read: {error}", path.display()),
            );
        }
    };
    let Some(value) = kde_value(&text, "Windows", "FocusStealingPreventionLevel") else {
        return Check::new(
            NAME,
            Status::Ok,
            "KWin's focus stealing prevention is at its default, Low",
        );
    };
    match value.parse::<usize>() {
        Ok(level @ 0..=1) => Check::new(
            NAME,
            Status::Ok,
            format!(
                "KWin's focus stealing prevention is {} ({level})",
                LEVELS[level]
            ),
        ),
        Ok(level) => Check::new(
            NAME,
            Status::Warn,
            format!(
                "KWin's focus stealing prevention is {} ({level}): a launched \
                 application's window does not take the focus, so launch keys \
                 do not switch the pad",
                LEVELS.get(level).copied().unwrap_or("above Extreme"),
            ),
        )
        .fix(FIX),
        Err(_) => Check::new(
            NAME,
            Status::Warn,
            format!(
                "{}: FocusStealingPreventionLevel `{value}` is not a level",
                path.display()
            ),
        )
        .fix(FIX),
    }
}

/// Check 6: the user unit installed and active.
fn unit(runner: &mut dyn Runner) -> Check {
    const NAME: &str = "unit";
    let Some(active) = runner.run("systemctl", &["--user", "is-active", UNIT]) else {
        return Check::new(
            NAME,
            Status::Warn,
            "systemctl not found, the user unit cannot be checked",
        );
    };
    let state = active.stdout.trim();
    if state == "active" {
        return Check::new(NAME, Status::Ok, "the user unit keyvo-daemon is active");
    }
    let enabled = runner
        .run("systemctl", &["--user", "is-enabled", UNIT])
        .map(|ran| ran.stdout.trim().to_owned())
        .unwrap_or_default();
    if enabled.is_empty() || enabled == "not-found" {
        return Check::new(
            NAME,
            Status::Warn,
            "the user unit keyvo-daemon is not installed",
        )
        .fix(UNIT_INSTALL);
    }
    Check::new(
        NAME,
        Status::Warn,
        format!(
            "the user unit keyvo-daemon is installed ({enabled}) but {}",
            if state.is_empty() {
                "not active"
            } else {
                state
            }
        ),
    )
    .fix("systemctl --user enable --now keyvo-daemon\njournalctl --user -u keyvo-daemon")
}

/// `Err` with the problem unless `dir` is a directory of `uid` with mode
/// `0700` and `socket` (the metadata of `path`) a socket with mode `0600`,
/// as the daemon creates them.
fn private(dir: &Path, path: &Path, socket: &std::fs::Metadata, uid: u32) -> Result<(), String> {
    let meta = std::fs::symlink_metadata(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    if !meta.is_dir() || meta.uid() != uid || meta.mode() & 0o777 != 0o700 {
        return Err(format!(
            "{} is mode {:o}, owner {}: not a directory of this user with mode 700",
            dir.display(),
            meta.mode() & 0o7777,
            meta.uid()
        ));
    }
    if !socket.file_type().is_socket() || socket.mode() & 0o777 != 0o600 {
        return Err(format!(
            "{} is mode {:o}: not a socket with mode 600",
            path.display(),
            socket.mode() & 0o7777
        ));
    }
    Ok(())
}

/// Check 7: the daemon's runtime directory `0700`, its socket `0600`, and
/// `status` answered within `[timing].doctor_reply`.
fn daemon(inputs: &Inputs<'_>) -> Check {
    const NAME: &str = "daemon";
    const START: &str = "systemctl --user start keyvo-daemon";
    let path = keyvo_client::socket_path(
        inputs.var("KEYVO_SOCKET"),
        inputs.var("XDG_RUNTIME_DIR"),
        inputs.uid,
    );
    let shown = path.display();
    let socket = match std::fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Check::new(
                NAME,
                Status::Warn,
                format!("no socket at {shown}: the daemon is not running"),
            )
            .fix(START);
        }
        Err(error) => {
            return Check::new(NAME, Status::Warn, format!("{shown}: {error}")).fix(START);
        }
    };
    let dir = path.parent().unwrap_or(Path::new("."));
    let restart = format!(
        "systemctl --user stop keyvo-daemon\nrm -r {}\n{START}",
        dir.display()
    );
    if let Err(problem) = private(dir, &path, &socket, inputs.uid) {
        return Check::new(NAME, Status::Fail, problem).fix(restart);
    }
    let stream = match keyvo_client::open(&path) {
        Ok(stream) => stream,
        Err(ClientError::Connect { .. }) => {
            return Check::new(
                NAME,
                Status::Warn,
                format!("nobody answers on {shown}: the daemon is not running"),
            )
            .fix(START);
        }
        Err(error) => return Check::new(NAME, Status::Fail, error.to_string()).fix(restart),
    };
    let reply = stream
        .set_read_timeout(Some(inputs.reply))
        .and_then(|()| stream.set_write_timeout(Some(inputs.reply)))
        .map_err(ClientError::from)
        .and_then(|()| Client::new(stream).request(Request::Status {}));
    match reply {
        Ok(Reply::Status(status)) => Check::new(
            NAME,
            Status::Ok,
            format!(
                "keyvo-daemon {} answers on {shown}, the Keypad is {}",
                status.version,
                if status.device.connected {
                    "connected"
                } else {
                    "not connected"
                }
            ),
        ),
        Ok(_) => Check::new(
            NAME,
            Status::Warn,
            format!("the daemon on {shown} answered `status` with another result"),
        ),
        Err(error) => Check::new(
            NAME,
            Status::Warn,
            format!(
                "the daemon on {shown} did not answer `status` within {} ms: {error}",
                inputs.reply.as_millis()
            ),
        )
        .fix("journalctl --user -u keyvo-daemon"),
    }
}

/// `true` when a process below `root/proc` has the command name `name`.
fn process_named(root: &Path, name: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(root.join("proc")) else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry
            .file_name()
            .to_string_lossy()
            .bytes()
            .all(|b| b.is_ascii_digit())
            && std::fs::read_to_string(entry.path().join("comm"))
                .is_ok_and(|comm| comm.trim_end() == name)
    })
}

/// Check 8: Solaar not running, as a Flatpak or a process.
fn solaar(root: &Path, runner: &mut dyn Runner) -> Check {
    const NAME: &str = "solaar";
    const RISK: &str = "it must set no divert on the Creative Console devices, \
                        they conflict with keyvo's own";
    let flatpak = runner
        .run("flatpak", &["ps", "--columns=application"])
        .is_some_and(|ran| ran.stdout.lines().any(|line| line.trim() == SOLAAR_FLATPAK));
    if flatpak {
        return Check::new(
            NAME,
            Status::Warn,
            format!("Solaar runs (Flatpak {SOLAAR_FLATPAK}): {RISK}"),
        )
        .fix(format!("flatpak kill {SOLAAR_FLATPAK}"));
    }
    if process_named(root, "solaar") {
        return Check::new(NAME, Status::Warn, format!("Solaar runs: {RISK}"))
            .fix("pkill -x solaar");
    }
    Check::new(NAME, Status::Ok, "Solaar is not running")
}

/// Check 9: the Flatpak app installed; informational, never a warning.
fn flatpak(runner: &mut dyn Runner) -> Check {
    const NAME: &str = "flatpak";
    let installed = runner
        .run("flatpak", &["info", APP_FLATPAK])
        .is_some_and(|ran| ran.success);
    let message = if installed {
        format!("the app {APP_FLATPAK} is installed")
    } else {
        format!("the app {APP_FLATPAK} is not installed (optional, docs/beta-test.md section 2)")
    };
    Check::new(NAME, Status::Ok, message)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;

    use super::*;

    /// Answers by `program args...`, `None` for a program not in the map.
    #[derive(Default)]
    struct Fake(BTreeMap<String, Ran>);

    impl Fake {
        fn with(mut self, line: &str, success: bool, stdout: &str) -> Self {
            self.0.insert(
                line.to_owned(),
                Ran {
                    success,
                    stdout: stdout.to_owned(),
                },
            );
            self
        }
    }

    impl Runner for Fake {
        fn run(&mut self, program: &str, args: &[&str]) -> Option<Ran> {
            let line = [&[program], args].concat().join(" ");
            let known = self
                .0
                .keys()
                .any(|key| key.starts_with(&format!("{program} ")));
            self.0.get(&line).cloned().or_else(|| {
                known.then(|| Ran {
                    success: false,
                    stdout: String::new(),
                })
            })
        }
    }

    fn uid() -> u32 {
        rustix::process::getuid().as_raw()
    }

    fn mode(path: &Path, mode: u32) {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    }

    fn env(pairs: &[(&str, &Path)]) -> BTreeMap<String, OsString> {
        pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.as_os_str().to_owned()))
            .collect()
    }

    fn inputs<'a>(root: &'a Path, env: &'a BTreeMap<String, OsString>) -> Inputs<'a> {
        Inputs {
            root,
            env,
            uid: uid(),
            reply: Duration::from_millis(200),
        }
    }

    #[test]
    fn no_keypad_in_sysfs_fails_with_plug_it_in() {
        let root = tempfile::tempdir().unwrap();
        let check = keypad(root.path());
        assert_eq!(check.status, Status::Fail);
        assert_eq!(check.fix.as_deref(), Some("plug the Keypad in by USB"));
    }

    #[test]
    fn an_unreadable_sysfs_entry_fails_with_the_error() {
        let root = tempfile::tempdir().unwrap();
        let node = root.path().join("sys/class/hidraw/hidraw2");
        std::fs::create_dir_all(&node).unwrap();
        let check = keypad(root.path());
        assert_eq!(check.status, Status::Fail);
        assert!(check.message.contains("cannot read"), "{}", check.message);
        assert_eq!(check.fix, None);
    }

    #[test]
    fn a_uinput_without_write_access_fails_with_the_rule() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("dev/uinput");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "").unwrap();
        mode(&path, 0o400);
        if open_rw(&path).is_ok() {
            return; // root ignores the mode
        }
        let check = uinput(root.path());
        assert_eq!(check.status, Status::Fail);
        assert_eq!(check.fix.as_deref(), Some(RULE_INSTALL));
    }

    #[test]
    fn the_rule_warns_with_access_and_fails_without() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(rule(root.path(), true).status, Status::Warn);
        let missing = rule(root.path(), false);
        assert_eq!(missing.status, Status::Fail);
        assert!(
            missing.message.ends_with("is not installed"),
            "{}",
            missing.message
        );
        let path = root.path().join(RULE_PATH);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "# an older rule\n").unwrap();
        let older = rule(root.path(), true);
        assert_eq!(older.status, Status::Warn);
        assert!(older.message.contains("differs"), "{}", older.message);
        assert_eq!(older.fix.as_deref(), Some(RULE_INSTALL));
        std::fs::write(&path, RULE).unwrap();
        assert_eq!(rule(root.path(), false).status, Status::Ok);
    }

    #[test]
    fn a_session_other_than_kde_on_wayland_warns() {
        let root = tempfile::tempdir().unwrap();
        let mut vars = BTreeMap::new();
        vars.insert("XDG_SESSION_TYPE".to_owned(), OsString::from("x11"));
        vars.insert("XDG_CURRENT_DESKTOP".to_owned(), OsString::from("GNOME"));
        let check = session(&inputs(root.path(), &vars));
        assert_eq!(check.status, Status::Warn);
        assert_eq!(
            check.message,
            "a x11 session on GNOME: profiles follow the focused window on KWin only"
        );
        let none = BTreeMap::new();
        assert!(
            session(&inputs(root.path(), &none))
                .message
                .contains("unknown")
        );
        vars.insert("XDG_SESSION_TYPE".to_owned(), OsString::from("wayland"));
        vars.insert(
            "XDG_CURRENT_DESKTOP".to_owned(),
            OsString::from("ubuntu:KDE"),
        );
        assert_eq!(session(&inputs(root.path(), &vars)).status, Status::Ok);
    }

    #[test]
    fn the_focus_level_reads_only_the_windows_group() {
        let root = tempfile::tempdir().unwrap();
        let vars = env(&[("XDG_CONFIG_HOME", root.path())]);
        let level = |text: &str| {
            std::fs::write(root.path().join("kwinrc"), text).unwrap();
            focus(&inputs(root.path(), &vars))
        };
        let default = focus(&inputs(root.path(), &vars));
        assert_eq!(default.status, Status::Ok);
        assert!(default.message.contains("default"), "{}", default.message);
        assert_eq!(
            level("[Other]\nFocusStealingPreventionLevel=4\n").status,
            Status::Ok
        );
        assert_eq!(
            level("[Windows]\nFocusStealingPreventionLevel=0\n").message,
            "KWin's focus stealing prevention is None (0)"
        );
        let extreme = level("[Windows]\nFocusStealingPreventionLevel = 4\n");
        assert_eq!(extreme.status, Status::Warn);
        assert!(
            extreme.message.contains("Extreme (4)"),
            "{}",
            extreme.message
        );
        assert!(
            level("[Windows]\nFocusStealingPreventionLevel=9\n")
                .message
                .contains("above Extreme")
        );
        let bad = level("[Windows]\nFocusStealingPreventionLevel=high\n");
        assert_eq!(bad.status, Status::Warn);
        assert!(bad.message.contains("is not a level"), "{}", bad.message);
        let unset = BTreeMap::new();
        assert_eq!(focus(&inputs(root.path(), &unset)).status, Status::Warn);
        std::fs::remove_file(root.path().join("kwinrc")).unwrap();
        std::fs::create_dir(root.path().join("kwinrc")).unwrap();
        assert!(
            focus(&inputs(root.path(), &vars))
                .message
                .contains("cannot be read")
        );
    }

    #[test]
    fn the_unit_is_active_inactive_missing_or_unknown() {
        let active = "systemctl --user is-active keyvo-daemon";
        let enabled = "systemctl --user is-enabled keyvo-daemon";
        assert_eq!(unit(&mut Fake::default()).status, Status::Warn);
        assert!(
            unit(&mut Fake::default())
                .message
                .contains("systemctl not found")
        );
        let ok = unit(&mut Fake::default().with(active, true, "active\n"));
        assert_eq!(ok.status, Status::Ok);
        let missing = unit(&mut Fake::default().with(active, false, "inactive\n"));
        assert_eq!(missing.status, Status::Warn);
        assert_eq!(missing.fix.as_deref(), Some(UNIT_INSTALL));
        let not_found = unit(&mut Fake::default().with(active, false, "inactive\n").with(
            enabled,
            false,
            "not-found\n",
        ));
        assert_eq!(not_found.fix.as_deref(), Some(UNIT_INSTALL));
        let stopped = unit(&mut Fake::default().with(active, false, "failed\n").with(
            enabled,
            false,
            "disabled\n",
        ));
        assert_eq!(
            stopped.message,
            "the user unit keyvo-daemon is installed (disabled) but failed"
        );
        let blank = unit(&mut Fake::default().with(active, false, "").with(
            enabled,
            true,
            "enabled\n",
        ));
        assert!(
            blank.message.ends_with("but not active"),
            "{}",
            blank.message
        );
    }

    /// A runtime directory `run/keyvo` below `root` with `dir_mode`, and the
    /// environment that points at it.
    fn runtime(root: &Path, dir_mode: u32) -> (PathBuf, BTreeMap<String, OsString>) {
        let run = root.join("run");
        let dir = run.join("keyvo");
        std::fs::create_dir_all(&dir).unwrap();
        mode(&dir, dir_mode);
        (dir.join("keyvo.sock"), env(&[("XDG_RUNTIME_DIR", &run)]))
    }

    #[test]
    fn no_socket_means_the_daemon_is_not_running() {
        let root = tempfile::tempdir().unwrap();
        let (_, vars) = runtime(root.path(), 0o700);
        let check = daemon(&inputs(root.path(), &vars));
        assert_eq!(check.status, Status::Warn);
        assert!(
            check.message.starts_with("no socket at"),
            "{}",
            check.message
        );
        assert_eq!(
            check.fix.as_deref(),
            Some("systemctl --user start keyvo-daemon")
        );
    }

    #[test]
    fn an_open_runtime_directory_or_socket_fails() {
        let root = tempfile::tempdir().unwrap();
        let (path, vars) = runtime(root.path(), 0o755);
        let _listener = UnixListener::bind(&path).unwrap();
        mode(&path, 0o600);
        let open_dir = daemon(&inputs(root.path(), &vars));
        assert_eq!(open_dir.status, Status::Fail);
        assert!(
            open_dir.message.contains("mode 755"),
            "{}",
            open_dir.message
        );
        mode(path.parent().unwrap(), 0o700);
        mode(&path, 0o666);
        let open_socket = daemon(&inputs(root.path(), &vars));
        assert_eq!(open_socket.status, Status::Fail);
        assert!(
            open_socket.message.contains("mode 666"),
            "{}",
            open_socket.message
        );
        assert!(open_socket.fix.unwrap().contains("rm -r"));
    }

    #[test]
    fn a_stale_socket_or_a_silent_daemon_warns() {
        let root = tempfile::tempdir().unwrap();
        let (path, vars) = runtime(root.path(), 0o700);
        drop(UnixListener::bind(&path).unwrap());
        mode(&path, 0o600);
        let stale = daemon(&inputs(root.path(), &vars));
        assert_eq!(stale.status, Status::Warn);
        assert!(
            stale.message.starts_with("nobody answers"),
            "{}",
            stale.message
        );
        std::fs::remove_file(&path).unwrap();
        let _listener = UnixListener::bind(&path).unwrap();
        mode(&path, 0o600);
        let silent = daemon(&inputs(root.path(), &vars));
        assert_eq!(silent.status, Status::Warn);
        assert!(
            silent.message.contains("within 200 ms"),
            "{}",
            silent.message
        );
        assert_eq!(
            silent.fix.as_deref(),
            Some("journalctl --user -u keyvo-daemon")
        );
    }

    #[test]
    fn solaar_as_a_process_warns_and_flatpak_info_is_informational() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(solaar(root.path(), &mut Fake::default()).status, Status::Ok);
        for (pid, comm) in [("self", "solaar\n"), ("12", "konsole\n")] {
            let dir = root.path().join("proc").join(pid);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("comm"), comm).unwrap();
        }
        assert_eq!(solaar(root.path(), &mut Fake::default()).status, Status::Ok);
        std::fs::write(root.path().join("proc/12/comm"), "solaar\n").unwrap();
        let process = solaar(root.path(), &mut Fake::default());
        assert_eq!(process.status, Status::Warn);
        assert_eq!(process.fix.as_deref(), Some("pkill -x solaar"));
        let missing = flatpak(&mut Fake::default().with("flatpak ps", true, ""));
        assert_eq!(missing.status, Status::Ok);
        assert!(
            missing.message.contains("not installed"),
            "{}",
            missing.message
        );
    }

    #[test]
    fn a_fail_anywhere_fails_the_run() {
        let ok = Check::new("a", Status::Ok, "fine");
        let warn = Check::new("b", Status::Warn, "hm");
        assert!(!failed(&[ok.clone(), warn.clone()]));
        assert!(failed(&[ok, warn, Check::new("c", Status::Fail, "no")]));
    }
}
