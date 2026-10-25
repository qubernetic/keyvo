//! Presses (spec §6.1, §7.2): a key held for `[timing].long_press` is a
//! `long` press, released earlier a `short` one. The long press fires once
//! the time is reached, without waiting for the release; the caller polls
//! at [`PressDetector::next_deadline`].

use std::collections::BTreeMap;
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};

use crate::profile::{KeySlot, Profile};
use crate::trace::{Outcome, TraceRecord, Trigger};

/// How a key was pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PressKind {
    /// Released before `[timing].long_press`; a socket `press` without
    /// `kind`.
    #[default]
    Short,
    /// Held for `[timing].long_press`.
    Long,
}

impl PressKind {
    /// The name in the spec, the trace and `KEYVO_PRESS`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Short => "short",
            Self::Long => "long",
        }
    }
}

/// One held key: since when, and whether its long press has fired.
#[derive(Debug, Clone, Copy)]
struct Held {
    since: Instant,
    fired: bool,
}

/// Turns press and release edges of keys `K` into [`PressKind`]s.
#[derive(Debug, Clone)]
pub struct PressDetector<K> {
    long_press: Duration,
    held: BTreeMap<K, Held>,
}

impl<K: Ord + Copy> PressDetector<K> {
    /// A detector with the long press time `long_press`
    /// (`[timing].long_press`).
    #[must_use]
    pub fn new(long_press: Duration) -> Self {
        Self {
            long_press,
            held: BTreeMap::new(),
        }
    }

    /// `key` went down at `now`. A repeated report while it is held keeps
    /// the first time.
    pub fn press(&mut self, key: K, now: Instant) {
        self.held.entry(key).or_insert(Held {
            since: now,
            fired: false,
        });
    }

    /// `key` came up at `now`: the press kind, or `None` when the key was
    /// not held or its long press already fired in [`Self::poll`].
    pub fn release(&mut self, key: K, now: Instant) -> Option<PressKind> {
        let held = self.held.remove(&key)?;
        if held.fired {
            None
        } else if now.saturating_duration_since(held.since) >= self.long_press {
            Some(PressKind::Long)
        } else {
            Some(PressKind::Short)
        }
    }

    /// The keys whose long press fires at `now`, each once per hold.
    pub fn poll(&mut self, now: Instant) -> Vec<K> {
        let mut fired = Vec::new();
        for (key, held) in &mut self.held {
            if !held.fired && now.saturating_duration_since(held.since) >= self.long_press {
                held.fired = true;
                fired.push(*key);
            }
        }
        fired
    }

    /// When the next long press fires, if a key is held.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        self.held
            .values()
            .filter(|held| !held.fired)
            .map(|held| held.since + self.long_press)
            .min()
    }
}

/// Resolves a press of `key` on page `page` (counted from 1) of `profile`
/// at `at` to a trace record. Both press kinds run the key's action; a
/// simulated press resolves but is [`Outcome::DryRun`], an unbound key
/// [`Outcome::Unbound`], anything else [`Outcome::Pending`] until the
/// daemon has run it and calls [`TraceRecord::finish`].
#[must_use]
pub fn resolve(
    profile: &Profile,
    page: usize,
    key: KeySlot,
    trigger: Trigger,
    at: SystemTime,
) -> TraceRecord {
    let action = page
        .checked_sub(1)
        .and_then(|index| profile.pages.get(index))
        .and_then(|found| found.keys.get(&key))
        .and_then(|bound| bound.action.clone());
    let result = match (&action, trigger.simulated) {
        (None, _) => Outcome::Unbound,
        (Some(_), true) => Outcome::DryRun,
        (Some(_), false) => Outcome::Pending,
    };
    TraceRecord {
        timestamp_ms: at
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |since| {
                u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
            }),
        trigger,
        profile: profile.profile.id.clone(),
        page,
        key,
        action,
        result,
    }
}
