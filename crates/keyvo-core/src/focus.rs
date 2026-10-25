//! The `WindowWatcher` platform trait (spec §4.1, §6.2): tells the state
//! machine which window has focus. The `KWin` script and later X11 implement
//! it in `keyvo-daemon`; tests use [`FakeWindowWatcher`], through which a
//! test reports a focus change by hand.

use std::sync::{Arc, Mutex, PoisonError};

/// The focused window as a focus source reports it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WindowContext {
    /// The window class, reverse-DNS (`org.kde.konsole`) or short
    /// (`konsole`); `KWin`'s `resourceClass`.
    pub class: String,
    /// The window title; `KWin`'s `caption`.
    pub title: String,
    /// The process that owns the window, when the focus source knows it;
    /// `KWin`'s `pid`. Kept for project binding (spec §6.2); matching does
    /// not use it.
    pub pid: Option<u32>,
}

impl WindowContext {
    /// A window of `class` titled `title`, its process unknown.
    #[must_use]
    pub fn new(class: &str, title: &str) -> Self {
        Self {
            class: class.to_owned(),
            title: title.to_owned(),
            pid: None,
        }
    }

    /// The same window, owned by process `pid`.
    #[must_use]
    pub fn with_pid(mut self, pid: u32) -> Self {
        self.pid = Some(pid);
        self
    }
}

/// Where a watcher reports every focus change, from any thread.
pub type FocusSink = Box<dyn FnMut(WindowContext) + Send>;

/// A focus source that cannot be watched.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("window watcher unavailable: {0}")]
pub struct WatchError(pub String);

/// Reports the focused window.
pub trait WindowWatcher {
    /// Starts watching: every later focus change goes to `sink` until the
    /// watcher is dropped. A watcher that knows the focused window reports
    /// it right away.
    ///
    /// # Errors
    ///
    /// [`WatchError`] when the focus source is missing; the daemon then
    /// runs without focus switching.
    fn start(&mut self, sink: FocusSink) -> Result<(), WatchError>;
}

/// A [`WindowWatcher`] driven by the test. Clones share the sink, so a test
/// keeps one clone and hands the other to the daemon.
#[derive(Clone, Default)]
pub struct FakeWindowWatcher {
    sink: Arc<Mutex<Option<FocusSink>>>,
    failure: Option<String>,
}

impl FakeWindowWatcher {
    /// A watcher that starts and reports what the test tells it.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A watcher that refuses to start with `message`.
    #[must_use]
    pub fn failing(message: &str) -> Self {
        Self {
            failure: Some(message.to_owned()),
            ..Self::default()
        }
    }

    /// Reports `window` as focused; returns `false` when nobody started
    /// the watcher yet.
    pub fn focus(&self, window: WindowContext) -> bool {
        let mut sink = self.sink.lock().unwrap_or_else(PoisonError::into_inner);
        sink.as_mut().map(|sink| sink(window)).is_some()
    }
}

impl std::fmt::Debug for FakeWindowWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeWindowWatcher")
            .field("failure", &self.failure)
            .finish_non_exhaustive()
    }
}

impl WindowWatcher for FakeWindowWatcher {
    fn start(&mut self, sink: FocusSink) -> Result<(), WatchError> {
        if let Some(message) = &self.failure {
            return Err(WatchError(message.clone()));
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
        let fake = FakeWindowWatcher::new();
        let window = WindowContext::new("konsole", "~ : bash");
        assert!(!fake.focus(window.clone()));
        let (sender, received) = mpsc::channel();
        fake.clone()
            .start(Box::new(move |window| sender.send(window).unwrap()))
            .unwrap();
        assert!(fake.focus(window.clone()));
        assert_eq!(received.try_recv().unwrap(), window);
        assert!(format!("{fake:?}").starts_with("FakeWindowWatcher"));
    }

    #[test]
    fn a_window_carries_its_process_id_when_known() {
        let window = WindowContext::new("konsole", "~");
        assert_eq!(window.pid, None);
        let window = window.with_pid(4242);
        assert_eq!(window.pid, Some(4242));
        assert_eq!(window.class, "konsole");
    }

    #[test]
    fn a_failing_fake_does_not_start() {
        let mut fake = FakeWindowWatcher::failing("no KWin");
        assert_eq!(
            fake.start(Box::new(|_| {})),
            Err(WatchError("no KWin".into()))
        );
        assert!(!fake.focus(WindowContext::default()));
        assert_eq!(
            WatchError("no KWin".into()).to_string(),
            "window watcher unavailable: no KWin"
        );
    }
}
