//! `config.toml`: the fixtures under `tests/fixtures/config/`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use keyvo_core::config::{Config, Render, Timing};
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
            page_grace: ms(10_000),
            keep_alive_period: ms(1000),
            keep_alive_timeout: ms(3000),
            ack_timeout: ms(50),
            divert_settle: ms(100),
            divert_spacing: ms(20),
            reply_timeout: ms(1000),
            exec_timeout: ms(30_000),
            reconnect_interval: ms(1000),
            reload_settle: ms(100),
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
fn durations_serialize_as_milliseconds_and_read_back() {
    let config = parse("valid/partial.toml").unwrap();
    let text = toml::to_string(&config).unwrap();
    assert!(text.contains("long_press = 800\n"), "{text}");
    assert!(text.contains("page_grace = 10000\n"), "{text}");
    assert_eq!(
        Config::from_toml(&text, Path::new("config.toml")),
        Ok(config)
    );
}
