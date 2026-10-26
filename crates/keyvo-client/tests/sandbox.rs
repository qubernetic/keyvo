//! A client in another pid namespace, as in a Flatpak sandbox, sees the
//! daemon as pid 0 through `SO_PEERCRED` and still connects (#104).
//!
//! The namespace maps the user's own uid, as bwrap (Flatpak) does. Without
//! a mapping both sides would read as the overflow uid and `open` would pass
//! against a listener of any uid, so the child checks the mapping first.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::process::Command;

/// Where the parent's listener is, for the child run.
const SOCKET: &str = "KEYVO_TEST_SANDBOX_SOCKET";

/// The parent's uid, which the child must also run as.
const UID: &str = "KEYVO_TEST_SANDBOX_UID";

/// The child half: runs only when the parent starts it inside `unshare`.
#[test]
#[ignore = "started by open_succeeds_from_another_pid_namespace"]
fn open_from_inside_the_namespace() {
    let path = std::env::var_os(SOCKET).expect("started without the socket path");
    let uid = std::env::var(UID).expect("started without the parent's uid");
    assert_eq!(
        rustix::process::getuid().as_raw().to_string(),
        uid,
        "the namespace does not map the parent's uid"
    );
    keyvo_client::open(path.as_ref()).unwrap();
}

#[test]
fn open_succeeds_from_another_pid_namespace() {
    let unshare = || {
        let mut command = Command::new("unshare");
        command.args(["--user", "--map-current-user", "--pid", "--fork"]);
        command
    };
    match unshare().arg("true").output() {
        Ok(probe) if probe.status.success() => {}
        Ok(probe) => {
            let reason = String::from_utf8_lossy(&probe.stderr);
            println!("skipped: unshare is refused here: {}", reason.trim());
            return;
        }
        Err(error) => {
            println!("skipped: unshare is not available: {error}");
            return;
        }
    }

    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let path = dir.path().join("keyvo.sock");
    let _listener = UnixListener::bind(&path).unwrap();

    let child = unshare()
        .arg(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "open_from_inside_the_namespace",
            "--ignored",
            "--nocapture",
        ])
        .env(SOCKET, &path)
        .env(UID, rustix::process::getuid().as_raw().to_string())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&child.stdout);
    let stderr = String::from_utf8_lossy(&child.stderr);
    assert!(child.status.success(), "{stdout}{stderr}");
    assert!(stdout.contains("1 passed"), "{stdout}{stderr}");
}
