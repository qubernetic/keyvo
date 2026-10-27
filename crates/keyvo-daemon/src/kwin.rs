//! The `KWin` focus source (spec §6.2): the daemon serves a private D-Bus
//! object, loads `contrib/kwin/keyvo-focus.js` into `KWin` through
//! `org.kde.kwin.Scripting`, and the script calls the object with the
//! class, caption and pid of every activated window.
//!
//! `NameOwnerChanged` for `org.kde.KWin` loads the script again after a
//! `KWin` restart; every load checks the script directory and writes the
//! script anew first. A missing `KWin` is logged and waited for, a script
//! `KWin` refuses is logged; neither ends the watch. A stale copy from an
//! earlier daemon is unloaded first, so the script runs once.

use std::ffi::OsStr;
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Mutex, PoisonError};

use futures_util::StreamExt;
use keyvo_core::focus::{FocusSink, WatchError, WindowContext, WindowWatcher};
use tokio::task::JoinHandle;
use zbus::message::Type;
use zbus::{Connection, MatchRule, MessageStream, interface};

/// The script the daemon loads, with its placeholders.
pub const SCRIPT: &str = include_str!("../../../contrib/kwin/keyvo-focus.js");
/// The file the filled-in script is written to, in the script directory.
pub const SCRIPT_FILE: &str = "keyvo-focus.js";
/// The plugin name the script is loaded under.
pub const PLUGIN: &str = "keyvo-focus";
/// Path of the daemon's focus object.
pub const OBJECT_PATH: &str = "/org/qubernetic/keyvo/Focus";
/// Interface of the daemon's focus object.
pub const INTERFACE: &str = "org.qubernetic.keyvo.Focus";

const KWIN: &str = "org.kde.KWin";
const SCRIPTING_PATH: &str = "/Scripting";
const SCRIPTING: &str = "org.kde.kwin.Scripting";
const SCRIPT_INTERFACE: &str = "org.kde.kwin.Script";
const BUS: &str = "org.freedesktop.DBus";
const BUS_PATH: &str = "/org/freedesktop/DBus";

/// Why the watch ended.
#[derive(Debug, thiserror::Error)]
pub enum KWinError {
    /// The D-Bus connection failed or closed.
    #[error("D-Bus: {0}")]
    Bus(#[from] zbus::Error),
    /// The script could not be written for `KWin` to read.
    #[error("cannot write the KWin script: {0}")]
    Script(#[from] io::Error),
}

/// The private object the script calls.
struct FocusObject {
    sink: Mutex<FocusSink>,
}

#[interface(name = "org.qubernetic.keyvo.Focus")]
impl FocusObject {
    /// A window was activated or the active window's caption changed;
    /// `pid` is a decimal string, empty when unknown.
    fn activated(&self, class: String, caption: String, pid: &str) {
        let window = WindowContext {
            class,
            title: caption,
            pid: pid.parse().ok().filter(|pid| *pid > 0),
        };
        (self.sink.lock().unwrap_or_else(PoisonError::into_inner))(window);
    }
}

/// Where the daemon writes the script for `KWin`: the runtime directory
/// ([`crate::runtime::dir`]).
#[must_use]
pub fn script_dir(runtime_dir: Option<&OsStr>) -> PathBuf {
    crate::runtime::dir(runtime_dir)
}

/// Serves the focus object on `conn`, writes the script to `script_dir`
/// and keeps it loaded in `KWin` until the connection closes. Every
/// reported window goes to `sink`.
///
/// # Errors
///
/// [`KWinError::Script`] when the script cannot be written to a directory
/// only this user can write; [`KWinError::Bus`] when the connection fails
/// or closes.
pub async fn watch(conn: &Connection, script_dir: &Path, sink: FocusSink) -> Result<(), KWinError> {
    let service = conn
        .unique_name()
        .map(ToString::to_string)
        .unwrap_or_default();
    // Checked once up front so a directory that is not private ends the
    // watch before anything is served; `load` checks and writes again.
    write_script(script_dir, &service)?;
    conn.object_server()
        .at(
            OBJECT_PATH,
            FocusObject {
                sink: Mutex::new(sink),
            },
        )
        .await?;
    let mut rule = MatchRule::builder().msg_type(Type::Signal);
    if conn.is_bus() {
        // Only the bus may say who owns a name. A peer-to-peer message has
        // no sender, and zbus would drop it under this rule.
        rule = rule.sender(BUS);
    }
    let rule = rule
        .path(BUS_PATH)
        .interface(BUS)
        .member("NameOwnerChanged")
        .add_arg(KWIN)
        .build()?;
    // Subscribed before the first look, so a KWin starting in between is
    // not missed.
    let mut owners = MessageStream::for_match_rule(rule, conn, None).await?;
    let running: bool = conn
        .call_method(Some(BUS), BUS_PATH, Some(BUS), "NameHasOwner", &KWIN)
        .await?
        .body()
        .deserialize()?;
    if running {
        load(conn, script_dir, &service).await;
    } else {
        tracing::info!("KWin is not running; focus switching waits for it");
    }
    while let Some(message) = owners.next().await {
        let (name, _old, new): (String, String, String) = message?.body().deserialize()?;
        if name != KWIN {
            continue;
        }
        if new.is_empty() {
            tracing::info!("KWin is gone; focus switching waits for it");
        } else {
            load(conn, script_dir, &service).await;
        }
    }
    Err(zbus::Error::InputOutput(io::Error::from(io::ErrorKind::BrokenPipe).into()).into())
}

/// Writes the script for `service` into `dir`, which must be private: the
/// script runs inside `KWin`.
fn write_script(dir: &Path, service: &str) -> io::Result<PathBuf> {
    crate::runtime::ensure_private(dir)?;
    let path = dir.join(SCRIPT_FILE);
    let script = SCRIPT
        .replace("@SERVICE@", service)
        .replace("@PATH@", OBJECT_PATH)
        .replace("@INTERFACE@", INTERFACE);
    std::fs::write(&path, script)?;
    Ok(path)
}

/// Checks the directory, writes the script again and loads and runs it; a
/// failure is logged, the watch goes on. The check and the write come
/// before every load: in the `/tmp/keyvo-<uid>` fallback the directory can
/// age away between two `KWin` starts and be recreated by another account,
/// and the file `KWin` reads must be the one written here.
async fn load(conn: &Connection, dir: &Path, service: &str) {
    let path = match write_script(dir, service) {
        Ok(path) => path,
        Err(error) => {
            tracing::warn!("not loading the KWin script: {error}");
            return;
        }
    };
    match try_load(conn, &path).await {
        Ok(id) => tracing::info!("KWin script loaded as script {id}; focus switching on"),
        Err(error) => tracing::warn!("cannot load the KWin script: {error}"),
    }
}

async fn try_load(conn: &Connection, path: &Path) -> zbus::Result<i32> {
    let loaded: bool = conn
        .call_method(
            Some(KWIN),
            SCRIPTING_PATH,
            Some(SCRIPTING),
            "isScriptLoaded",
            &PLUGIN,
        )
        .await?
        .body()
        .deserialize()?;
    if loaded {
        conn.call_method(
            Some(KWIN),
            SCRIPTING_PATH,
            Some(SCRIPTING),
            "unloadScript",
            &PLUGIN,
        )
        .await?;
    }
    let id: i32 = conn
        .call_method(
            Some(KWIN),
            SCRIPTING_PATH,
            Some(SCRIPTING),
            "loadScript",
            &(path.to_string_lossy().as_ref(), PLUGIN),
        )
        .await?
        .body()
        .deserialize()?;
    if id < 0 {
        return Err(zbus::Error::Failure(format!(
            "KWin refused {} (loadScript returned {id})",
            path.display()
        )));
    }
    conn.call_method(
        Some(KWIN),
        format!("{SCRIPTING_PATH}/Script{id}").as_str(),
        Some(SCRIPT_INTERFACE),
        "run",
        &(),
    )
    .await?;
    Ok(id)
}

/// How the watcher gets its D-Bus connection.
type Connect = Pin<Box<dyn Future<Output = zbus::Result<Connection>> + Send>>;

/// The [`WindowWatcher`] on `KWin`: [`watch`] in its own tokio task, which
/// logs why it ended and stops when the watcher is dropped.
pub struct KWinWatcher {
    script_dir: PathBuf,
    connect: Option<Connect>,
    task: Option<JoinHandle<()>>,
}

impl KWinWatcher {
    /// A watcher on the session bus that writes its script to `script_dir`.
    #[must_use]
    pub fn session(script_dir: &Path) -> Self {
        Self::new(script_dir, Connection::session())
    }

    /// A watcher on the connection `connect` yields.
    pub fn new(
        script_dir: &Path,
        connect: impl Future<Output = zbus::Result<Connection>> + Send + 'static,
    ) -> Self {
        Self {
            script_dir: script_dir.to_path_buf(),
            connect: Some(Box::pin(connect)),
            task: None,
        }
    }
}

impl std::fmt::Debug for KWinWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KWinWatcher")
            .field("script_dir", &self.script_dir)
            .field("started", &self.task.is_some())
            .finish_non_exhaustive()
    }
}

impl WindowWatcher for KWinWatcher {
    /// Starts the watch and returns at once: a missing session bus or
    /// `KWin` is logged by the task, not returned.
    fn start(&mut self, sink: FocusSink) -> Result<(), WatchError> {
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|_| WatchError("the KWin watcher needs a tokio runtime".into()))?;
        let connect = self
            .connect
            .take()
            .ok_or_else(|| WatchError("the KWin watcher is already started".into()))?;
        let dir = self.script_dir.clone();
        self.task = Some(runtime.spawn(async move {
            let result = match connect.await {
                Ok(conn) => watch(&conn, &dir, sink).await,
                Err(error) => Err(error.into()),
            };
            if let Err(error) = result {
                tracing::warn!("focus switching off: {error}");
            }
        }));
        Ok(())
    }
}

impl Drop for KWinWatcher {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_start_is_refused() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let _guard = runtime.enter();
        let dir = tempfile::tempdir().unwrap();
        let mut watcher = KWinWatcher::new(dir.path(), async {
            Err(zbus::Error::Failure("no bus".into()))
        });
        assert!(format!("{watcher:?}").contains("started: false"));
        watcher.start(Box::new(|_| {})).unwrap();
        assert!(format!("{watcher:?}").contains("started: true"));
        assert_eq!(
            watcher.start(Box::new(|_| {})),
            Err(WatchError("the KWin watcher is already started".into()))
        );
    }
}
