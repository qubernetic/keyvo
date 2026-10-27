//! `config.toml`: the fixtures under `tests/fixtures/config/`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use keyvo_core::config::{Config, Daemon, Focus, Render, Timing, Trace};
use keyvo_core::diagnostic::{Diagnostic, Position};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/config")
        .join(name)
}

fn parse(name: &str) -> Result<Config, Diagnostic> {
    let path = fixture(name);
    let text = std::fs::read_to_string(&path).unwrap();
    Config::from_toml(&text, &path)
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

#[test]
fn defaults_match_spec_6_1_and_9() {
    assert_eq!(
        Timing::default(),
        Timing {
            focus_hysteresis: ms(150),
            render_coalesce: ms(50),
            long_press: ms(1000),
            page_return_global: ms(10_000),
            page_return_own: ms(0),
            keep_alive_period: ms(1000),
            keep_alive_timeout: ms(3000),
            ack_timeout: ms(50),
            divert_settle: ms(100),
            divert_spacing: ms(20),
            reply_timeout: ms(1000),
            exec_timeout: ms(30_000),
            launch_settle: ms(1000),
            reconnect_interval: ms(1000),
            reload_settle: ms(100),
            accept_backoff: ms(100),
            claim_splash_retry: ms(500),
            claim_splash_timeout: ms(10_000),
        }
    );
    assert_eq!(
        Render::default(),
        Render {
            feedback_hold: ms(2000),
            long_run_threshold: ms(10_000),
            splash: false,
        }
    );
}

#[test]
fn the_profile_switch_splash_can_be_turned_on() {
    let config = Config::from_toml("[render]\nsplash = true\n", Path::new("config.toml")).unwrap();
    assert!(config.render.splash);
    let error = Config::from_toml("[render]\nsplash = 1\n", Path::new("config.toml")).unwrap_err();
    assert_eq!(error.position.map(|p| (p.line, p.column)), Some((2, 10)));
}

#[test]
fn the_trace_ring_holds_1000_entries_unless_set() {
    assert_eq!(Trace::default(), Trace { ring: 1000 });
    let config = Config::from_toml("[trace]\nring = 50\n", Path::new("config.toml")).unwrap();
    assert_eq!(config.trace.ring, 50);
    let error = Config::from_toml("[trace]\nring = -1\n", Path::new("config.toml")).unwrap_err();
    assert_eq!(error.position.map(|p| (p.line, p.column)), Some((2, 8)));
}

#[test]
fn the_socket_limits_default_to_64_kib_lines_and_32_connections() {
    let daemon = Daemon::default();
    assert_eq!(daemon.socket_line_bytes.get(), 65_536);
    assert_eq!(daemon.socket_connections.get(), 32);
    let config = Config::from_toml(
        "[daemon]\nsocket_line_bytes = 1024\nsocket_connections = 4\n[timing]\naccept_backoff = 500\n",
        Path::new("config.toml"),
    )
    .unwrap();
    assert_eq!(config.daemon.socket_line_bytes.get(), 1024);
    assert_eq!(config.daemon.socket_connections.get(), 4);
    assert_eq!(config.timing.accept_backoff, ms(500));
    for (text, column) in [
        ("[daemon]\nsocket_line_bytes = 0\n", 21),
        ("[daemon]\nsocket_connections = 0\n", 22),
    ] {
        let error = Config::from_toml(text, Path::new("config.toml")).unwrap_err();
        assert_eq!(
            error.position.map(|p| (p.line, p.column)),
            Some((2, column)),
            "{text}"
        );
    }
}

#[test]
fn the_keyvo_app_holds_the_context_unless_set() {
    // The app's window class is `keyvo-app` on the host and the
    // application id under Flatpak.
    assert_eq!(
        Focus::default(),
        Focus {
            hold_classes: vec![
                "keyvo-app".to_owned(),
                "io.github.qubernetic.keyvo".to_owned()
            ],
        }
    );
    assert!(Focus::default().holds("io.github.qubernetic.keyvo"));
    let config = parse("valid/focus.toml").unwrap();
    assert_eq!(
        config.focus.hold_classes,
        ["keyvo-app", "org.kde.plasmashell"]
    );
    // An empty list holds nothing.
    let config =
        Config::from_toml("[focus]\nhold_classes = []\n", Path::new("config.toml")).unwrap();
    assert!(config.focus.hold_classes.is_empty());
    assert!(!config.focus.holds("keyvo-app"));
}

#[test]
fn a_hold_class_matches_case_insensitively() {
    let focus = Focus::default();
    assert!(focus.holds("keyvo-app"));
    assert!(focus.holds("Keyvo-App"));
    assert!(!focus.holds("org.kde.konsole"));
    assert!(!focus.holds("keyvo"));
}

#[test]
fn an_empty_file_is_the_defaults() {
    assert_eq!(
        Config::from_toml("", Path::new("config.toml")),
        Ok(Config::default())
    );
}

#[test]
fn keys_left_out_keep_their_default() {
    let config = parse("valid/partial.toml").unwrap();
    assert_eq!(
        config.timing,
        Timing {
            focus_hysteresis: ms(200),
            long_press: ms(800),
            ..Timing::default()
        }
    );
    assert_eq!(
        config.render,
        Render {
            feedback_hold: ms(3000),
            ..Render::default()
        }
    );
}

#[test]
fn invalid_fixtures_report_file_line_and_column() {
    let cases = [
        (
            "invalid/unknown-timing-key.toml",
            Position { line: 3, column: 1 },
            "unknown field `long_pres`",
        ),
        (
            // Removed by #157: page_return_global and page_return_own.
            "invalid/page-grace-removed.toml",
            Position { line: 2, column: 1 },
            "unknown field `page_grace`",
        ),
        (
            "invalid/focus-hold-class-number.toml",
            Position {
                line: 2,
                column: 30,
            },
            "invalid type: integer `7`, expected a string",
        ),
        (
            "invalid/negative-duration.toml",
            Position {
                line: 2,
                column: 22,
            },
            "invalid value: integer `-1`, expected a duration in milliseconds",
        ),
    ];
    for (name, position, message) in cases {
        let diagnostic = parse(name).unwrap_err();
        assert_eq!(diagnostic.position, Some(position), "{diagnostic}");
        assert!(diagnostic.message.starts_with(message), "{diagnostic}");
    }
}

#[test]
fn a_keep_alive_period_not_below_the_timeout_is_refused() {
    let cases = [
        (
            "invalid/keep-alive-period-equal.toml",
            4,
            "`keep_alive_period` (3000 ms) must be shorter than `keep_alive_timeout` (3000 ms)",
        ),
        (
            "invalid/keep-alive-period-above.toml",
            2,
            "`keep_alive_period` (3500 ms) must be shorter than `keep_alive_timeout` (3000 ms)",
        ),
    ];
    for (name, line, message) in cases {
        let diagnostic = parse(name).unwrap_err();
        assert_eq!(diagnostic.path, fixture(name));
        assert_eq!(
            diagnostic.position,
            Some(Position { line, column: 21 }),
            "{diagnostic}"
        );
        assert!(diagnostic.message.starts_with(message), "{diagnostic}");
    }
    // A timeout lowered to the period points at the timeout.
    let text = "[timing]\nkeep_alive_timeout = 1000\n";
    let diagnostic = Config::from_toml(text, Path::new("config.toml")).unwrap_err();
    assert_eq!(
        diagnostic.position,
        Some(Position {
            line: 2,
            column: 22
        })
    );
    // Just below is fine.
    let text = "[timing]\nkeep_alive_period = 2999\n";
    assert!(Config::from_toml(text, Path::new("config.toml")).is_ok());
}

#[test]
fn the_page_return_keys_parse() {
    let text = "[timing]\npage_return_global = 0\npage_return_own = 30000\n";
    let timing = Config::from_toml(text, Path::new("config.toml"))
        .unwrap()
        .timing;
    assert_eq!(timing.page_return_global, ms(0));
    assert_eq!(timing.page_return_own, ms(30_000));
}

#[test]
fn the_claim_splash_keys_parse() {
    let text = "[timing]\nclaim_splash_retry = 250\nclaim_splash_timeout = 15000\n";
    let timing = Config::from_toml(text, Path::new("config.toml"))
        .unwrap()
        .timing;
    assert_eq!(timing.claim_splash_retry, ms(250));
    assert_eq!(timing.claim_splash_timeout, ms(15_000));
}

#[test]
fn a_claim_splash_retry_not_below_the_timeout_is_refused() {
    let name = "invalid/claim-splash-retry-equal.toml";
    let diagnostic = parse(name).unwrap_err();
    assert_eq!(diagnostic.path, fixture(name));
    // The retry the user set, first.
    assert_eq!(
        diagnostic.position,
        Some(Position {
            line: 4,
            column: 22
        }),
        "{diagnostic}"
    );
    assert!(
        diagnostic.message.starts_with(
            "`claim_splash_retry` (2000 ms) must be shorter than `claim_splash_timeout` (2000 ms)"
        ),
        "{diagnostic}"
    );
    // Above the default timeout, with only the retry set.
    let text = "[timing]\nclaim_splash_retry = 12000\n";
    let diagnostic = Config::from_toml(text, Path::new("config.toml")).unwrap_err();
    assert_eq!(
        diagnostic.position,
        Some(Position {
            line: 2,
            column: 22
        })
    );
    // A timeout lowered to the default retry points at the timeout.
    let text = "[timing]\nclaim_splash_timeout = 500\n";
    let diagnostic = Config::from_toml(text, Path::new("config.toml")).unwrap_err();
    assert_eq!(
        diagnostic.position,
        Some(Position {
            line: 2,
            column: 24
        })
    );
    assert!(
        diagnostic.to_string().contains("config.toml"),
        "{diagnostic}"
    );
    // Just below is fine.
    let text = "[timing]\nclaim_splash_retry = 9999\n";
    assert!(Config::from_toml(text, Path::new("config.toml")).is_ok());
}

#[test]
fn the_keep_alive_rule_names_the_keys_it_is_given() {
    assert_eq!(
        keyvo_core::config::keep_alive_problem(
            ("--keep-alive-period", ms(3000)),
            ("--keep-alive-timeout", ms(3000)),
        )
        .unwrap()
        .split(';')
        .next(),
        Some("--keep-alive-period (3000 ms) must be shorter than --keep-alive-timeout (3000 ms)")
    );
    assert_eq!(
        keyvo_core::config::keep_alive_problem(("p", ms(400)), ("t", ms(500))),
        None
    );
}

#[test]
fn durations_serialize_as_milliseconds_and_read_back() {
    let config = parse("valid/partial.toml").unwrap();
    let text = toml::to_string(&config).unwrap();
    assert!(text.contains("long_press = 800\n"), "{text}");
    assert!(text.contains("page_return_global = 10000\n"), "{text}");
    assert!(text.contains("page_return_own = 0\n"), "{text}");
    assert_eq!(
        Config::from_toml(&text, Path::new("config.toml")),
        Ok(config)
    );
}
