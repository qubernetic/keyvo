//! `config.toml`: the `[timing]` and `[focus]` (spec §6.1), `[render]`
//! (spec §9), `[trace]` and `[daemon]` (spec §8) and `[dialpad]` (spec
//! §7.2) tables, pre-alpha subset. Durations are integer milliseconds; a key left
//! out keeps its documented default; unknown keys are errors.

use std::num::NonZeroUsize;
use std::path::Path;
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::diagnostic::Diagnostic;

/// The whole `config.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(title = "keyvo config")]
pub struct Config {
    /// Timing constants, all in milliseconds (spec §6.1).
    pub timing: Timing,
    /// Rendering settings (spec §9).
    pub render: Render,
    /// Trace settings (spec §8).
    pub trace: Trace,
    /// Limits of the daemon's control socket (spec §8).
    pub daemon: Daemon,
    /// Which focused windows switch the context (spec §6.1).
    pub focus: Focus,
    /// How the Dialpad's turns run actions (spec §7.2).
    pub dialpad: DialpadConfig,
}

impl Config {
    /// Parses and validates the text of `config.toml`; `path` only labels
    /// the diagnostic.
    ///
    /// # Errors
    ///
    /// A [`Diagnostic`] with the position of the first problem.
    pub fn from_toml(text: &str, path: &Path) -> Result<Self, Diagnostic> {
        let config: Self =
            toml::from_str(text).map_err(|e| Diagnostic::from_toml(path, text, &e))?;
        let timing = config.timing;
        // The text parsed once already; the spans point at the key the
        // user set, the shorter one first.
        let spans = || toml::from_str::<Spans>(text).unwrap_or_default().timing;
        if let Some(message) = keep_alive_problem(
            ("`keep_alive_period`", timing.keep_alive_period),
            ("`keep_alive_timeout`", timing.keep_alive_timeout),
        ) {
            let spans = spans();
            let span = spans
                .keep_alive_period
                .or(spans.keep_alive_timeout)
                .map(|value| value.span());
            return Err(Diagnostic::at(path, text, span, message));
        }
        if let Some(message) = claim_splash_problem(
            ("`claim_splash_retry`", timing.claim_splash_retry),
            ("`claim_splash_timeout`", timing.claim_splash_timeout),
        ) {
            let spans = spans();
            let span = spans
                .claim_splash_retry
                .or(spans.claim_splash_timeout)
                .map(|value| value.span());
            return Err(Diagnostic::at(path, text, span, message));
        }
        Ok(config)
    }
}

/// The rule of spec §6.1 between the two splash-wait values of the display
/// claim: a retry not below the timeout would never retry. The message
/// names the two settings as given; `None` when the rule holds.
#[must_use]
pub fn claim_splash_problem(retry: (&str, Duration), timeout: (&str, Duration)) -> Option<String> {
    let ((retry_name, retry), (timeout_name, timeout)) = (retry, timeout);
    (retry >= timeout).then(|| {
        format!(
            "{retry_name} ({} ms) must be shorter than {timeout_name} ({} ms); the display \
             claim re-sends setMode(a0) every retry until the timeout",
            retry.as_millis(),
            timeout.as_millis()
        )
    })
}

/// The rule of spec §6.1 between the two keep-alive values: every ping has
/// to come while the timeout of the one before still runs, so the period
/// stays below the timeout. The message names the two settings as given
/// (config keys or command-line flags); `None` when the rule holds.
#[must_use]
pub fn keep_alive_problem(period: (&str, Duration), timeout: (&str, Duration)) -> Option<String> {
    let ((period_name, period), (timeout_name, timeout)) = (period, timeout);
    (period >= timeout).then(|| {
        format!(
            "{period_name} ({} ms) must be shorter than {timeout_name} ({} ms); the Keypad \
             drops the display claim when the timeout runs out before the next ping",
            period.as_millis(),
            timeout.as_millis()
        )
    })
}

/// Where the keep-alive values stand in the text, for the diagnostic.
#[derive(Debug, Default, Deserialize)]
struct Spans {
    #[serde(default)]
    timing: TimingSpans,
}

#[derive(Debug, Default, Deserialize)]
struct TimingSpans {
    keep_alive_period: Option<toml::Spanned<toml::Value>>,
    keep_alive_timeout: Option<toml::Spanned<toml::Value>>,
    claim_splash_retry: Option<toml::Spanned<toml::Value>>,
    claim_splash_timeout: Option<toml::Spanned<toml::Value>>,
}

/// The `[timing]` table (spec §6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Timing {
    /// A focus change shorter than this does not repaint. Default 150 ms.
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub focus_hysteresis: Duration,
    /// Repaints within this window are merged. Default 50 ms.
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub render_coalesce: Duration,
    /// Hold duration that turns a press into `long`. Default 1000 ms.
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub long_press: Duration,
    /// On a global page (list index 0 or below), the pad returns to the
    /// shown profile's context page after this long without a page-button
    /// or key press (spec §6.3); 0 keeps the page. Default 10000 ms.
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub page_return_global: Duration,
    /// On one of the shown profile's further pages (list index 2 or
    /// above), the pad returns to its context page after this long without
    /// a page-button or key press (spec §6.3); 0 keeps the page. Default 0
    /// (off).
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub page_return_own: Duration,
    /// Time between two keep-alive pings (spec §5.3); must be shorter than
    /// `keep_alive_timeout`. Default 1000 ms.
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub keep_alive_period: Duration,
    /// How long the device holds the claim without the next ping (spec
    /// §5.3). Default 3000 ms.
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub keep_alive_timeout: Duration,
    /// How long an image fragment waits for its ACK (spec §5.5). Default
    /// 50 ms.
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub ack_timeout: Duration,
    /// Wait before the first page-button divert request (spec §5.6).
    /// Default 100 ms.
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub divert_settle: Duration,
    /// Wait between two divert requests (spec §5.6). Default 20 ms.
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub divert_spacing: Duration,
    /// How long an HID++ request waits for its reply (spec §5.1). Default
    /// 1000 ms.
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub reply_timeout: Duration,
    /// An `exec` action still running after this long is killed and fails
    /// (spec §8). Default 30000 ms.
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub exec_timeout: Duration,
    /// How long a `launch` action is watched for an early exit (spec §8): a
    /// command still running after this long is let go and counts as
    /// started. Default 1000 ms.
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub launch_settle: Duration,
    /// How long a `launch` key's window is expected after the press (spec
    /// §6.3): a window that comes within it and belongs to the launch
    /// keeps a global page instead of showing the context page; 0 turns
    /// this off. Default 15000 ms.
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub launch_focus: Duration,
    /// Wait between two attempts to open the Keypad after it is gone (spec
    /// §5.1). Default 1000 ms.
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub reconnect_interval: Duration,
    /// Quiet time after the last change in the configuration directory
    /// before it is reloaded (spec §7.2). Default 100 ms.
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub reload_settle: Duration,
    /// Wait after a failed `accept` on the control socket before the next
    /// one, so a full file table does not spin the daemon (spec §8).
    /// Default 100 ms.
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub accept_backoff: Duration,
    /// While the display shows the plug-in splash (mode `a4`), the claim
    /// re-sends `setMode(a0)` this often (spec §5.4); must be shorter than
    /// `claim_splash_timeout`. Default 500 ms.
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub claim_splash_retry: Duration,
    /// How long the claim waits for the plug-in splash to end before it
    /// reports the display as not claimed (spec §5.4). Default 10000 ms.
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub claim_splash_timeout: Duration,
    /// How long `keyvo doctor` waits for the daemon's answer to `status`
    /// (spec §12). Default 1000 ms.
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub doctor_reply: Duration,
    /// How long the Dialpad search waits for one receiver slot's answer
    /// before it counts the slot as silent (spec §5.1). Default 1000 ms: a
    /// device's first reply from idle or sleep took up to 771 ms over Bolt.
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub slot_probe: Duration,
    /// An armed toggle modifier key ends after this long without a turn
    /// of the dial or the roller (spec §6.1); 0 keeps it until the next
    /// press. Default 0 (off).
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub modifier_timeout: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            focus_hysteresis: Duration::from_millis(150),
            render_coalesce: Duration::from_millis(50),
            long_press: Duration::from_millis(1000),
            page_return_global: Duration::from_millis(10_000),
            page_return_own: Duration::ZERO,
            keep_alive_period: Duration::from_millis(1000),
            keep_alive_timeout: Duration::from_millis(3000),
            ack_timeout: Duration::from_millis(50),
            divert_settle: Duration::from_millis(100),
            divert_spacing: Duration::from_millis(20),
            reply_timeout: Duration::from_millis(1000),
            exec_timeout: Duration::from_secs(30),
            launch_settle: Duration::from_millis(1000),
            launch_focus: Duration::from_secs(15),
            reconnect_interval: Duration::from_millis(1000),
            reload_settle: Duration::from_millis(100),
            accept_backoff: Duration::from_millis(100),
            claim_splash_retry: Duration::from_millis(500),
            claim_splash_timeout: Duration::from_millis(10_000),
            doctor_reply: Duration::from_millis(1000),
            slot_probe: Duration::from_millis(1000),
            modifier_timeout: Duration::ZERO,
        }
    }
}

/// The `[render]` table (spec §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Render {
    /// How long the `success` look stays before the key reverts. Default
    /// 2000 ms.
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub feedback_hold: Duration,
    /// After this long the `running` spinner becomes a static badge.
    /// Default 10000 ms.
    #[serde(with = "millis")]
    #[schemars(with = "u64")]
    pub long_run_threshold: Duration,
    /// Show the profile's accent on the whole grid for about 300 ms on a
    /// profile switch. Default `false` (spec §16, question 16).
    pub splash: bool,
}

impl Default for Render {
    fn default() -> Self {
        Self {
            feedback_hold: Duration::from_millis(2000),
            long_run_threshold: Duration::from_millis(10_000),
            splash: false,
        }
    }
}

/// The `[trace]` table (spec §8), pre-alpha subset: the ring buffer the
/// daemon keeps in memory; the file settings come with M1-8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Trace {
    /// Trace entries the daemon keeps in memory; the oldest goes first.
    /// Default 1000.
    pub ring: usize,
}

impl Default for Trace {
    fn default() -> Self {
        Self { ring: 1000 }
    }
}

/// The `[dialpad]` table (spec §5.8, §7.2): how the Dialpad's turns run
/// actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct DialpadConfig {
    /// The most action runs one turn report starts; a fast turn's larger
    /// delta runs a binding up to this often, the rest is dropped. At
    /// least 1. Default 8: a slow turn of the dial reports 1 to 6 detents,
    /// a fast one up to 18 (#187).
    pub max_per_event: std::num::NonZeroU32,
}

impl Default for DialpadConfig {
    fn default() -> Self {
        Self {
            max_per_event: std::num::NonZeroU32::MIN.saturating_add(8 - 1),
        }
    }
}

/// The `[daemon]` table (spec §8): limits of the control socket. They are
/// read when the daemon starts; a reload does not change them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Daemon {
    /// Longest request line in bytes, without its newline; a longer one
    /// gets an error response and the connection is closed. At least 1.
    /// Default 65536.
    pub socket_line_bytes: NonZeroUsize,
    /// Connections served at once; one more gets an `unavailable` error
    /// and is closed. At least 1. Default 32.
    pub socket_connections: NonZeroUsize,
}

impl Default for Daemon {
    fn default() -> Self {
        Self {
            socket_line_bytes: NonZeroUsize::MIN.saturating_add(64 * 1024 - 1),
            socket_connections: NonZeroUsize::MIN.saturating_add(32 - 1),
        }
    }
}

/// The `[focus]` table (spec §6.1): the window classes that hold the
/// context. While one of them has focus, the pad keeps the context and
/// profile of the last other window, so the keyvo app can configure the
/// profile it was opened from. Read when the daemon starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Focus {
    /// Window classes, compared without case; an empty list holds nothing.
    /// Default `["keyvo-app", "io.github.qubernetic.keyvo"]`, the keyvo
    /// app's own window: its class is the binary name on the host and the
    /// application id under Flatpak.
    pub hold_classes: Vec<String>,
}

impl Focus {
    /// Whether a window of `class` holds the context.
    #[must_use]
    pub fn holds(&self, class: &str) -> bool {
        self.hold_classes
            .iter()
            .any(|held| held.eq_ignore_ascii_case(class))
    }
}

impl Default for Focus {
    fn default() -> Self {
        Self {
            hold_classes: vec![
                "keyvo-app".to_owned(),
                "io.github.qubernetic.keyvo".to_owned(),
            ],
        }
    }
}

/// A [`Duration`] written as integer milliseconds.
mod millis {
    use std::fmt;
    use std::time::Duration;

    use serde::de::{self, Deserializer, Unexpected, Visitor};
    use serde::ser::Serializer;

    struct MillisVisitor;

    impl Visitor<'_> for MillisVisitor {
        type Value = Duration;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a duration in milliseconds")
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> Result<Duration, E> {
            u64::try_from(v)
                .map(Duration::from_millis)
                .map_err(|_| E::invalid_value(Unexpected::Signed(v), &self))
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> Result<Duration, E> {
            Ok(Duration::from_millis(v))
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
        deserializer.deserialize_u64(MillisVisitor)
    }

    pub fn serialize<S: Serializer>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
    }
}
