//! The systemd-logind session source (spec §6.2): the daemon's own session
//! on the system bus tells when it is locked.
//!
//! The session is the one `org.freedesktop.login1.Manager.GetSessionByPID`
//! names for the daemon's process, else `/org/freedesktop/login1/session/auto`
//! (logind's name for the caller's session or, for a process outside every
//! session such as a systemd user service, the user's display session),
//! resolved to its own path through its `Id`. Its `Lock` and `Unlock`
//! signals and every change of its `LockedHint` property are reported;
//! `LockedHint` is read once at the start, so a daemon started under the
//! lock starts locked. The screen locker sets `LockedHint`, so a lock
//! without `loginctl lock-session` (a shortcut, the idle timeout) and the
//! unlock with the password are seen as well.
//!
//! The `KWin` lock-screen window (class `kwin_wayland`, empty title) is not a
//! signal: it is a side effect, not a contract.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::future::Future;
use std::pin::Pin;

use futures_util::StreamExt;
use keyvo_core::session::{Session, SessionError, SessionSink, SessionWatcher};
use tokio::task::JoinHandle;
use zbus::message::Type;
use zbus::{Connection, MatchRule, Message, MessageStream};
use zbus::{ObjectPath, OwnedObjectPath, OwnedVariant};

/// logind's bus name.
pub const LOGIND: &str = "org.freedesktop.login1";
/// The manager object.
pub const MANAGER_PATH: &str = "/org/freedesktop/login1";
/// The manager interface.
pub const MANAGER: &str = "org.freedesktop.login1.Manager";
/// The session interface: `Lock`, `Unlock`, `LockedHint`, `Id`.
pub const SESSION: &str = "org.freedesktop.login1.Session";
/// The session logind resolves to the caller's.
pub const AUTO_PATH: &str = "/org/freedesktop/login1/session/auto";
/// Where every session object lives.
const SESSION_PREFIX: &str = "/org/freedesktop/login1/session/";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

/// The object path of the session `id`, escaped as systemd escapes a bus
/// path label: every byte but `A-Za-z`, and `0-9` after the first, as `_`
/// and two lowercase hex digits (`2` is `_32`).
#[must_use]
pub fn session_path(id: &str) -> String {
    let mut path = String::from(SESSION_PREFIX);
    if id.is_empty() {
        path.push('_');
    }
    for (index, byte) in id.bytes().enumerate() {
        if byte.is_ascii_alphabetic() || (index > 0 && byte.is_ascii_digit()) {
            path.push(char::from(byte));
        } else {
            let _ = write!(path, "_{byte:02x}");
        }
    }
    path
}

/// What a signal of the session object says about the lock: `Lock`,
/// `Unlock`, or a `PropertiesChanged` of the session interface that
/// carries `LockedHint`; `None` for anything else.
#[must_use]
pub fn decode(message: &Message) -> Option<Session> {
    let header = message.header();
    let interface = header.interface()?.as_str();
    let member = header.member()?.as_str();
    match (interface, member) {
        (SESSION, "Lock") => Some(Session::Locked),
        (SESSION, "Unlock") => Some(Session::Unlocked),
        (PROPERTIES, "PropertiesChanged") => {
            let (changed_interface, changed, _invalidated): (
                String,
                HashMap<String, OwnedVariant>,
                Vec<String>,
            ) = message.body().deserialize().ok()?;
            if changed_interface != SESSION {
                return None;
            }
            let hint = changed.get("LockedHint")?;
            bool::try_from(hint).ok().map(Session::from_locked)
        }
        _ => None,
    }
}

/// The path of the daemon's session: the one of process `pid`, else the
/// `auto` session's own path.
///
/// # Errors
///
/// When neither the manager nor the `auto` session answers.
pub async fn find_session(conn: &Connection, pid: u32) -> zbus::Result<String> {
    let by_pid = conn
        .call_method(
            Some(LOGIND),
            MANAGER_PATH,
            Some(MANAGER),
            "GetSessionByPID",
            &pid,
        )
        .await;
    match by_pid {
        Ok(reply) => {
            let path: OwnedObjectPath = reply.body().deserialize()?;
            Ok(path.as_str().to_owned())
        }
        Err(error) => {
            tracing::info!("no logind session for pid {pid} ({error}); using the auto session");
            let id: String = property(conn, AUTO_PATH, "Id").await?;
            Ok(session_path(&id))
        }
    }
}

/// The property `name` of the session at `path`.
async fn property<T>(conn: &Connection, path: &str, name: &str) -> zbus::Result<T>
where
    T: TryFrom<OwnedVariant>,
    T::Error: Into<zbus::Error>,
{
    let value: OwnedVariant = conn
        .call_method(
            Some(LOGIND),
            path,
            Some(PROPERTIES),
            "Get",
            &(SESSION, name),
        )
        .await?
        .body()
        .deserialize()?;
    T::try_from(value).map_err(Into::into)
}

/// Watches the session of process `pid` on `conn` until the connection
/// closes: the `LockedHint` at the start, then every lock and unlock, go
/// to `sink`.
///
/// # Errors
///
/// When logind does not answer or the connection fails or closes.
pub async fn watch(conn: &Connection, pid: u32, mut sink: SessionSink) -> zbus::Result<()> {
    let path = find_session(conn, pid).await?;
    let object = ObjectPath::try_from(path.as_str())?;
    let mut rule = MatchRule::builder().msg_type(Type::Signal);
    if conn.is_bus() {
        // Only logind speaks for the session. A peer-to-peer message has
        // no sender, and zbus would drop it under this rule.
        rule = rule.sender(LOGIND);
    }
    let rule = rule.path(object).build()?;
    // Subscribed before the first look, so a lock in between is not missed.
    let mut signals = MessageStream::for_match_rule(rule, conn, None).await?;
    let locked: bool = property(conn, &path, "LockedHint").await?;
    tracing::info!("watching the session lock of {path}; locked: {locked}");
    sink(Session::from_locked(locked));
    while let Some(message) = signals.next().await {
        if let Some(session) = decode(&message?) {
            sink(session);
        }
    }
    Err(zbus::Error::Failure("the system bus closed".into()))
}

/// How the watcher gets its D-Bus connection.
type Connect = Pin<Box<dyn Future<Output = zbus::Result<Connection>> + Send>>;

/// The [`SessionWatcher`] on logind: [`watch`] in its own tokio task, which
/// logs once why it ended and stops when the watcher is dropped.
pub struct LogindSessionWatcher {
    pid: u32,
    connect: Option<Connect>,
    task: Option<JoinHandle<()>>,
}

impl LogindSessionWatcher {
    /// A watcher of this process's session on the system bus.
    #[must_use]
    pub fn system() -> Self {
        Self::new(std::process::id(), Connection::system())
    }

    /// A watcher of the session of `pid` on the connection `connect`
    /// yields.
    pub fn new(
        pid: u32,
        connect: impl Future<Output = zbus::Result<Connection>> + Send + 'static,
    ) -> Self {
        Self {
            pid,
            connect: Some(Box::pin(connect)),
            task: None,
        }
    }
}

impl std::fmt::Debug for LogindSessionWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogindSessionWatcher")
            .field("pid", &self.pid)
            .field("started", &self.task.is_some())
            .finish_non_exhaustive()
    }
}

impl SessionWatcher for LogindSessionWatcher {
    /// Starts the watch and returns at once: a missing system bus or
    /// logind is logged by the task, once, and the daemon runs without
    /// lock detection.
    fn start(&mut self, sink: SessionSink) -> Result<(), SessionError> {
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|_| SessionError("the logind watcher needs a tokio runtime".into()))?;
        let connect = self
            .connect
            .take()
            .ok_or_else(|| SessionError("the logind watcher is already started".into()))?;
        let pid = self.pid;
        self.task = Some(runtime.spawn(async move {
            let result = match connect.await {
                Ok(conn) => watch(&conn, pid, sink).await,
                Err(error) => Err(error),
            };
            if let Err(error) = result {
                tracing::warn!("lock detection off, the pad stays live under a lock: {error}");
            }
        }));
        Ok(())
    }
}

impl Drop for LogindSessionWatcher {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use zbus::Variant as Value;

    use super::*;

    const PATH: &str = "/org/freedesktop/login1/session/_32";

    fn signal<B>(interface: &str, member: &str, body: &B) -> Message
    where
        B: zbus::export::serde::Serialize + zbus::DynamicType,
    {
        Message::signal(PATH, interface, member)
            .build(body)
            .unwrap()
    }

    /// The body of a `PropertiesChanged` of `interface` with `changed`.
    fn changed(interface: &str, changed: &[(&str, Value<'_>)]) -> Message {
        let changed: HashMap<&str, Value<'_>> = changed.iter().cloned().collect();
        signal(
            PROPERTIES,
            "PropertiesChanged",
            &(interface, changed, Vec::<String>::new()),
        )
    }

    #[test]
    fn lock_and_unlock_signals_are_the_lock_state() {
        assert_eq!(decode(&signal(SESSION, "Lock", &())), Some(Session::Locked));
        assert_eq!(
            decode(&signal(SESSION, "Unlock", &())),
            Some(Session::Unlocked)
        );
        assert_eq!(
            decode(&signal(SESSION, "PauseDevice", &(1u32, 2u32, "x"))),
            None
        );
        assert_eq!(decode(&signal(MANAGER, "Lock", &())), None);
    }

    #[test]
    fn a_locked_hint_change_is_the_lock_state() {
        let locked = changed(SESSION, &[("LockedHint", Value::Bool(true))]);
        assert_eq!(decode(&locked), Some(Session::Locked));
        let unlocked = changed(
            SESSION,
            &[
                ("IdleHint", Value::Bool(true)),
                ("LockedHint", Value::Bool(false)),
            ],
        );
        assert_eq!(decode(&unlocked), Some(Session::Unlocked));
        // Another property, another interface or a wrong type say nothing.
        assert_eq!(
            decode(&changed(SESSION, &[("IdleHint", Value::Bool(true))])),
            None
        );
        assert_eq!(
            decode(&changed(MANAGER, &[("LockedHint", Value::Bool(true))])),
            None
        );
        assert_eq!(
            decode(&changed(SESSION, &[("LockedHint", Value::U32(1))])),
            None
        );
        assert_eq!(decode(&signal(PROPERTIES, "PropertiesChanged", &"x")), None);
    }

    #[test]
    fn a_session_id_is_escaped_into_its_path() {
        assert_eq!(session_path("2"), "/org/freedesktop/login1/session/_32");
        assert_eq!(session_path("c1"), "/org/freedesktop/login1/session/c1");
        assert_eq!(session_path("12"), "/org/freedesktop/login1/session/_312");
        assert_eq!(session_path("a-b"), "/org/freedesktop/login1/session/a_2db");
        assert_eq!(session_path(""), "/org/freedesktop/login1/session/_");
    }

    #[test]
    fn a_second_start_is_refused() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let _guard = runtime.enter();
        let mut watcher =
            LogindSessionWatcher::new(42, async { Err(zbus::Error::Failure("no bus".into())) });
        assert!(format!("{watcher:?}").contains("started: false"));
        watcher.start(Box::new(|_| {})).unwrap();
        assert!(format!("{watcher:?}").contains("started: true"));
        assert_eq!(
            watcher.start(Box::new(|_| {})),
            Err(SessionError("the logind watcher is already started".into()))
        );
    }

    #[test]
    fn without_a_runtime_the_watcher_does_not_start() {
        let mut watcher = LogindSessionWatcher::system();
        assert_eq!(
            watcher.start(Box::new(|_| {})),
            Err(SessionError(
                "the logind watcher needs a tokio runtime".into()
            ))
        );
    }
}
