//! The logind session source without logind: the daemon's side of a
//! peer-to-peer zbus connection talks to a fake that plays the manager
//! (`GetSessionByPID`) and the session objects (`Id`, `LockedHint`), and
//! emits the session's `Lock`, `Unlock` and `PropertiesChanged` signals.

#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;
use std::os::unix::net::UnixStream;
use std::time::Duration;

use keyvo_core::session::{Session, SessionSink};
use keyvo_daemon::logind::{self, AUTO_PATH, MANAGER_PATH, SESSION};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use zbus::connection::Builder;
use zbus::{Connection, Guid, OwnedObjectPath, Variant, interface};

/// How long a test waits for one report before it fails.
const PATIENCE: Duration = Duration::from_secs(10);
/// How long a test waits to be sure a report does not come.
const QUIET: Duration = Duration::from_millis(200);
/// The daemon's process.
const PID: u32 = 4242;
/// The daemon's session, id `2`.
const OWN: &str = "/org/freedesktop/login1/session/_32";
/// Another session of the machine.
const OTHER: &str = "/org/freedesktop/login1/session/_33";

/// `org.freedesktop.login1.Manager` of the fake.
struct FakeManager {
    /// Whether the daemon's process is in a session.
    in_session: bool,
}

#[interface(name = "org.freedesktop.login1.Manager")]
impl FakeManager {
    #[zbus(name = "GetSessionByPID")]
    fn get_session_by_pid(&self, pid: u32) -> zbus::fdo::Result<OwnedObjectPath> {
        if self.in_session && pid == PID {
            Ok(OwnedObjectPath::try_from(OWN).unwrap())
        } else {
            Err(zbus::fdo::Error::Failed(format!(
                "PID {pid} does not belong to any known session"
            )))
        }
    }
}

/// `org.freedesktop.login1.Session` of one session of the fake.
struct FakeSession {
    id: String,
    locked: bool,
}

#[interface(name = "org.freedesktop.login1.Session")]
impl FakeSession {
    #[zbus(property, name = "Id")]
    fn id(&self) -> String {
        self.id.clone()
    }

    #[zbus(property, name = "LockedHint")]
    fn locked_hint(&self) -> bool {
        self.locked
    }
}

/// A fake logind whose own session starts `locked`, and the daemon's end
/// of the connection to it. Without `in_session` the manager knows no
/// session for the daemon and `auto` is the session `2`; without
/// `sessions` the fake serves no session at all.
async fn pair(in_session: bool, sessions: bool, locked: bool) -> (Connection, Connection) {
    let (left, right) = UnixStream::pair().unwrap();
    let mut fake = Builder::unix_stream(left)
        .server(Guid::generate())
        .p2p()
        .serve_at(MANAGER_PATH, FakeManager { in_session });
    if sessions {
        fake = fake
            .serve_at(
                OWN,
                FakeSession {
                    id: "2".into(),
                    locked,
                },
            )
            .serve_at(
                AUTO_PATH,
                FakeSession {
                    id: "2".into(),
                    locked,
                },
            );
    }
    let daemon = Builder::unix_stream(right).p2p().build();
    tokio::try_join!(fake.build(), daemon).unwrap()
}

/// A sink that forwards every report.
fn sink() -> (SessionSink, UnboundedReceiver<Session>) {
    let (sender, received) = mpsc::unbounded_channel();
    (
        Box::new(move |session| {
            let _ = sender.send(session);
        }),
        received,
    )
}

async fn next(reports: &mut UnboundedReceiver<Session>) -> Session {
    tokio::time::timeout(PATIENCE, reports.recv())
        .await
        .expect("no report")
        .unwrap()
}

async fn quiet(reports: &mut UnboundedReceiver<Session>) {
    let report = tokio::time::timeout(QUIET, reports.recv()).await;
    assert!(report.is_err(), "unexpected report {report:?}");
}

/// The fake emits `member` of the session interface on `path`.
async fn emit(fake: &Connection, path: &str, member: &str) {
    fake.emit_signal(None::<()>, path, SESSION, member, &())
        .await
        .unwrap();
}

/// The fake says `LockedHint` of the session at `path` is now `locked`.
async fn hint(fake: &Connection, path: &str, locked: bool) {
    let changed = HashMap::from([("LockedHint", Variant::Bool(locked))]);
    fake.emit_signal(
        None::<()>,
        path,
        "org.freedesktop.DBus.Properties",
        "PropertiesChanged",
        &(SESSION, changed, Vec::<String>::new()),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn the_hint_at_the_start_then_every_lock_and_unlock_are_reported() {
    let (fake, daemon) = pair(true, true, true).await;
    let (sink, mut reports) = sink();
    let watch = tokio::spawn(async move { logind::watch(&daemon, PID, sink).await });
    // Started under the lock: locked at once.
    assert_eq!(next(&mut reports).await, Session::Locked);
    emit(&fake, OWN, "Unlock").await;
    assert_eq!(next(&mut reports).await, Session::Unlocked);
    emit(&fake, OWN, "Lock").await;
    assert_eq!(next(&mut reports).await, Session::Locked);
    // The screen locker's unlock sets the hint.
    hint(&fake, OWN, false).await;
    assert_eq!(next(&mut reports).await, Session::Unlocked);
    // Another session's lock is not ours.
    emit(&fake, OTHER, "Lock").await;
    quiet(&mut reports).await;
    // The connection closing ends the watch with an error.
    drop(fake);
    let result = tokio::time::timeout(PATIENCE, watch)
        .await
        .unwrap()
        .unwrap();
    assert!(result.is_err());
}

#[tokio::test]
async fn a_daemon_outside_every_session_watches_the_auto_session() {
    let (fake, daemon) = pair(false, true, false).await;
    let (sink, mut reports) = sink();
    let _watch = tokio::spawn(async move { logind::watch(&daemon, PID, sink).await });
    assert_eq!(next(&mut reports).await, Session::Unlocked);
    // The auto session `2` emits on its own path.
    emit(&fake, OWN, "Lock").await;
    assert_eq!(next(&mut reports).await, Session::Locked);
}

#[tokio::test]
async fn without_a_session_the_watch_fails() {
    let (_fake, daemon) = pair(false, false, false).await;
    let (sink, mut reports) = sink();
    let result = logind::watch(&daemon, PID, sink).await;
    assert!(result.is_err());
    // The sink went with the watch, nothing reported.
    assert_eq!(reports.recv().await, None);
}

#[tokio::test]
async fn the_watcher_reports_through_its_task() {
    use keyvo_core::session::SessionWatcher;

    let (fake, daemon) = pair(true, true, false).await;
    let (sink, mut reports) = sink();
    let mut watcher = logind::LogindSessionWatcher::new(PID, async move { Ok(daemon) });
    watcher.start(sink).unwrap();
    assert_eq!(next(&mut reports).await, Session::Unlocked);
    emit(&fake, OWN, "Lock").await;
    assert_eq!(next(&mut reports).await, Session::Locked);
    drop(watcher);
}
