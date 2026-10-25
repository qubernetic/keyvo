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
