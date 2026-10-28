//! `exec` actions (spec §7.2, §8): the command runs through `sh -c` in its
//! own process group, never blocking the daemon, with the `KEYVO_*`
//! variables of the press in its environment. Past `[timing].exec_timeout`
//! the whole group is killed and the action fails.
//!
//! `launch` actions run the same way in a new session and are let go once
//! they outlive `[timing].launch_settle`: an application keeps running past
//! `exec_timeout` and past the daemon.
//!
//! Pre-alpha environment: `KEYVO_KEY`, `KEYVO_PROFILE`, `KEYVO_PAGE`,
//! `KEYVO_PRESS`, `KEYVO_LAYER` and `KEYVO_SOCKET`. The window and project
//! variables come with their slices.

use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use keyvo_core::profile::GLOBAL_ID;
use keyvo_core::stack::Layer;
use keyvo_core::trace::{Outcome, TraceRecord};
use tokio::process::{Child, Command};

/// The `KEYVO_*` variables an `exec` started by `record` receives.
/// `KEYVO_PROFILE` is the active profile, `KEYVO_LAYER` the layer that
/// supplied the key: `global` for a global key under an app profile.
/// `KEYVO_SOCKET` is the daemon's control `socket`, so the command can
/// talk back (`keyvo ctl` reads the variable).
#[must_use]
pub fn env(record: &TraceRecord, socket: &Path) -> Vec<(&'static str, String)> {
    // A record without a layer ran no key; the profile's own layer then.
    let layer = record.layer.unwrap_or(if record.profile == GLOBAL_ID {
        Layer::Global
    } else {
        Layer::App
    });
    vec![
        ("KEYVO_KEY", record.key.get().to_string()),
        ("KEYVO_PROFILE", record.profile.clone()),
        ("KEYVO_PAGE", record.page.to_string()),
        ("KEYVO_PRESS", record.trigger.press.as_str().to_owned()),
        ("KEYVO_LAYER", layer.as_str().to_owned()),
        ("KEYVO_SOCKET", socket.to_string_lossy().into_owned()),
    ]
}

/// Runs `cmd` with `env` added to the daemon's environment and waits for
/// it at most `timeout`. Its output is not read; stderr goes to the
/// daemon's log.
pub async fn run(cmd: &str, env: &[(&'static str, String)], timeout: Duration) -> Outcome {
    let start = Instant::now();
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(cmd)
        .envs(env.iter().map(|(name, value)| (name, value)))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return Outcome::Failed {
                error: format!("cannot start `sh`: {error}"),
            };
        }
    };
    match tokio::time::timeout(timeout, child.wait()).await {
        Ok(Ok(status)) => Outcome::Exited {
            code: status.code(),
            duration_ms: millis(start.elapsed()),
        },
        Ok(Err(error)) => Outcome::Failed {
            error: format!("cannot wait for the command: {error}"),
        },
        Err(_) => {
            kill_group(child.id());
            // The group is gone or was never there; `kill_on_drop` reaps
            // the shell itself.
            let _ = child.kill().await;
            Outcome::Failed {
                error: format!("timed out after {} ms", millis(timeout)),
            }
        }
    }
}

/// Runs the `launch` command `cmd` like [`run`], but in a new session, and
/// watches it at most `settle`. A command that ends by then is reported as
/// [`run`] would; one still running is let go and has started. A task of
/// the daemon reaps it when it ends, so it leaves no zombie and changes no
/// result; once the daemon exits, the system reaps it.
pub async fn launch(cmd: &str, env: &[(&'static str, String)], settle: Duration) -> Outcome {
    match Launch::spawn(cmd, env) {
        Ok(launch) => launch.settle(settle).await,
        Err(outcome) => outcome,
    }
}

/// A `launch` command started and not yet settled; [`launch`] in two
/// steps, so the caller knows the process id before the settle.
#[derive(Debug)]
pub struct Launch {
    child: Child,
    start: Instant,
}

impl Launch {
    /// Starts the `launch` command `cmd` in a new session with `env` added
    /// to the daemon's environment.
    ///
    /// # Errors
    ///
    /// [`Outcome::Failed`] when `sh` cannot be started.
    pub fn spawn(cmd: &str, env: &[(&'static str, String)]) -> Result<Self, Outcome> {
        let start = Instant::now();
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg(cmd)
            .envs(env.iter().map(|(name, value)| (name, value)))
            .stdin(Stdio::null())
            .stdout(Stdio::null());
        new_session(&mut command);
        let child = command.spawn().map_err(|error| Outcome::Failed {
            error: format!("cannot start `sh`: {error}"),
        })?;
        Ok(Self { child, start })
    }

    /// The process id of the shell, which leads the launch's session.
    #[must_use]
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    /// Watches the command at most `settle`, as [`launch`] describes.
    pub async fn settle(self, settle: Duration) -> Outcome {
        let Self { mut child, start } = self;
        match tokio::time::timeout(settle, child.wait()).await {
            Ok(Ok(status)) => Outcome::Exited {
                code: status.code(),
                duration_ms: millis(start.elapsed()),
            },
            Ok(Err(error)) => Outcome::Failed {
                error: format!("cannot wait for the command: {error}"),
            },
            Err(_) => {
                let pid = child.id();
                tokio::spawn(async move {
                    let _ = child.wait().await;
                });
                // A child not yet waited for always has its id.
                pid.map_or_else(
                    || Outcome::Failed {
                        error: "the command has no process id".into(),
                    },
                    |pid| Outcome::Started { pid },
                )
            }
        }
    }
}

/// Starts the command of `command` in a new session (`setsid`), so it
/// leaves the daemon's process group and session and outlives both.
fn new_session(command: &mut Command) {
    #[cfg(target_os = "linux")]
    // SAFETY: the closure runs in the forked child before `exec`; it only
    // calls `setsid`, which is async-signal-safe, and allocates nothing.
    unsafe {
        command.pre_exec(|| {
            rustix::process::setsid()
                .map(drop)
                .map_err(std::io::Error::from)
        });
    }
    #[cfg(not(target_os = "linux"))]
    let _ = command;
}

/// Kills the process group led by `pid`, the command and everything it
/// started.
fn kill_group(pid: Option<u32>) {
    #[cfg(target_os = "linux")]
    if let Some(pid) = pid
        .and_then(|pid| i32::try_from(pid).ok())
        .and_then(rustix::process::Pid::from_raw)
    {
        // An error means the group has already gone.
        let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
    }
    #[cfg(not(target_os = "linux"))]
    let _ = pid;
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use keyvo_core::press::PressKind;
    use keyvo_core::profile::{Action, KeySlot};
    use keyvo_core::trace::Trigger;

    use super::*;

    const LONG: Duration = Duration::from_secs(20);

    fn record(profile: &str, layer: Layer, press: PressKind) -> TraceRecord {
        TraceRecord {
            timestamp_ms: 0,
            trigger: Trigger {
                press,
                simulated: false,
            },
            profile: profile.to_owned(),
            page: 2,
            key: KeySlot::new(7).unwrap(),
            layer: Some(layer),
            action: Some(Action::Exec { cmd: "true".into() }),
            result: Outcome::Pending,
            branch: None,
        }
    }

    const SOCKET: &str = "/run/user/1000/keyvo/keyvo.sock";

    #[test]
    fn the_environment_names_key_profile_page_press_layer_and_socket() {
        let env = env(
            &record("global", Layer::Global, PressKind::Long),
            Path::new(SOCKET),
        );
        assert_eq!(
            env,
            vec![
                ("KEYVO_KEY", "7".to_owned()),
                ("KEYVO_PROFILE", "global".to_owned()),
                ("KEYVO_PAGE", "2".to_owned()),
                ("KEYVO_PRESS", "long".to_owned()),
                ("KEYVO_LAYER", "global".to_owned()),
                ("KEYVO_SOCKET", SOCKET.to_owned()),
            ]
        );
        let app = super::env(
            &record("konsole", Layer::App, PressKind::Short),
            Path::new(SOCKET),
        );
        assert_eq!(app[4], ("KEYVO_LAYER", "app".to_owned()));
        // A global key under the konsole profile: the active profile, the
        // layer that supplied the key.
        let fallen = super::env(
            &record("konsole", Layer::Global, PressKind::Short),
            Path::new(SOCKET),
        );
        assert_eq!(fallen[1], ("KEYVO_PROFILE", "konsole".to_owned()));
        assert_eq!(fallen[4], ("KEYVO_LAYER", "global".to_owned()));
    }

    #[tokio::test]
    async fn the_exit_code_is_the_outcome() {
        assert!(matches!(
            run("exit 1", &[], LONG).await,
            Outcome::Exited { code: Some(1), .. }
        ));
        assert!(run("exit 0", &[], LONG).await.is_success());
        assert!(matches!(
            run("kill -9 $$", &[], LONG).await,
            Outcome::Exited { code: None, .. }
        ));
    }

    #[tokio::test]
    async fn the_command_sees_its_keyvo_variables() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("env");
        let cmd = format!(
            "printf '%s %s %s' \"$KEYVO_KEY\" \"$KEYVO_PRESS\" \"$KEYVO_SOCKET\" > '{}'",
            out.display()
        );
        let env = env(
            &record("global", Layer::Global, PressKind::Short),
            Path::new(SOCKET),
        );
        assert!(run(&cmd, &env, LONG).await.is_success());
        // The child can talk back to the daemon (`keyvo ctl`) on its socket.
        assert_eq!(
            std::fs::read_to_string(out).unwrap(),
            format!("7 short {SOCKET}")
        );
    }

    #[tokio::test]
    async fn a_command_past_the_timeout_is_killed_with_its_children() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("late");
        // The child of the shell would write the marker after the timeout.
        let cmd = format!("(sleep 1; touch '{}') & wait", marker.display());
        let start = std::time::Instant::now();
        let outcome = run(&cmd, &[], Duration::from_millis(100)).await;
        assert_eq!(
            outcome,
            Outcome::Failed {
                error: "timed out after 100 ms".into()
            }
        );
        assert!(start.elapsed() < Duration::from_secs(1));
        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert!(!marker.exists());
    }

    const SETTLE: Duration = Duration::from_millis(100);

    #[tokio::test]
    async fn a_launch_that_ends_within_the_settle_is_reported_like_exec() {
        assert!(matches!(
            launch("true", &[], LONG).await,
            Outcome::Exited { code: Some(0), .. }
        ));
        assert!(matches!(
            launch("exit 3", &[], LONG).await,
            Outcome::Exited { code: Some(3), .. }
        ));
        // `sh` reports a command it cannot find with 127.
        assert!(matches!(
            launch("keyvo-no-such-command-155", &[], LONG).await,
            Outcome::Exited {
                code: Some(127),
                ..
            }
        ));
    }

    /// The fields of `/proc/<pid>/stat` after the command name: the state
    /// is the first, the session id the fourth.
    #[cfg(target_os = "linux")]
    fn stat(pid: u32) -> Option<Vec<String>> {
        let text = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        let (_, rest) = text.rsplit_once(") ")?;
        Some(rest.split(' ').map(str::to_owned).collect())
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn a_launch_still_running_after_the_settle_is_let_go_in_its_own_session() {
        let start = std::time::Instant::now();
        let outcome = launch("sleep 5", &[], SETTLE).await;
        assert!(start.elapsed() < Duration::from_secs(1), "{outcome:?}");
        let Outcome::Started { pid } = outcome else {
            panic!("not started: {outcome:?}");
        };
        let fields = stat(pid).expect("a running child");
        assert_ne!(fields[0], "Z", "{fields:?}");
        // A new session led by the child, so it outlives the daemon's.
        assert_eq!(fields[3], pid.to_string(), "{fields:?}");
        for fd in [0, 1] {
            let target = std::fs::read_link(format!("/proc/{pid}/fd/{fd}")).unwrap();
            assert_eq!(target, Path::new("/dev/null"), "fd {fd}");
        }
        // The daemon reaps it when it ends: no zombie is left. The shell
        // leads its own group; killing the group also ends a `sleep` it
        // forked.
        kill_group(Some(pid));
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while stat(pid).is_some() {
            assert!(std::time::Instant::now() < deadline, "{:?}", stat(pid));
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}
