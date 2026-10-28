//! Press kinds with a fake clock, and the resolution of a press to a trace
//! record (spec §6.1, §7.2, §8).

use std::time::Duration;

use keyvo_core::clock::{Clock, FakeClock};
use keyvo_core::press::{PressDetector, PressKind};

const LONG: Duration = Duration::from_millis(1000);

#[test]
fn a_release_before_the_long_press_time_is_short() {
    let clock = FakeClock::new();
    let mut presses = PressDetector::new(LONG);
    presses.press(3, clock.now());
    clock.advance(Duration::from_millis(999));
    assert_eq!(presses.poll(clock.now()), Vec::<u8>::new());
    assert_eq!(presses.release(3, clock.now()), Some(PressKind::Short));
    // The key is no longer held.
    assert_eq!(presses.release(3, clock.now()), None);
}

#[test]
fn holding_for_the_long_press_time_fires_long_once_without_waiting_for_the_release() {
    let clock = FakeClock::new();
    let mut presses = PressDetector::new(LONG);
    presses.press(5, clock.now());
    assert_eq!(presses.next_deadline(), Some(clock.now() + LONG));
    clock.advance(LONG);
    assert_eq!(presses.poll(clock.now()), vec![5]);
    assert_eq!(presses.next_deadline(), None);
    clock.advance(LONG);
    assert_eq!(presses.poll(clock.now()), Vec::<u8>::new());
    // The long press has fired; the release adds nothing.
    assert_eq!(presses.release(5, clock.now()), None);
}

#[test]
fn a_late_release_without_a_poll_is_long() {
    let clock = FakeClock::new();
    let mut presses = PressDetector::new(LONG);
    presses.press(1, clock.now());
    clock.advance(Duration::from_millis(1500));
    assert_eq!(presses.release(1, clock.now()), Some(PressKind::Long));
}

#[test]
fn keys_are_timed_independently_and_a_repeated_press_keeps_the_first_time() {
    let clock = FakeClock::new();
    let mut presses = PressDetector::new(LONG);
    presses.press(1, clock.now());
    clock.advance(Duration::from_millis(600));
    presses.press(2, clock.now());
    // A second press report while held does not restart the timer.
    presses.press(1, clock.now());
    assert_eq!(
        presses.next_deadline(),
        Some(clock.now() + Duration::from_millis(400))
    );
    clock.advance(Duration::from_millis(400));
    assert_eq!(presses.poll(clock.now()), vec![1]);
    assert_eq!(presses.release(2, clock.now()), Some(PressKind::Short));
    clock.advance(Duration::from_millis(600));
    assert_eq!(presses.poll(clock.now()), Vec::<u8>::new());
}

#[test]
fn press_kinds_have_their_spec_names() {
    assert_eq!(PressKind::Short.as_str(), "short");
    assert_eq!(PressKind::Long.as_str(), "long");
    assert_eq!(serde_json::to_string(&PressKind::Long).unwrap(), "\"long\"");
}

mod resolution {
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    use keyvo_core::chord::Chord;
    use keyvo_core::press::{self, PressKind};
    use keyvo_core::profile::{Action, KeySlot, Profile};
    use keyvo_core::stack::{Layer, Stack};
    use keyvo_core::trace::{Branch, Outcome, TraceRecord, Trigger};

    fn demo() -> Profile {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/profiles/valid/demo.toml");
        Profile::from_toml(&std::fs::read_to_string(&path).unwrap(), &path).unwrap()
    }

    /// A press of `key` on page `page` of the demo profile alone.
    fn resolve(page: usize, key: u8, trigger: Trigger) -> TraceRecord {
        let profile = demo();
        let stack = Stack::alone(&profile, page).unwrap();
        press::resolve(&stack, slot(key), trigger, at())
    }

    fn slot(n: u8) -> KeySlot {
        KeySlot::new(n).unwrap()
    }

    /// 2026-10-25 12:00:00.250 UTC.
    fn at() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_millis(1_792_929_600_250)
    }

    const REAL_SHORT: Trigger = Trigger {
        press: PressKind::Short,
        simulated: false,
    };

    #[test]
    fn a_press_resolves_to_the_key_action_and_waits_for_its_result() {
        let record = resolve(1, 2, REAL_SHORT);
        assert_eq!(
            record,
            TraceRecord {
                timestamp_ms: 1_792_929_600_250,
                trigger: REAL_SHORT,
                profile: "demo".to_owned(),
                page: 1,
                key: slot(2),
                layer: Some(Layer::App),
                action: Some(Action::Chord {
                    keys: Chord::parse("ctrl+shift+g").unwrap()
                }),
                result: Outcome::Pending,
                branch: None,
            }
        );
    }

    #[test]
    fn an_on_fail_run_is_a_record_of_the_same_press_on_its_branch() {
        let mut primary = resolve(1, 1, REAL_SHORT);
        primary.finish(Outcome::Exited {
            code: Some(3),
            duration_ms: 12,
        });
        let notify = Action::Exec {
            cmd: "notify-send failed".into(),
        };
        let record = primary.on_fail(notify.clone());
        assert_eq!(
            record,
            TraceRecord {
                action: Some(notify),
                result: Outcome::Pending,
                branch: Some(Branch::OnFail),
                ..primary.clone()
            }
        );
        assert_eq!(primary.branch, None);
        assert_eq!(
            serde_json::to_value(Branch::OnFail).unwrap(),
            serde_json::json!("on_fail")
        );
    }

    #[test]
    fn a_long_press_runs_the_key_action_too() {
        let trigger = Trigger {
            press: PressKind::Long,
            simulated: false,
        };
        let record = resolve(1, 1, trigger);
        assert_eq!(
            record.action,
            Some(Action::Exec {
                cmd: "cargo build".to_owned()
            })
        );
        assert_eq!(record.trigger.press, PressKind::Long);
        assert_eq!(record.result, Outcome::Pending);
    }

    #[test]
    fn a_simulated_press_resolves_but_executes_nothing() {
        let trigger = Trigger {
            press: PressKind::Short,
            simulated: true,
        };
        let record = resolve(1, 5, trigger);
        assert!(record.action.is_some());
        assert_eq!(record.result, Outcome::DryRun);
    }

    #[test]
    fn a_blocked_press_resolves_nothing_and_says_why() {
        for simulated in [false, true] {
            let trigger = Trigger {
                press: PressKind::Short,
                simulated,
            };
            let record = keyvo_core::press::blocked("demo", 1, slot(1), trigger, at(), "locked");
            assert_eq!(
                record,
                TraceRecord {
                    timestamp_ms: 1_792_929_600_250,
                    trigger,
                    profile: "demo".to_owned(),
                    page: 1,
                    key: slot(1),
                    layer: None,
                    action: None,
                    result: Outcome::Blocked {
                        reason: "locked".to_owned()
                    },
                    branch: None,
                }
            );
        }
    }

    #[test]
    fn a_key_without_an_action_is_unbound() {
        for (page, key) in [(1, 7), (1, 8), (2, 5)] {
            let record = resolve(page, key, REAL_SHORT);
            assert_eq!(record.action, None, "page {page} key {key}");
            assert_eq!(record.result, Outcome::Unbound, "page {page} key {key}");
        }
        // A page the profile does not have has no stack to press on.
        assert!(Stack::alone(&demo(), 3).is_err());
        assert!(Stack::alone(&demo(), 0).is_err());
    }

    #[test]
    fn outcomes_tell_an_error_from_a_success() {
        let table = [
            (Outcome::Pending, false, false),
            (Outcome::DryRun, false, false),
            (Outcome::Unbound, false, false),
            (Outcome::Sent { events: vec![] }, true, false),
            (
                Outcome::Exited {
                    code: Some(0),
                    duration_ms: 5,
                },
                true,
                false,
            ),
            (
                Outcome::Exited {
                    code: Some(1),
                    duration_ms: 5,
                },
                false,
                true,
            ),
            (
                Outcome::Exited {
                    code: None,
                    duration_ms: 5,
                },
                false,
                true,
            ),
            (
                Outcome::Failed {
                    error: "timed out".to_owned(),
                },
                false,
                true,
            ),
            // A `launch` whose application still runs after the settle.
            (Outcome::Started { pid: 4242 }, true, false),
            // A press under the session lock ran nothing: no look.
            (
                Outcome::Blocked {
                    reason: "session locked".to_owned(),
                },
                false,
                false,
            ),
        ];
        for (outcome, success, error) in table {
            assert_eq!(outcome.is_success(), success, "{outcome:?}");
            assert_eq!(outcome.is_error(), error, "{outcome:?}");
        }
    }

    #[test]
    fn records_serialize_as_the_fixture_lines() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/trace/records.ndjson");
        let expected = std::fs::read_to_string(&path).unwrap();
        let mut chord = resolve(1, 4, REAL_SHORT);
        let events = Chord::parse("ctrl+p").unwrap().events();
        chord.finish(Outcome::Sent { events });
        let mut exec = resolve(1, 1, REAL_SHORT);
        exec.finish(Outcome::Exited {
            code: Some(101),
            duration_ms: 2318,
        });
        let mut failed = resolve(1, 9, REAL_SHORT);
        failed.finish(Outcome::Failed {
            error: "no such file or directory: gh".to_owned(),
        });
        let dry = resolve(
            1,
            5,
            Trigger {
                press: PressKind::Long,
                simulated: true,
            },
        );
        let unbound = resolve(2, 1, REAL_SHORT);
        let blocked =
            keyvo_core::press::blocked("demo", 1, slot(5), REAL_SHORT, at(), "session locked");
        let mut on_fail = exec.on_fail(Action::Exec {
            cmd: "notify-send 'build failed'".to_owned(),
        });
        on_fail.finish(Outcome::Exited {
            code: Some(0),
            duration_ms: 41,
        });
        let lines: String = [chord, exec, on_fail, failed, dry, unbound, blocked]
            .iter()
            .map(|record| serde_json::to_string(record).unwrap() + "\n")
            .collect();
        assert_eq!(lines, expected);
    }
}
