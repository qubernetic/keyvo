//! The `SessionWatcher` platform trait (spec §4.1, §6.2): tells the daemon
//! when the user's session is locked. systemd-logind implements it in
//! `keyvo-daemon`; tests use [`FakeSessionWatcher`], through which a test
//! locks and unlocks the session by hand.
//!
//! While the session is locked the pad is inert (`SECURITY.md`): the lock
//! screen takes the input devices it knows, not the Keypad, and the daemon
//! runs in the session.

use std::sync::{Arc, Mutex, PoisonError};

/// Whether the user's session is locked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Session {
    /// The lock screen shows.
    Locked,
    /// The session is in use.
    Unlocked,
}

impl Session {
    /// [`Session::Locked`] when `locked`.
    #[must_use]
    pub const fn from_locked(locked: bool) -> Self {
        if locked { Self::Locked } else { Self::Unlocked }
    }

    /// Whether this is [`Session::Locked`].
    #[must_use]
    pub const fn is_locked(self) -> bool {
        matches!(self, Self::Locked)
    }
}

/// Where a watcher reports every lock and unlock, from any thread.
pub type SessionSink = Box<dyn FnMut(Session) + Send>;

/// A session lock source that cannot be watched.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("session watcher unavailable: {0}")]
pub struct SessionError(pub String);

/// Reports the lock state of the user's session.
pub trait SessionWatcher {
    /// Starts watching: every later lock and unlock goes to `sink` until
    /// the watcher is dropped. A watcher that knows the state reports it
    /// right away, so a daemon started under the lock starts locked.
    ///
    /// # Errors
    ///
    /// [`SessionError`] when the source is missing; the daemon then runs
    /// without lock detection.
    fn start(&mut self, sink: SessionSink) -> Result<(), SessionError>;
}

/// A [`SessionWatcher`] driven by the test. Clones share the sink, so a
/// test keeps one clone and hands the other to the daemon.
#[derive(Clone, Default)]
pub struct FakeSessionWatcher {
    sink: Arc<Mutex<Option<SessionSink>>>,
    failure: Option<String>,
}

impl FakeSessionWatcher {
    /// A watcher that starts and reports what the test tells it.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A watcher that refuses to start with `message`, as without logind.
    #[must_use]
    pub fn failing(message: &str) -> Self {
        Self {
            failure: Some(message.to_owned()),
            ..Self::default()
        }
    }

    /// Reports `session`; returns `false` when nobody started the watcher
    /// yet.
    pub fn report(&self, session: Session) -> bool {
        let mut sink = self.sink.lock().unwrap_or_else(PoisonError::into_inner);
        sink.as_mut().map(|sink| sink(session)).is_some()
    }
}

impl std::fmt::Debug for FakeSessionWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeSessionWatcher")
            .field("failure", &self.failure)
            .finish_non_exhaustive()
    }
}

impl SessionWatcher for FakeSessionWatcher {
    fn start(&mut self, sink: SessionSink) -> Result<(), SessionError> {
        if let Some(message) = &self.failure {
            return Err(SessionError(message.clone()));
        }
        *self.sink.lock().unwrap_or_else(PoisonError::into_inner) = Some(sink);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    #[test]
    fn the_fake_reports_to_the_started_sink() {
        let fake = FakeSessionWatcher::new();
        assert!(!fake.report(Session::Locked));
        let (sender, received) = mpsc::channel();
        fake.clone()
            .start(Box::new(move |session| sender.send(session).unwrap()))
            .unwrap();
        assert!(fake.report(Session::Locked));
        assert!(fake.report(Session::Unlocked));
        assert_eq!(received.try_recv().unwrap(), Session::Locked);
        assert_eq!(received.try_recv().unwrap(), Session::Unlocked);
        assert!(format!("{fake:?}").starts_with("FakeSessionWatcher"));
    }

    #[test]
    fn a_failing_fake_does_not_start() {
        let mut fake = FakeSessionWatcher::failing("no logind");
        assert_eq!(
            fake.start(Box::new(|_| {})),
            Err(SessionError("no logind".into()))
        );
        assert!(!fake.report(Session::Locked));
        assert_eq!(
            SessionError("no logind".into()).to_string(),
            "session watcher unavailable: no logind"
        );
    }

    #[test]
    fn a_session_says_whether_it_is_locked() {
        assert!(Session::Locked.is_locked());
        assert!(!Session::Unlocked.is_locked());
        assert_eq!(Session::from_locked(true), Session::Locked);
        assert_eq!(Session::from_locked(false), Session::Unlocked);
    }
}
