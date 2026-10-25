//! `exec` actions (spec §7.2, §8): the command runs through `sh -c` in its
//! own process group, never blocking the daemon, with the `KEYVO_*`
//! variables of the press in its environment. Past `[timing].exec_timeout`
//! the whole group is killed and the action fails.
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
use tokio::process::Command;

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
}
