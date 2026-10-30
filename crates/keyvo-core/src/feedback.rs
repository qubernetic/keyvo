//! The automatic key states (spec §7.2, §9): an `exec` action shows
//! `running` while it runs, after `[render].long_run_threshold` the static
//! long-run look, then `success` for `[render].feedback_hold` or `error`
//! until the next press of the key or until the GUI inspects the trace
//! entry. Time comes from the caller's [`crate::clock::Clock`], so the
//! transitions are tested without sleeping.

use std::time::{Duration, Instant};

use crate::config::Render;
use crate::trace::Outcome;

/// What a key shows because of its last action.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Look {
    /// The key's own look.
    #[default]
    Idle,
    /// The action runs: dimmed icon and a spinner arc.
    Running,
    /// It runs longer than `[render].long_run_threshold`: a static badge.
    RunningLong,
    /// It succeeded: a green badge.
    Success,
    /// It failed: a red badge.
    Error,
    /// The key has the dial and the roller (spec §9): a ring in the accent
    /// and a dial badge in the top left corner, held or toggled.
    Modifier,
}

impl Look {
    /// The `states.<name>` of a key that overrides this look; `None` for
    /// [`Look::Idle`].
    #[must_use]
    pub const fn state(self) -> Option<&'static str> {
        match self {
            Self::Idle => None,
            Self::Running | Self::RunningLong => Some("running"),
            Self::Success => Some("success"),
            Self::Error => Some("error"),
            Self::Modifier => Some("modifier"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Running(Instant),
    Success(Instant),
    Error,
}

/// The automatic state of one key.
#[derive(Debug, Clone)]
pub struct Feedback {
    hold: Duration,
    long_run: Duration,
    phase: Phase,
}

impl Feedback {
    /// An idle key with the times of `[render]`.
    #[must_use]
    pub fn new(render: &Render) -> Self {
        Self {
            hold: render.feedback_hold,
            long_run: render.long_run_threshold,
            phase: Phase::Idle,
        }
    }

    /// The key was pressed: an `error` or `success` look ends; a running
    /// action keeps its look.
    pub fn pressed(&mut self) {
        if !matches!(self.phase, Phase::Running(_)) {
            self.phase = Phase::Idle;
        }
    }

    /// The key's action started running at `now`.
    pub fn started(&mut self, now: Instant) {
        self.phase = Phase::Running(now);
    }

    /// The key's action ended at `now` with `outcome`.
    pub fn finished(&mut self, outcome: &Outcome, now: Instant) {
        self.phase = if outcome.is_success() {
            Phase::Success(now)
        } else if outcome.is_error() {
            Phase::Error
        } else {
            Phase::Idle
        };
    }

    /// The GUI showed the trace entry: an `error` look ends.
    pub fn inspected(&mut self) {
        if self.phase == Phase::Error {
            self.phase = Phase::Idle;
        }
    }

    /// The look at `now`.
    #[must_use]
    pub fn look(&self, now: Instant) -> Look {
        match self.phase {
            Phase::Idle => Look::Idle,
            Phase::Running(since) if now.saturating_duration_since(since) >= self.long_run => {
                Look::RunningLong
            }
            Phase::Running(_) => Look::Running,
            Phase::Success(since) if now.saturating_duration_since(since) >= self.hold => {
                Look::Idle
            }
            Phase::Success(_) => Look::Success,
            Phase::Error => Look::Error,
        }
    }

    /// When after `now` the look changes without an event (running to
    /// long-run, success to idle), so the daemon repaints then.
    #[must_use]
    pub fn next_change(&self, now: Instant) -> Option<Instant> {
        let at = match self.phase {
            Phase::Running(since) => since + self.long_run,
            Phase::Success(since) => since + self.hold,
            Phase::Idle | Phase::Error => return None,
        };
        (at > now).then_some(at)
    }
}
