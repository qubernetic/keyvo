//! The trace record (spec §8): what one press resolved to and what came of
//! it. `keyvo-core` defines the record and fills it in
//! [`crate::press::resolve`]; the daemon sets the result after executing,
//! keeps the ring buffer and writes the file.
//!
//! Pre-alpha subset: trigger, profile, page, key, the layer that supplied
//! the key, resolved action, result, and the branch: a key's `on_fail` run
//! is a second record of the same press. The context snapshot comes with
//! its issue.

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::chord::KeyEvent;
use crate::dialpad::Control;
use crate::press::PressKind;
use crate::profile::{Action, KeySlot, ModifierMode};
use crate::stack::Layer;

/// What caused the press.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trigger {
    /// Short or long.
    pub press: PressKind,
    /// A press from a client with `dry_run` (the simulator), not from the
    /// device; it resolves but executes nothing.
    pub simulated: bool,
}

/// One trace entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceRecord {
    /// Wall-clock time of the press, milliseconds since the Unix epoch.
    pub timestamp_ms: u64,
    /// What caused the press.
    pub trigger: Trigger,
    /// The active profile's id.
    pub profile: String,
    /// The active page, counted from 1.
    pub page: usize,
    /// The pressed key; absent for a Dialpad control.
    #[serde(
        default,
        deserialize_with = "crate::profile::optional_slot_number",
        skip_serializing_if = "Option::is_none"
    )]
    pub key: Option<KeySlot>,
    /// The Dialpad control turned or pressed (`dial`, `roller`,
    /// `button:N`); absent for a key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<Control>,
    /// The layer that supplied the key (spec §6.1): the active profile's
    /// own, or the global one under it; `None` when no layer defines it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<Layer>,
    /// A turn taken by a modifier key (`key` and `control` both set), or a
    /// press that changed one: how the key holds the dial and the roller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifier: Option<ModifierMode>,
    /// The action the press resolved to; `None` for an unbound key.
    #[serde(deserialize_with = "known_action_fields")]
    pub action: Option<Action>,
    /// What came of it.
    pub result: Outcome,
    /// The branch the record belongs to: `None` (written as an absent
    /// field, read as `null` too) for the key's own action,
    /// [`Branch::OnFail`] for the `on_fail` run after it failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<Branch>,
}

/// Which action of a press a record traces, beside the key's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Branch {
    /// The key's `on_fail`, run because its action failed.
    OnFail,
}

/// The fields an [`Action`] reads; a record drops the others.
const ACTION_FIELDS: [&str; 5] = ["type", "keys", "cmd", "class", "wait_focus"];

/// Reads the action of a record. A profile rejects unknown action fields,
/// a record ignores them, so a v1 client reads the records of a later
/// daemon (`docs/socket-api.md`).
fn known_action_fields<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Action>, D::Error> {
    let Some(fields) = Option::<Map<String, Value>>::deserialize(deserializer)? else {
        return Ok(None);
    };
    known(fields).map(Some).map_err(de::Error::custom)
}

/// Reads an action of a socket message the same way: unknown fields are
/// dropped, so a v1 client reads the actions of a later daemon.
pub(crate) fn known_action<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Action, D::Error> {
    known(Map::deserialize(deserializer)?).map_err(de::Error::custom)
}

fn known(mut fields: Map<String, Value>) -> Result<Action, serde_json::Error> {
    fields.retain(|name, _| ACTION_FIELDS.contains(&name.as_str()));
    Action::deserialize(Value::Object(fields))
}

impl TraceRecord {
    /// Sets the result once the action has run.
    pub fn finish(&mut self, result: Outcome) {
        self.result = result;
    }

    /// The record of the key's `on_fail` `action`, run after this press
    /// failed: the same press, trigger and key, on [`Branch::OnFail`],
    /// [`Outcome::Pending`] until it has run.
    #[must_use]
    pub fn on_fail(&self, action: Action) -> Self {
        Self {
            action: Some(action),
            result: Outcome::Pending,
            branch: Some(Branch::OnFail),
            ..self.clone()
        }
    }
}

/// The result of a press.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// A `launch` command still ran after `[timing].launch_settle` and was
    /// let go; its end is not reported.
    Started {
        /// The process id of the shell that runs the command.
        pid: u32,
    },
    /// The action could not run or did not end in time.
    Failed {
        /// What went wrong.
        error: String,
    },
    /// The press was not resolved and nothing ran: `session locked` while
    /// the session is locked (spec §8). No look, no `on_fail`.
    Blocked {
        /// Why.
        reason: String,
    },
    /// The press of a modifier key held or armed it, or let it go; no
    /// action ran (spec §7.2).
    Modifier {
        /// The key has the dial and the roller now.
        active: bool,
    },
}

impl Outcome {
    /// The action ran and succeeded: the key gets the `success` look.
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(
            self,
            Self::Sent { .. } | Self::Exited { code: Some(0), .. } | Self::Started { .. }
        )
    }

    /// The action failed: the key gets the `error` look.
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Exited { .. } | Self::Failed { .. }) && !self.is_success()
    }
}
