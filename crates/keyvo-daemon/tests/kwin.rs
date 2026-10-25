//! The `KWin` focus source without `KWin`: the daemon's side of a
//! peer-to-peer zbus connection talks to a fake that plays the bus
//! (`NameHasOwner`, `NameOwnerChanged`) and `KWin`'s scripting interface,
//! and calls the daemon's focus object the way the script does.

#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::Duration;

use keyvo_core::focus::{WatchError, WindowContext, WindowWatcher};
use keyvo_daemon::kwin::{self, KWinWatcher};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use zbus::connection::Builder;
use zbus::{Connection, Guid, interface};

/// How long a test waits for one call before it fails.
const PATIENCE: Duration = Duration::from_secs(10);
/// How long a test waits to be sure a call does not come.
const QUIET: Duration = Duration::from_millis(200);
/// The script id the fake `KWin` hands out.
const SCRIPT_ID: i32 = 7;

/// What the fake saw, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Call {
    HasOwner,
    IsLoaded,
    Unload,
    Load { plugin: String, script: String },
    Run(i32),
}

/// The bus daemon part of the fake.
struct FakeBus {
    kwin: Arc<AtomicBool>,
    calls: UnboundedSender<Call>,
}

#[interface(name = "org.freedesktop.DBus")]
impl FakeBus {
    fn name_has_owner(&self, name: &str) -> bool {
        self.calls.send(Call::HasOwner).unwrap();
        name == "org.kde.KWin" && self.kwin.load(Ordering::SeqCst)
    }
}

/// `org.kde.kwin.Scripting` of the fake.
struct FakeScripting {
    loaded: Arc<AtomicBool>,
    next_id: Arc<AtomicI32>,
    calls: UnboundedSender<Call>,
}

#[interface(name = "org.kde.kwin.Scripting")]
impl FakeScripting {
    #[zbus(name = "isScriptLoaded")]
    fn is_script_loaded(&self, plugin: &str) -> bool {
        self.calls.send(Call::IsLoaded).unwrap();
        plugin == kwin::PLUGIN && self.loaded.load(Ordering::SeqCst)
    }

    #[zbus(name = "unloadScript")]
    fn unload_script(&self, plugin: &str) -> bool {
        assert_eq!(plugin, kwin::PLUGIN);
        self.calls.send(Call::Unload).unwrap();
        self.loaded.swap(false, Ordering::SeqCst)
    }

    #[zbus(name = "loadScript")]
    fn load_script(&self, path: String, plugin: String) -> i32 {
        let script = std::fs::read_to_string(path).unwrap();
        self.calls.send(Call::Load { plugin, script }).unwrap();
        let id = self.next_id.load(Ordering::SeqCst);
        self.loaded.store(id >= 0, Ordering::SeqCst);
        id
    }
}

/// `org.kde.kwin.Script` of the loaded script.
struct FakeScript {
    id: i32,
    calls: UnboundedSender<Call>,
}

#[interface(name = "org.kde.kwin.Script")]
impl FakeScript {
    #[zbus(name = "run")]
    fn run(&self) {
        self.calls.send(Call::Run(self.id)).unwrap();
    }
}

/// The fake's connection and switches, and what it saw.
struct Fake {
    peer: Connection,
    kwin: Arc<AtomicBool>,
    next_id: Arc<AtomicI32>,
    calls: UnboundedReceiver<Call>,
}

impl Fake {
    /// The next call, or a failure after [`PATIENCE`].
    async fn next(&mut self) -> Call {
        tokio::time::timeout(PATIENCE, self.calls.recv())
            .await
            .expect("no call from the daemon")
            .unwrap()
    }

    /// No call comes within [`QUIET`].
    async fn quiet(&mut self) {
        let call = tokio::time::timeout(QUIET, self.calls.recv()).await;
        assert!(call.is_err(), "unexpected call {call:?}");
    }

    /// The bus says `org.kde.KWin` now belongs to `new_owner` (`""`: to
    /// nobody).
    async fn owner_changed(&self, old_owner: &str, new_owner: &str) {
        self.peer
            .emit_signal(
                None::<()>,
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "NameOwnerChanged",
                &("org.kde.KWin", old_owner, new_owner),
            )
            .await
            .unwrap();
    }

    /// Calls the daemon's focus object as the script does.
    async fn activated(&self, class: &str, caption: &str, pid: &str) -> zbus::Result<()> {
        self.peer
            .call_method(
                None::<()>,
                kwin::OBJECT_PATH,
                Some(kwin::INTERFACE),
                "Activated",
                &(class, caption, pid),
            )
            .await
            .map(drop)
    }

    /// The fake expects one complete load: the stale check, `loadScript`
    /// and `run`; it returns the loaded script.
    async fn load(&mut self) -> String {
        assert_eq!(self.next().await, Call::IsLoaded);
        let Call::Load { plugin, script } = self.next().await else {
            panic!("no loadScript");
        };
        assert_eq!(plugin, kwin::PLUGIN);
        assert_eq!(self.next().await, Call::Run(SCRIPT_ID));
        script
    }
}

/// A fake `KWin` (running if `kwin`) and the daemon's end of the
/// connection to it.
async fn pair(kwin: bool) -> (Fake, Connection) {
    let (calls, received) = mpsc::unbounded_channel();
    let running = Arc::new(AtomicBool::new(kwin));
    let next_id = Arc::new(AtomicI32::new(SCRIPT_ID));
    let (left, right) = UnixStream::pair().unwrap();
    let fake = Builder::unix_stream(left)
        .server(Guid::generate())
        .p2p()
        .serve_at(
            "/org/freedesktop/DBus",
            FakeBus {
                kwin: Arc::clone(&running),
                calls: calls.clone(),
            },
        )
        .serve_at(
            "/Scripting",
            FakeScripting {
                loaded: Arc::new(AtomicBool::new(false)),
                next_id: Arc::clone(&next_id),
                calls: calls.clone(),
            },
        )
        .serve_at(
            format!("/Scripting/Script{SCRIPT_ID}"),
            FakeScript {
                id: SCRIPT_ID,
                calls,
            },
        )
        .build();
    let daemon = Builder::unix_stream(right).p2p().build();
    let (peer, daemon) = tokio::try_join!(fake, daemon).unwrap();
    let fake = Fake {
        peer,
        kwin: running,
        next_id,
        calls: received,
    };
    (fake, daemon)
}

/// A temporary script directory only this user can write, as the daemon
/// requires.
fn private_dir() -> tempfile::TempDir {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    dir
}

/// A sink that forwards every reported window.
fn sink() -> (
    keyvo_core::focus::FocusSink,
    UnboundedReceiver<WindowContext>,
) {
    let (sender, received) = mpsc::unbounded_channel();
    (
        Box::new(move |window| {
            let _ = sender.send(window);
        }),
        received,
    )
}

async fn next_window(windows: &mut UnboundedReceiver<WindowContext>) -> WindowContext {
    tokio::time::timeout(PATIENCE, windows.recv())
        .await
        .expect("no window reported")
        .unwrap()
}

/// Runs [`kwin::watch`] on `daemon` in its own task.
fn spawn_watch(
    daemon: Connection,
    dir: &Path,
    sink: keyvo_core::focus::FocusSink,
) -> tokio::task::JoinHandle<Result<(), kwin::KWinError>> {
    let dir = dir.to_path_buf();
    tokio::spawn(async move { kwin::watch(&daemon, &dir, sink).await })
}

#[tokio::test]
async fn the_script_is_loaded_and_run_when_kwin_is_there() {
    let (mut fake, daemon) = pair(true).await;
    let dir = private_dir();
    let (sink, _windows) = sink();
    let _watch = spawn_watch(daemon, dir.path(), sink);

    assert_eq!(fake.next().await, Call::HasOwner);
    let script = fake.load().await;
    assert!(script.contains(&format!("\"{}\"", kwin::OBJECT_PATH)));
    assert!(script.contains(&format!("\"{}\"", kwin::INTERFACE)));
    assert!(!script.contains('@'), "a placeholder is left: {script}");
    assert_eq!(
        std::fs::read_to_string(dir.path().join(kwin::SCRIPT_FILE)).unwrap(),
        script
    );
    fake.quiet().await;
}

#[tokio::test]
async fn the_focus_object_reports_class_caption_and_pid() {
    let (mut fake, daemon) = pair(true).await;
    let dir = private_dir();
    let (sink, mut windows) = sink();
    let _watch = spawn_watch(daemon, dir.path(), sink);
    assert_eq!(fake.next().await, Call::HasOwner);
    fake.load().await;

    fake.activated("org.kde.konsole", "~ : bash", "4242")
        .await
        .unwrap();
    assert_eq!(
        next_window(&mut windows).await,
        WindowContext::new("org.kde.konsole", "~ : bash").with_pid(4242)
    );
    fake.activated("code", "main.rs", "").await.unwrap();
    assert_eq!(
        next_window(&mut windows).await,
        WindowContext::new("code", "main.rs")
    );
    fake.activated("code", "lib.rs", "-1").await.unwrap();
    assert_eq!(
        next_window(&mut windows).await,
        WindowContext::new("code", "lib.rs")
    );
}

#[tokio::test]
async fn a_missing_kwin_waits_and_loads_when_kwin_appears() {
    let (mut fake, daemon) = pair(false).await;
    let dir = private_dir();
    let (sink, _windows) = sink();
    let watch = spawn_watch(daemon, dir.path(), sink);

    assert_eq!(fake.next().await, Call::HasOwner);
    fake.quiet().await;
    assert!(!watch.is_finished(), "a missing KWin ended the watch");

    fake.kwin.store(true, Ordering::SeqCst);
    fake.owner_changed("", ":1.9").await;
    fake.load().await;
}

#[tokio::test]
async fn the_script_is_reloaded_after_a_kwin_restart() {
    let (mut fake, daemon) = pair(true).await;
    let dir = private_dir();
    let (sink, _windows) = sink();
    let _watch = spawn_watch(daemon, dir.path(), sink);
    assert_eq!(fake.next().await, Call::HasOwner);
    fake.load().await;

    // KWin goes away: nothing to load into.
    fake.owner_changed(":1.9", "").await;
    fake.quiet().await;

    // The new KWin still lists the script (the fake keeps its state), so
    // the stale copy goes first.
    fake.owner_changed("", ":1.12").await;
    assert_eq!(fake.next().await, Call::IsLoaded);
    assert_eq!(fake.next().await, Call::Unload);
    let Call::Load { plugin, .. } = fake.next().await else {
        panic!("no loadScript");
    };
    assert_eq!(plugin, kwin::PLUGIN);
    assert_eq!(fake.next().await, Call::Run(SCRIPT_ID));
}

#[tokio::test]
async fn a_script_directory_opened_to_others_gets_no_load_after_a_restart() {
    use std::os::unix::fs::PermissionsExt;

    let (mut fake, daemon) = pair(true).await;
    let dir = private_dir();
    let (sink, _windows) = sink();
    let watch = spawn_watch(daemon, dir.path(), sink);
    assert_eq!(fake.next().await, Call::HasOwner);
    fake.load().await;

    // Another account could now swap the script before KWin reads it.
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
    fake.owner_changed(":1.9", "").await;
    fake.owner_changed("", ":1.12").await;
    fake.quiet().await;
    assert!(!watch.is_finished(), "a refused directory ended the watch");

    // Private again: the next KWin gets the script.
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    fake.owner_changed("", ":1.14").await;
    assert_eq!(fake.next().await, Call::IsLoaded);
    assert_eq!(fake.next().await, Call::Unload);
    assert!(matches!(fake.next().await, Call::Load { .. }));
}

#[tokio::test]
async fn an_overwritten_script_is_rewritten_before_the_next_load() {
    let (mut fake, daemon) = pair(true).await;
    let dir = private_dir();
    let (sink, _windows) = sink();
    let _watch = spawn_watch(daemon, dir.path(), sink);
    assert_eq!(fake.next().await, Call::HasOwner);
    let first = fake.load().await;

    std::fs::write(dir.path().join(kwin::SCRIPT_FILE), "print('not keyvo');").unwrap();
    fake.owner_changed("", ":1.12").await;
    assert_eq!(fake.next().await, Call::IsLoaded);
    assert_eq!(fake.next().await, Call::Unload);
    let Call::Load { script, .. } = fake.next().await else {
        panic!("no loadScript");
    };
    assert_eq!(script, first);
}

#[tokio::test]
async fn a_refused_script_is_logged_and_the_watch_goes_on() {
    let (mut fake, daemon) = pair(true).await;
    fake.next_id.store(-1, Ordering::SeqCst);
    let dir = private_dir();
    let (sink, _windows) = sink();
    let watch = spawn_watch(daemon, dir.path(), sink);

    assert_eq!(fake.next().await, Call::HasOwner);
    assert_eq!(fake.next().await, Call::IsLoaded);
    assert!(matches!(fake.next().await, Call::Load { .. }));
    fake.quiet().await;
    assert!(!watch.is_finished());

    fake.next_id.store(SCRIPT_ID, Ordering::SeqCst);
    fake.owner_changed("", ":1.12").await;
    fake.load().await;
}

#[tokio::test]
async fn the_watch_ends_with_the_connection() {
    let (mut fake, daemon) = pair(true).await;
    let dir = private_dir();
    let (sink, _windows) = sink();
    let watch = spawn_watch(daemon, dir.path(), sink);
    assert_eq!(fake.next().await, Call::HasOwner);
    fake.load().await;

    fake.peer.graceful_shutdown().await;
    let ended = tokio::time::timeout(PATIENCE, watch).await.unwrap();
    assert!(ended.unwrap().is_err());
}

#[tokio::test]
async fn an_unwritable_script_directory_ends_the_watch() {
    let (_fake, daemon) = pair(true).await;
    let dir = private_dir();
    let file = dir.path().join("file");
    std::fs::write(&file, "").unwrap();
    let (sink, _windows) = sink();
    let error = kwin::watch(&daemon, &file.join("keyvo"), sink)
        .await
        .unwrap_err();
    assert!(matches!(error, kwin::KWinError::Script(_)), "{error}");
}

#[tokio::test]
async fn a_script_directory_others_can_write_is_refused() {
    use std::os::unix::fs::PermissionsExt;

    let (_fake, daemon) = pair(true).await;
    let dir = private_dir();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
    let (sink, _windows) = sink();
    let error = kwin::watch(&daemon, dir.path(), sink).await.unwrap_err();
    assert!(error.to_string().contains("not private"), "{error}");
    assert!(!dir.path().join(kwin::SCRIPT_FILE).exists());
}

#[tokio::test]
async fn the_watcher_connects_and_reports_focus() {
    let (mut fake, daemon) = pair(true).await;
    let dir = private_dir();
    let (sink, mut windows) = sink();
    let mut watcher = KWinWatcher::new(dir.path(), async move { Ok(daemon) });
    watcher.start(sink).unwrap();

    assert_eq!(fake.next().await, Call::HasOwner);
    fake.load().await;
    fake.activated("org.kde.dolphin", "Home", "77")
        .await
        .unwrap();
    assert_eq!(
        next_window(&mut windows).await,
        WindowContext::new("org.kde.dolphin", "Home").with_pid(77)
    );

    // Dropping the watcher takes its object off the connection.
    drop(watcher);
    let gone = tokio::time::timeout(PATIENCE, async {
        loop {
            if fake
                .activated("org.kde.dolphin", "Home", "77")
                .await
                .is_err()
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await;
    assert!(gone.is_ok(), "the focus object outlived the watcher");
}

#[tokio::test]
async fn a_watcher_without_a_bus_starts_and_stays_quiet() {
    let dir = private_dir();
    let (sink, mut windows) = sink();
    let mut watcher = KWinWatcher::new(dir.path(), async {
        Err(zbus::Error::Failure("no session bus".into()))
    });
    watcher.start(sink).unwrap();
    let window = tokio::time::timeout(QUIET, windows.recv()).await;
    assert!(matches!(window, Ok(None) | Err(_)));
}

#[test]
fn a_watcher_needs_a_tokio_runtime() {
    let dir = private_dir();
    let mut watcher = KWinWatcher::session(dir.path());
    assert_eq!(
        watcher.start(Box::new(|_| {})),
        Err(WatchError("the KWin watcher needs a tokio runtime".into()))
    );
}

#[test]
fn the_script_file_starts_with_its_header_comment() {
    let header: Vec<&str> = kwin::SCRIPT
        .lines()
        .take_while(|line| line.starts_with("//"))
        .collect();
    assert!(header.len() > 5, "{header:?}");
    assert_eq!(header[0], "// SPDX-License-Identifier: Apache-2.0");
    assert!(header.iter().any(|line| line.contains("keyvo-daemon")));
    for placeholder in ["@SERVICE@", "@PATH@", "@INTERFACE@"] {
        assert!(kwin::SCRIPT.contains(&format!("\"{placeholder}\"")));
    }
    assert!(kwin::SCRIPT.contains("workspace.windowActivated.connect"));
}

#[test]
fn the_script_goes_to_the_runtime_directory() {
    assert_eq!(
        kwin::script_dir(Some("/run/user/1000".as_ref())),
        Path::new("/run/user/1000/keyvo")
    );
    for runtime_dir in [None, Some("".as_ref())] {
        let dir = kwin::script_dir(runtime_dir);
        assert!(dir.starts_with(std::env::temp_dir()), "{}", dir.display());
        let name = dir.file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with("keyvo-"), "{name}");
    }
}
