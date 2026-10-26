//! The context engine (spec §6): profile selection from a window context
//! and the mode and page state machine, on the profiles of
//! `tests/fixtures/dirs/focus/`.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

use keyvo_core::config::{self, Config, Timing};
use keyvo_core::focus::WindowContext;
use keyvo_core::layout::{self, Profiles};
use keyvo_core::profile::Profile;
use keyvo_core::state::{Mode, PageButton, State, View};

fn profiles() -> Profiles {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dirs/focus");
    layout::load(&dir).unwrap().profiles
}

fn window(class: &str, title: &str) -> WindowContext {
    WindowContext::new(class, title)
}

#[test]
fn the_profile_for_a_window_follows_the_match_rules_and_priority() {
    let profiles = profiles();
    let table = [
        // Reverse-DNS rule, reverse-DNS window.
        (window("org.kde.konsole", "~ : bash"), "konsole"),
        // A short window class matches a reverse-DNS rule.
        (window("konsole", "~ : bash"), "konsole"),
        // Class and title match a higher priority profile.
        (window("org.kde.konsole", "Claude Code"), "claude"),
        // The class of a title rule still has to match.
        (window("org.kde.dolphin", "claude"), "global"),
        // Class names are case-insensitive.
        (window("Code", "main.rs - keyvo"), "code"),
        // Another vendor's `konsole` is not the short form of the rule.
        (window("org.other.konsole", "~"), "global"),
        // A profile without classes matches on the title alone.
        (window("firefox", "Fix the pull request - GitHub"), "titled"),
        // Nothing matches: the global profile, whose empty rules never match.
        (window("org.kde.dolphin", "Home"), "global"),
        (window("", ""), "global"),
    ];
    for (window, expected) in table {
        assert_eq!(profiles.select(&window).profile.id, expected, "{window:?}");
    }
}

#[test]
fn between_equal_priorities_a_title_rule_wins_then_the_id_order() {
    let app = |id: &str, rules: &str| {
        let text = format!(
            "schema_version = 1\n[profile]\nid = \"{id}\"\nname = \"{id}\"\nmatch = {rules}\n"
        );
        let profile = Profile::from_toml(&text, Path::new("x.toml")).unwrap();
        (id.to_owned(), profile)
    };
    let profiles = Profiles::new(BTreeMap::from([
        app("a", "{ class = [\"code\"] }"),
        app("b", "{ class = [\"code\"], title = \"rs\" }"),
        app("c", "{ class = [\"code\"], title = \"rs\" }"),
    ]));
    assert_eq!(profiles.select(&window("code", "main.rs")).profile.id, "b");
    assert_eq!(profiles.select(&window("code", "a.md")).profile.id, "a");
}

/// One input to the state machine at a time in milliseconds after the
/// start.
#[derive(Debug, Clone, Copy)]
enum Step {
    /// The focus source reports a window.
    Focus(&'static str, &'static str, u64),
    /// A page button goes down.
    Down(PageButton, u64),
    /// A page button comes up.
    Up(PageButton, u64),
    /// The caller's timer: pending focus changes and long presses.
    Poll(u64),
}

use PageButton::{Left, Right};
use Step::{Down, Focus, Poll, Up};

const AUTO: Mode = Mode::Auto;
const MANUAL: Mode = Mode::Manual;

/// Runs `table` on a fresh state with the default `[timing]` (hysteresis
/// 150 ms, long press 1000 ms, page grace 10 s) and `[focus]` (`keyvo-app`
/// holds) and checks the profile, page and mode after every step.
fn run(profiles: &Profiles, table: &[(Step, &str, usize, Mode)]) {
    run_with(&Config::default(), profiles, table);
}

fn run_with(config: &Config, profiles: &Profiles, table: &[(Step, &str, usize, Mode)]) {
    let start = Instant::now();
    let at = |ms: u64| start + Duration::from_millis(ms);
    let mut state = State::new(config, profiles);
    assert_eq!(state.view(profiles), view("global", 1, AUTO));
    for (row, &(step, profile, page, mode)) in table.iter().enumerate() {
        match step {
            Focus(class, title, ms) => state.focus(window(class, title), at(ms), profiles),
            Down(button, ms) => state.press(button, at(ms), profiles),
            Up(button, ms) => state.release(button, at(ms), profiles),
            Poll(ms) => state.poll(at(ms), profiles),
        }
        assert_eq!(
            state.view(profiles),
            view(profile, page, mode),
            "row {row}: {step:?}"
        );
    }
}

fn view(profile: &str, page: usize, mode: Mode) -> View {
    View {
        profile: profile.to_owned(),
        page,
        mode,
    }
}

#[test]
fn a_focus_change_applies_after_the_hysteresis() {
    run(
        &profiles(),
        &[
            (Focus("org.kde.konsole", "~", 0), "global", 1, AUTO),
            (Poll(149), "global", 1, AUTO),
            (Poll(150), "konsole", 1, AUTO),
            // A flicker to another window and back never switches.
            (Focus("code", "main.rs", 200), "konsole", 1, AUTO),
            (Focus("org.kde.konsole", "~", 300), "konsole", 1, AUTO),
            (Poll(400), "konsole", 1, AUTO),
            // A newer report restarts the wait.
            (Focus("org.kde.dolphin", "Home", 500), "konsole", 1, AUTO),
            (Focus("code", "main.rs", 600), "konsole", 1, AUTO),
            (Poll(700), "konsole", 1, AUTO),
            // The context page is the page named `context`, here the second.
            (Poll(750), "code", 2, AUTO),
            // A title change in the same window selects again.
            (Focus("org.kde.konsole", "claude", 1000), "code", 2, AUTO),
            (Poll(1150), "claude", 1, AUTO),
        ],
    );
}

#[test]
fn in_auto_mode_a_page_button_toggles_context_and_general() {
    run(
        &profiles(),
        &[
            (Focus("org.kde.konsole", "~", 0), "global", 1, AUTO),
            (Poll(150), "konsole", 1, AUTO),
            // The press alone does nothing; the short release toggles.
            (Down(Left, 1000), "konsole", 1, AUTO),
            (Up(Left, 1100), "global", 2, AUTO),
            (Down(Right, 1200), "global", 2, AUTO),
            (Up(Right, 1300), "konsole", 1, AUTO),
            (Down(Right, 1400), "konsole", 1, AUTO),
            (Up(Right, 1500), "global", 2, AUTO),
        ],
    );
}

#[test]
fn an_app_switch_returns_to_the_context_page_after_the_page_grace() {
    run(
        &profiles(),
        &[
            (Down(Left, 0), "global", 1, AUTO),
            (Up(Left, 100), "global", 2, AUTO),
            // Within 10 s of the press the general page sticks.
            (Focus("code", "main.rs", 9000), "global", 2, AUTO),
            (Poll(9150), "global", 2, AUTO),
            // Later an app switch returns to the new app's context page.
            (Focus("org.kde.konsole", "~", 10_000), "global", 2, AUTO),
            (Poll(10_150), "konsole", 1, AUTO),
            (Down(Left, 20_000), "konsole", 1, AUTO),
            (Up(Left, 20_100), "global", 2, AUTO),
            // A title change in the same app is no app switch.
            (
                Focus("org.kde.konsole", "claude", 40_000),
                "global",
                2,
                AUTO,
            ),
            (Poll(40_150), "global", 2, AUTO),
            (Down(Left, 41_000), "global", 2, AUTO),
            (Up(Left, 41_100), "claude", 1, AUTO),
        ],
    );
}

#[test]
fn a_long_press_toggles_manual_mode_which_pages_through_the_profile() {
    run(
        &profiles(),
        &[
            (Focus("code", "main.rs", 0), "global", 1, AUTO),
            (Poll(150), "code", 2, AUTO),
            // The long press fires at the deadline, before the release.
            (Down(Left, 1000), "code", 2, AUTO),
            (Poll(1999), "code", 2, AUTO),
            (Poll(2000), "code", 2, MANUAL),
            (Up(Left, 2100), "code", 2, MANUAL),
            // Right pages forward, left back, both wrap around.
            (Down(Right, 3000), "code", 2, MANUAL),
            (Up(Right, 3100), "code", 3, MANUAL),
            (Down(Right, 3200), "code", 3, MANUAL),
            (Up(Right, 3300), "code", 1, MANUAL),
            (Down(Left, 3400), "code", 1, MANUAL),
            (Up(Left, 3500), "code", 3, MANUAL),
            // Focus changes are ignored in manual mode.
            (Focus("org.kde.konsole", "~", 4000), "code", 3, MANUAL),
            (Poll(4150), "code", 3, MANUAL),
            // Back in auto mode the focused window's context page shows. A
            // long press noticed only at the release counts as well.
            (Down(Right, 5000), "code", 3, MANUAL),
            (Up(Right, 6500), "konsole", 1, AUTO),
        ],
    );
}

#[test]
fn manual_mode_keeps_what_the_pad_shows() {
    run(
        &profiles(),
        &[
            (Down(Left, 0), "global", 1, AUTO),
            (Up(Left, 100), "global", 2, AUTO),
            (Down(Left, 200), "global", 2, AUTO),
            (Poll(1200), "global", 2, MANUAL),
            (Up(Left, 1300), "global", 2, MANUAL),
            (Down(Left, 1400), "global", 2, MANUAL),
            (Up(Left, 1500), "global", 1, MANUAL),
        ],
    );
}

#[test]
fn both_page_buttons_escape_to_the_global_context_page_in_auto_mode() {
    run(
        &profiles(),
        &[
            (Focus("code", "main.rs", 0), "global", 1, AUTO),
            (Poll(150), "code", 2, AUTO),
            (Down(Left, 1000), "code", 2, AUTO),
            (Poll(2000), "code", 2, MANUAL),
            (Down(Right, 2500), "global", 1, AUTO),
            // Neither release nor the hold that goes on does anything more.
            (Poll(5000), "global", 1, AUTO),
            (Up(Left, 5100), "global", 1, AUTO),
            (Up(Right, 5200), "global", 1, AUTO),
            // From the general page in auto mode, too, before any long press.
            (Down(Right, 6000), "global", 1, AUTO),
            (Up(Right, 6100), "global", 2, AUTO),
            (Down(Right, 6200), "global", 2, AUTO),
            (Down(Left, 6300), "global", 1, AUTO),
            (Up(Left, 6400), "global", 1, AUTO),
            (Up(Right, 6500), "global", 1, AUTO),
            // The next focus change selects again.
            (Focus("org.kde.konsole", "~", 20_000), "global", 1, AUTO),
            (Poll(20_150), "konsole", 1, AUTO),
        ],
    );
}

#[test]
fn without_a_general_page_the_page_button_does_nothing_in_auto_mode() {
    run(
        &Profiles::new(BTreeMap::new()),
        &[
            (Down(Left, 0), "global", 1, AUTO),
            (Up(Left, 100), "global", 1, AUTO),
            // A profile with one page stays on it in manual mode.
            (Down(Left, 200), "global", 1, AUTO),
            (Poll(1200), "global", 1, MANUAL),
            (Up(Left, 1300), "global", 1, MANUAL),
            (Down(Right, 1400), "global", 1, MANUAL),
            (Up(Right, 1500), "global", 1, MANUAL),
        ],
    );
}

#[test]
fn a_zero_hysteresis_switches_at_the_report() {
    let profiles = profiles();
    let config = Config {
        timing: Timing {
            focus_hysteresis: Duration::ZERO,
            ..Timing::default()
        },
        ..Config::default()
    };
    let mut state = State::new(&config, &profiles);
    let now = Instant::now();
    state.focus(window("code", "x"), now, &profiles);
    assert_eq!(state.view(&profiles), view("code", 2, AUTO));
    assert_eq!(state.next_deadline(), None);
}

#[test]
fn the_next_deadline_is_the_pending_focus_or_the_long_press() {
    let profiles = profiles();
    let mut state = State::new(&Config::default(), &profiles);
    let now = Instant::now();
    assert_eq!(state.next_deadline(), None);
    state.press(Left, now, &profiles);
    assert_eq!(state.next_deadline(), Some(now + Duration::from_secs(1)));
    state.focus(window("code", "x"), now, &profiles);
    assert_eq!(
        state.next_deadline(),
        Some(now + Duration::from_millis(150))
    );
    assert_eq!(state.mode(), AUTO);
    assert_eq!(Mode::Manual.as_str(), "manual");
}

#[test]
fn a_reload_selects_again_and_falls_back_when_profiles_are_gone() {
    let profiles = profiles();
    let start = Instant::now();
    let mut state = State::new(&Config::default(), &profiles);
    state.focus(window("code", "x"), start, &profiles);
    state.poll(start + Duration::from_secs(1), &profiles);
    assert_eq!(state.view(&profiles), view("code", 2, AUTO));

    // The rules changed: the same window selects another profile.
    let mut by_id: BTreeMap<String, Profile> = profiles
        .iter()
        .map(|p| (p.profile.id.clone(), p.clone()))
        .collect();
    by_id.remove("code");
    let fewer = Profiles::new(by_id);
    state.reload(&fewer);
    assert_eq!(state.view(&fewer), view("global", 1, AUTO));

    // Manual mode on the general page, then the general page is gone.
    state.press(Left, start, &fewer);
    state.release(Left, start, &fewer);
    assert_eq!(state.view(&fewer), view("global", 2, AUTO));
    state.press(Left, start, &fewer);
    state.poll(start + Duration::from_secs(2), &fewer);
    state.release(Left, start + Duration::from_secs(2), &fewer);
    assert_eq!(state.view(&fewer), view("global", 2, MANUAL));
    let builtin = Profiles::new(BTreeMap::new());
    state.reload(&builtin);
    assert_eq!(state.view(&builtin), view("global", 1, MANUAL));
    // Back in auto mode, the general page does not come back either.
    state.press(Left, start, &builtin);
    state.poll(start + Duration::from_secs(5), &builtin);
    assert_eq!(state.view(&builtin), view("global", 1, AUTO));
}

#[test]
fn a_held_window_keeps_the_profile_and_page_of_the_window_before() {
    run(
        &profiles(),
        &[
            (Focus("org.kde.konsole", "~", 0), "global", 1, AUTO),
            (Poll(150), "konsole", 1, AUTO),
            (Focus("keyvo-app", "keyvo", 1000), "konsole", 1, AUTO),
            (Poll(1150), "konsole", 1, AUTO),
            // The page buttons work as before while held.
            (Down(Left, 2000), "konsole", 1, AUTO),
            (Up(Left, 2100), "global", 2, AUTO),
            // Past the page grace a held report is still no app switch.
            (Focus("org.kde.konsole", "~", 13_000), "global", 2, AUTO),
            (Poll(13_150), "global", 2, AUTO),
            (Focus("keyvo-app", "keyvo", 30_000), "global", 2, AUTO),
            (Poll(30_150), "global", 2, AUTO),
            // Back to the window held over: nothing to apply.
            (Focus("org.kde.konsole", "~", 31_000), "global", 2, AUTO),
            (Poll(31_150), "global", 2, AUTO),
        ],
    );
}

#[test]
fn a_held_report_cancels_a_pending_one() {
    run(
        &profiles(),
        &[
            (Focus("org.kde.konsole", "~", 0), "global", 1, AUTO),
            (Poll(150), "konsole", 1, AUTO),
            (Focus("code", "main.rs", 1000), "konsole", 1, AUTO),
            (Focus("Keyvo-App", "keyvo", 1050), "konsole", 1, AUTO),
            (Poll(1150), "konsole", 1, AUTO),
            (Poll(5000), "konsole", 1, AUTO),
        ],
    );
}

#[test]
fn the_window_after_a_held_one_applies_after_the_hysteresis() {
    run(
        &profiles(),
        &[
            (Focus("org.kde.konsole", "~", 0), "global", 1, AUTO),
            (Poll(150), "konsole", 1, AUTO),
            (Focus("keyvo-app", "keyvo", 1000), "konsole", 1, AUTO),
            (Focus("code", "main.rs", 2000), "konsole", 1, AUTO),
            (Poll(2149), "konsole", 1, AUTO),
            (Poll(2150), "code", 2, AUTO),
        ],
    );
}

#[test]
fn a_held_window_before_any_other_keeps_the_global_profile() {
    run(
        &profiles(),
        &[
            (Focus("keyvo-app", "keyvo", 0), "global", 1, AUTO),
            (Poll(150), "global", 1, AUTO),
            (Poll(1000), "global", 1, AUTO),
        ],
    );
}

#[test]
fn without_hold_classes_the_keyvo_app_switches_like_any_window() {
    let config = Config {
        focus: config::Focus {
            hold_classes: Vec::new(),
        },
        ..Config::default()
    };
    run_with(
        &config,
        &profiles(),
        &[
            (Focus("org.kde.konsole", "~", 0), "global", 1, AUTO),
            (Poll(150), "konsole", 1, AUTO),
            (Focus("keyvo-app", "keyvo", 1000), "konsole", 1, AUTO),
            (Poll(1150), "global", 1, AUTO),
        ],
    );
}

#[test]
fn the_state_tells_the_held_window_until_another_reports() {
    let profiles = profiles();
    let now = Instant::now();
    let mut state = State::new(&Config::default(), &profiles);
    assert_eq!(state.held(), None);
    state.focus(window("org.kde.konsole", "~"), now, &profiles);
    state.focus(window("keyvo-app", "keyvo"), now, &profiles);
    assert_eq!(state.held(), Some(&window("keyvo-app", "keyvo")));
    assert_eq!(state.next_deadline(), None);
    assert!(state.holds("KEYVO-APP"));
    state.focus(window("code", "x"), now, &profiles);
    assert_eq!(state.held(), None);
}

#[test]
fn a_reload_while_held_keeps_the_profile() {
    let profiles = profiles();
    let start = Instant::now();
    let mut state = State::new(&Config::default(), &profiles);
    state.focus(window("org.kde.konsole", "~"), start, &profiles);
    state.poll(start + Duration::from_secs(1), &profiles);
    state.focus(
        window("keyvo-app", "keyvo"),
        start + Duration::from_secs(2),
        &profiles,
    );
    state.reload(&profiles);
    assert_eq!(state.view(&profiles), view("konsole", 1, AUTO));
    assert_eq!(state.held(), Some(&window("keyvo-app", "keyvo")));
    state.poll(start + Duration::from_secs(3), &profiles);
    assert_eq!(state.view(&profiles), view("konsole", 1, AUTO));
}
