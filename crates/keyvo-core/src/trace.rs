//! The trace record (spec §8): what one press resolved to and what came of
//! it. `keyvo-core` defines the record and fills it in
//! [`crate::press::resolve`]; the daemon sets the result after executing,
//! keeps the ring buffer and writes the file.
//!
//! Pre-alpha subset: trigger, profile, page, key, resolved action, result.
//! The context snapshot, layer resolution and `on_fail` branch come with
//! their issues.

use serde::Serialize;

use crate::chord::KeyEvent;
use crate::press::PressKind;
use crate::profile::{Action, KeySlot};

/// What caused the press.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Trigger {
    /// Short or long.
    pub press: PressKind,
    /// A press from a client with `dry_run` (the simulator), not from the
    /// device; it resolves but executes nothing.
    pub simulated: bool,
}

/// One trace entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TraceRecord {
    /// Wall-clock time of the press, milliseconds since the Unix epoch.
    pub timestamp_ms: u64,
    /// What caused the press.
    pub trigger: Trigger,
    /// The active profile's id.
    pub profile: String,
    /// The active page, counted from 1.
    pub page: usize,
    /// The pressed key.
    pub key: KeySlot,
    /// The action the press resolved to; `None` for an unbound key.
    pub action: Option<Action>,
    /// What came of it.
    pub result: Outcome,
}

impl TraceRecord {
    /// Sets the result once the action has run.
    pub fn finish(&mut self, result: Outcome) {
        self.result = result;
    }
}

/// The result of a press.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Outcome {
    /// The key has no action on this page; nothing runs.
    Unbound,
    /// A simulated press: resolved, not executed.
    DryRun,
    /// Resolved and handed to the executor; the result is not in yet.
    Pending,
    /// A chord was sent as these key events.
    Sent {
        /// The presses and releases, in order.
        events: Vec<KeyEvent>,
    },
    /// An `exec` command ended.
    Exited {
        /// The exit code; `None` when a signal ended the process.
        code: Option<i32>,
        /// How long it ran, in milliseconds.
        duration_ms: u64,
    },
    /// The action could not run or did not end in time.
    Failed {
        /// What went wrong.
        error: String,
    },
}

impl Outcome {
    /// The action ran and succeeded: the key gets the `success` look.
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Sent { .. } | Self::Exited { code: Some(0), .. })
    }

    /// The action failed: the key gets the `error` look.
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Exited { .. } | Self::Failed { .. }) && !self.is_success()
    }
}
