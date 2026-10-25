//! The `InputInjector` platform trait (spec §4.1): sends a chord's key
//! events to the desktop. Linux implements it on uinput in `keyvo-daemon`;
//! tests use [`FakeInputInjector`], which records what it was given.

use std::sync::{Arc, Mutex, PoisonError};

use crate::chord::KeyEvent;

/// A failed injection.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("input injection failed: {0}")]
pub struct InjectError(pub String);

/// Synthesizes key presses and releases on the desktop.
pub trait InputInjector {
    /// Sends `events` in order, each press or release as its own report.
    ///
    /// # Errors
    ///
    /// [`InjectError`] when the device refuses the events or is missing.
    fn send(&mut self, events: &[KeyEvent]) -> Result<(), InjectError>;
}

/// An [`InputInjector`] that records every event. Clones share the record,
/// so a test keeps one clone and hands the other to the daemon.
#[derive(Debug, Clone, Default)]
pub struct FakeInputInjector {
    sent: Arc<Mutex<Vec<KeyEvent>>>,
    failure: Option<String>,
}

impl FakeInputInjector {
    /// An injector that accepts everything.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// An injector that refuses every send with `message`.
    #[must_use]
    pub fn failing(message: &str) -> Self {
        Self {
            failure: Some(message.to_owned()),
            ..Self::default()
        }
    }

    /// Every event sent so far, in order.
    #[must_use]
    pub fn sent(&self) -> Vec<KeyEvent> {
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl InputInjector for FakeInputInjector {
    fn send(&mut self, events: &[KeyEvent]) -> Result<(), InjectError> {
        if let Some(message) = &self.failure {
            return Err(InjectError(message.clone()));
        }
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(events);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chord::Chord;

    #[test]
    fn the_fake_records_events_for_every_clone() {
        let record = FakeInputInjector::new();
        let mut injector = record.clone();
        let events = Chord::parse("ctrl+c").unwrap().events();
        injector.send(&events).unwrap();
        assert_eq!(record.sent(), events);
    }

    #[test]
    fn a_failing_fake_records_nothing() {
        let mut injector = FakeInputInjector::failing("no uinput");
        let events = Chord::parse("a").unwrap().events();
        assert_eq!(injector.send(&events), Err(InjectError("no uinput".into())));
        assert!(injector.sent().is_empty());
        assert_eq!(
            InjectError("no uinput".into()).to_string(),
            "input injection failed: no uinput"
        );
    }
}
