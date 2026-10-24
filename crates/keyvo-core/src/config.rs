//! `config.toml`: the `[timing]` (spec §6.1) and `[render]` (spec §9)
//! tables, pre-alpha subset. Durations are integer milliseconds; a key left
//! out keeps its documented default; unknown keys are errors.

use std::path::Path;
use std::time::Duration;

use schemars::JsonSchema;
use serde::Deserialize;
use serde::de::{self, Deserializer, Unexpected};

use crate::diagnostic::Diagnostic;

/// The whole `config.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(title = "keyvo config")]
pub struct Config {
    /// Timing constants, all in milliseconds (spec §6.1).
    pub timing: Timing,
    /// Rendering settings (spec §9).
    pub render: Render,
}

impl Config {
    /// Parses and validates the text of `config.toml`; `path` only labels
    /// the diagnostic.
    ///
    /// # Errors
    ///
    /// A [`Diagnostic`] with the position of the first problem.
    pub fn from_toml(text: &str, path: &Path) -> Result<Self, Diagnostic> {
        toml::from_str(text).map_err(|e| Diagnostic::from_toml(path, text, &e))
    }
}

/// The `[timing]` table (spec §6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Timing {
    /// A focus change shorter than this does not repaint. Default 150 ms.
    #[serde(deserialize_with = "millis")]
    #[schemars(with = "u64")]
    pub focus_hysteresis: Duration,
    /// Repaints within this window are merged. Default 50 ms.
    #[serde(deserialize_with = "millis")]
    #[schemars(with = "u64")]
    pub render_coalesce: Duration,
    /// Hold duration that turns a press into `long`. Default 1000 ms.
    #[serde(deserialize_with = "millis")]
    #[schemars(with = "u64")]
    pub long_press: Duration,
    /// After a page-button use, an app switch keeps the general page for
    /// this long (spec §6.3). Default 10000 ms.
    #[serde(deserialize_with = "millis")]
    #[schemars(with = "u64")]
    pub page_grace: Duration,
    /// Time between two keep-alive pings (spec §5.3). Default 1000 ms.
    #[serde(deserialize_with = "millis")]
    #[schemars(with = "u64")]
    pub keep_alive_period: Duration,
    /// How long the device holds the claim without the next ping (spec
    /// §5.3). Default 3000 ms.
    #[serde(deserialize_with = "millis")]
    #[schemars(with = "u64")]
    pub keep_alive_timeout: Duration,
    /// How long an image fragment waits for its ACK (spec §5.5). Default
    /// 50 ms.
    #[serde(deserialize_with = "millis")]
    #[schemars(with = "u64")]
    pub ack_timeout: Duration,
    /// Wait before the first page-button divert request (spec §5.6).
    /// Default 100 ms.
    #[serde(deserialize_with = "millis")]
    #[schemars(with = "u64")]
    pub divert_settle: Duration,
    /// Wait between two divert requests (spec §5.6). Default 20 ms.
    #[serde(deserialize_with = "millis")]
    #[schemars(with = "u64")]
    pub divert_spacing: Duration,
    /// How long an HID++ request waits for its reply (spec §5.1). Default
    /// 1000 ms.
    #[serde(deserialize_with = "millis")]
    #[schemars(with = "u64")]
    pub reply_timeout: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            focus_hysteresis: Duration::from_millis(150),
            render_coalesce: Duration::from_millis(50),
            long_press: Duration::from_millis(1000),
            page_grace: Duration::from_millis(10_000),
            keep_alive_period: Duration::from_millis(1000),
            keep_alive_timeout: Duration::from_millis(3000),
            ack_timeout: Duration::from_millis(50),
            divert_settle: Duration::from_millis(100),
            divert_spacing: Duration::from_millis(20),
            reply_timeout: Duration::from_millis(1000),
        }
    }
}

/// The `[render]` table (spec §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Render {
    /// How long the `success` look stays before the key reverts. Default
    /// 2000 ms.
    #[serde(deserialize_with = "millis")]
    #[schemars(with = "u64")]
    pub feedback_hold: Duration,
    /// After this long the `running` spinner becomes a static badge.
    /// Default 10000 ms.
    #[serde(deserialize_with = "millis")]
    #[schemars(with = "u64")]
    pub long_run_threshold: Duration,
}

impl Default for Render {
    fn default() -> Self {
        Self {
            feedback_hold: Duration::from_millis(2000),
            long_run_threshold: Duration::from_millis(10_000),
        }
    }
}

fn millis<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
    let n = i64::deserialize(deserializer)?;
    u64::try_from(n)
        .map(Duration::from_millis)
        .map_err(|_| de::Error::invalid_value(Unexpected::Signed(n), &"a duration in milliseconds"))
}
