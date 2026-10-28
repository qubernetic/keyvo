//! The automatic key states (spec §7.2, §9) with a fake clock.

use std::time::Duration;

use keyvo_core::chord::Chord;
use keyvo_core::clock::{Clock, FakeClock};
use keyvo_core::config::Render;
use keyvo_core::feedback::{Feedback, Look};
use keyvo_core::profile::Action;
use keyvo_core::trace::Outcome;

fn render() -> Render {
    Render {
        feedback_hold: Duration::from_millis(2000),
        long_run_threshold: Duration::from_millis(10_000),
        ..Render::default()
    }
}

fn exited(code: i32) -> Outcome {
    Outcome::Exited {
        code: Some(code),
        duration_ms: 1,
    }
}

#[test]
fn a_key_starts_idle() {
    let clock = FakeClock::new();
    let feedback = Feedback::new(&render());
    assert_eq!(feedback.look(clock.now()), Look::Idle);
    assert_eq!(feedback.next_change(clock.now()), None);
}

#[test]
fn running_turns_into_the_long_run_look_after_the_threshold() {
    let clock = FakeClock::new();
    let mut feedback = Feedback::new(&render());
    feedback.started(clock.now());
    assert_eq!(feedback.look(clock.now()), Look::Running);
    assert_eq!(
        feedback.next_change(clock.now()),
        Some(clock.now() + Duration::from_millis(10_000))
    );
    clock.advance(Duration::from_millis(9_999));
    assert_eq!(feedback.look(clock.now()), Look::Running);
    clock.advance(Duration::from_millis(1));
    assert_eq!(feedback.look(clock.now()), Look::RunningLong);
    assert_eq!(feedback.next_change(clock.now()), None);
}

#[test]
fn success_holds_for_feedback_hold_then_reverts() {
    let clock = FakeClock::new();
    let mut feedback = Feedback::new(&render());
    feedback.started(clock.now());
    clock.advance(Duration::from_millis(300));
    feedback.finished(&exited(0), clock.now());
    assert_eq!(feedback.look(clock.now()), Look::Success);
    assert_eq!(
        feedback.next_change(clock.now()),
        Some(clock.now() + Duration::from_millis(2000))
    );
    clock.advance(Duration::from_millis(1999));
    assert_eq!(feedback.look(clock.now()), Look::Success);
    clock.advance(Duration::from_millis(1));
    assert_eq!(feedback.look(clock.now()), Look::Idle);
}

#[test]
fn error_stays_until_the_next_press() {
    let clock = FakeClock::new();
    let mut feedback = Feedback::new(&render());
    feedback.started(clock.now());
    feedback.finished(&exited(1), clock.now());
    assert_eq!(feedback.look(clock.now()), Look::Error);
    assert_eq!(feedback.next_change(clock.now()), None);
    clock.advance(Duration::from_secs(3600));
    assert_eq!(feedback.look(clock.now()), Look::Error);
    feedback.pressed();
    assert_eq!(feedback.look(clock.now()), Look::Idle);
}

#[test]
fn inspecting_the_trace_entry_clears_an_error() {
    let clock = FakeClock::new();
    let mut feedback = Feedback::new(&render());
    feedback.finished(
        &Outcome::Failed {
            error: "timed out".to_owned(),
        },
        clock.now(),
    );
    assert_eq!(feedback.look(clock.now()), Look::Error);
    feedback.inspected();
    assert_eq!(feedback.look(clock.now()), Look::Idle);
}

#[test]
fn a_press_while_running_keeps_running_and_clears_success() {
    let clock = FakeClock::new();
    let mut feedback = Feedback::new(&render());
    feedback.started(clock.now());
    feedback.pressed();
    assert_eq!(feedback.look(clock.now()), Look::Running);
    feedback.finished(&exited(0), clock.now());
    feedback.pressed();
    assert_eq!(feedback.look(clock.now()), Look::Idle);
}

#[test]
fn a_result_that_is_neither_success_nor_error_returns_to_idle() {
    let clock = FakeClock::new();
    let mut feedback = Feedback::new(&render());
    feedback.started(clock.now());
    feedback.finished(&Outcome::DryRun, clock.now());
    assert_eq!(feedback.look(clock.now()), Look::Idle);
}

#[test]
fn looks_name_the_key_state_that_overrides_them() {
    assert_eq!(Look::Idle.state(), None);
    assert_eq!(Look::Running.state(), Some("running"));
    assert_eq!(Look::RunningLong.state(), Some("running"));
    assert_eq!(Look::Success.state(), Some("success"));
    assert_eq!(Look::Error.state(), Some("error"));
}

#[test]
fn only_exec_and_launch_actions_have_automatic_states() {
    let exec = Action::Exec {
        cmd: "make".to_owned(),
    };
    let launch = Action::launch("kcalc");
    let chord = Action::Chord {
        keys: Chord::parse("ctrl+s").unwrap(),
    };
    assert!(exec.has_feedback());
    assert!(launch.has_feedback());
    assert!(!chord.has_feedback());
}
