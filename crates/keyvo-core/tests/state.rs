//! The context engine (spec §6): profile selection from a window context
//! and the mode and page state machine, on the profiles of
//! `tests/fixtures/dirs/focus/` and the example profiles of
//! `contrib/profiles/` (the shapes the owner has: an app profile with one
//! page, one with two, a global profile with three).

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

/// `contrib/profiles/`: `global` with the pages `context`, `general` and
/// `third`, `vscode` with `context` and a second page, `konsole` with
/// `context` alone.
fn contrib() -> Profiles {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contrib");
    layout::load(&dir).unwrap().profiles
}

/// A profile `id` with one page per name, matching the class `id`.
fn shaped(id: &str, pages: &[&str]) -> (String, Profile) {
    let rules = if id == "global" {
        String::new()
    } else {
        format!("match = {{ class = [\"{id}\"] }}\n")
    };
    let pages = pages
        .iter()
        .map(|name| format!("[[pages]]\nname = \"{name}\"\n"))
        .collect::<Vec<_>>()
        .concat();
    let text =
        format!("schema_version = 1\n[profile]\nid = \"{id}\"\nname = \"{id}\"\n{rules}{pages}");
    let profile = Profile::from_toml(&text, Path::new("x.toml")).unwrap();
    (id.to_owned(), profile)
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
    /// A short press: down at the time, up 100 ms later.
    Short(PageButton, u64),
    /// A key on the pad goes down.
    Key(u64),
    /// The caller's timer: pending focus changes, long presses and the
    /// page return.
    Poll(u64),
}

use PageButton::{Left, Right};
use Step::{Down, Focus, Key, Poll, Short, Up};

const AUTO: Mode = Mode::Auto;
const MANUAL: Mode = Mode::Manual;

/// After a step: the profile and page the pad shows, the list index and the
/// mode.
type Row = (Step, &'static str, usize, isize, Mode);

/// Runs `table` on a fresh state with the default `[timing]` (hysteresis
/// 150 ms, long press 1000 ms, global page return 10 s, own page return
/// off) and `[focus]` (`keyvo-app`
/// holds) and checks the view after every step.
fn run(profiles: &Profiles, table: &[Row]) {
    run_with(&Config::default(), profiles, table);
}

fn run_with(config: &Config, profiles: &Profiles, table: &[Row]) {
    let start = Instant::now();
    let at = |ms: u64| start + Duration::from_millis(ms);
    let mut state = State::new(config, profiles);
    assert_eq!(state.view(profiles), view("global", 1, 1, AUTO));
    for (row, &(step, profile, page, index, mode)) in table.iter().enumerate() {
        match step {
            Focus(class, title, ms) => state.focus(window(class, title), at(ms), profiles),
            Down(button, ms) => state.press(button, at(ms), profiles),
            Up(button, ms) => state.release(button, at(ms), profiles),
            Short(button, ms) => {
                state.press(button, at(ms), profiles);
                state.release(button, at(ms + 100), profiles);
            }
            Key(ms) => state.touch(at(ms)),
            Poll(ms) => state.poll(at(ms), profiles),
        }
        assert_eq!(
            state.view(profiles),
            view(profile, page, index, mode),
            "row {row}: {step:?}"
        );
    }
}

fn view(profile: &str, page: usize, index: isize, mode: Mode) -> View {
    View {
        profile: profile.to_owned(),
        page,
        mode,
        index,
    }
}

#[test]
fn a_focus_change_applies_after_the_hysteresis() {
    run(
        &profiles(),
        &[
            (Focus("org.kde.konsole", "~", 0), "global", 1, 1, AUTO),
            (Poll(149), "global", 1, 1, AUTO),
            (Poll(150), "konsole", 1, 1, AUTO),
            // A flicker to another window and back never switches.
            (Focus("code", "main.rs", 200), "konsole", 1, 1, AUTO),
            (Focus("org.kde.konsole", "~", 300), "konsole", 1, 1, AUTO),
            (Poll(400), "konsole", 1, 1, AUTO),
            // A newer report restarts the wait.
            (Focus("org.kde.dolphin", "Home", 500), "konsole", 1, 1, AUTO),
            (Focus("code", "main.rs", 600), "konsole", 1, 1, AUTO),
            (Poll(700), "konsole", 1, 1, AUTO),
            // Index 1 is the page named `context`, here the second.
            (Poll(750), "code", 2, 1, AUTO),
            // A title change in the same window selects again.
            (Focus("org.kde.konsole", "claude", 1000), "code", 2, 1, AUTO),
            (Poll(1150), "claude", 1, 1, AUTO),
        ],
    );
}

#[test]
fn the_page_buttons_walk_the_list_of_the_shown_profile() {
    // `code` has the pages edit, context, debug: its list is
    // [0 global general, 1 context, 2 edit, 3 debug].
    run(
        &profiles(),
        &[
            (Focus("code", "main.rs", 0), "global", 1, 1, AUTO),
            (Poll(150), "code", 2, 1, AUTO),
            // The press alone does nothing; the short release pages.
            (Down(Right, 1000), "code", 2, 1, AUTO),
            (Up(Right, 1100), "code", 1, 2, AUTO),
            (Short(Right, 1200), "code", 3, 3, AUTO),
            // Past the right end: nothing.
            (Short(Right, 1400), "code", 3, 3, AUTO),
            (Short(Left, 1600), "code", 1, 2, AUTO),
            (Short(Left, 1800), "code", 2, 1, AUTO),
            (Short(Left, 2000), "global", 2, 0, AUTO),
            // The global profile has no page after `general`: the left end.
            (Short(Left, 2200), "global", 2, 0, AUTO),
            (Short(Right, 2400), "code", 2, 1, AUTO),
        ],
    );
}

#[test]
fn from_the_vscode_context_page_right_shows_page_two_and_left_walks_the_global_pages() {
    run(
        &contrib(),
        &[
            (Focus("code", "main.rs", 0), "global", 1, 1, AUTO),
            (Poll(150), "vscode", 1, 1, AUTO),
            (Short(Right, 1000), "vscode", 2, 2, AUTO),
            (Short(Right, 1200), "vscode", 2, 2, AUTO),
            (Short(Left, 1400), "vscode", 1, 1, AUTO),
            (Short(Left, 1600), "global", 2, 0, AUTO),
            (Short(Left, 1800), "global", 3, -1, AUTO),
            (Short(Left, 2000), "global", 3, -1, AUTO),
        ],
    );
}

#[test]
fn on_konsole_with_one_page_right_does_nothing_and_left_walks_the_global_pages() {
    run(
        &contrib(),
        &[
            (Focus("org.kde.konsole", "~", 0), "global", 1, 1, AUTO),
            (Poll(150), "konsole", 1, 1, AUTO),
            (Short(Right, 1000), "konsole", 1, 1, AUTO),
            (Short(Left, 1200), "global", 2, 0, AUTO),
            (Short(Left, 1400), "global", 3, -1, AUTO),
            (Short(Left, 1600), "global", 3, -1, AUTO),
        ],
    );
}

#[test]
fn the_global_profile_on_an_unmatched_window_walks_its_three_pages() {
    run(
        &contrib(),
        &[
            (Focus("org.kde.dolphin", "Home", 0), "global", 1, 1, AUTO),
            (Poll(150), "global", 1, 1, AUTO),
            (Short(Right, 1000), "global", 1, 1, AUTO),
            (Short(Left, 1200), "global", 2, 0, AUTO),
            (Short(Left, 1400), "global", 3, -1, AUTO),
            (Short(Left, 1600), "global", 3, -1, AUTO),
        ],
    );
}

#[test]
fn walking_left_from_general_follows_the_global_pages_in_file_order() {
    // The global pages after `general` in file order take -1, -2, ...; a
    // global page before `general` other than `context` is the global
    // profile's own index 2; an app page named `general` is an ordinary page.
    let profiles = Profiles::new(BTreeMap::from([
        shaped("global", &["context", "notes", "general", "apps", "media"]),
        shaped("app", &["general", "context"]),
    ]));
    run(
        &profiles,
        &[
            (Short(Right, 0), "global", 2, 2, AUTO),
            (Short(Right, 200), "global", 2, 2, AUTO),
            (Short(Left, 400), "global", 1, 1, AUTO),
            (Short(Left, 600), "global", 3, 0, AUTO),
            (Short(Left, 800), "global", 4, -1, AUTO),
            (Short(Left, 1000), "global", 5, -2, AUTO),
            (Short(Left, 1200), "global", 5, -2, AUTO),
            (Focus("app", "x", 2000), "global", 5, -2, AUTO),
            (Poll(2150), "app", 2, 1, AUTO),
            (Short(Right, 3000), "app", 1, 2, AUTO),
            (Short(Right, 3200), "app", 1, 2, AUTO),
        ],
    );
}

#[test]
fn an_app_switch_shows_index_one_of_the_new_profile_at_once() {
    run(
        &contrib(),
        &[
            (Focus("org.kde.konsole", "~", 0), "global", 1, 1, AUTO),
            (Poll(150), "konsole", 1, 1, AUTO),
            (Short(Left, 1000), "global", 2, 0, AUTO),
            // Right after the press: no grace.
            (Focus("code", "main.rs", 2000), "global", 2, 0, AUTO),
            (Poll(2150), "vscode", 1, 1, AUTO),
            (Short(Right, 3000), "vscode", 2, 2, AUTO),
            (Focus("org.kde.konsole", "~", 3500), "vscode", 2, 2, AUTO),
            (Poll(3650), "konsole", 1, 1, AUTO),
            (Short(Left, 4000), "global", 2, 0, AUTO),
            (Short(Left, 4200), "global", 3, -1, AUTO),
            // A title change in the same app keeps the index and the clock.
            (Focus("org.kde.konsole", "vim", 5000), "global", 3, -1, AUTO),
            (Poll(5150), "global", 3, -1, AUTO),
            (Poll(14_299), "global", 3, -1, AUTO),
            (Poll(14_300), "konsole", 1, 1, AUTO),
        ],
    );
}

#[test]
fn a_global_page_returns_to_the_context_page_after_the_global_return_time() {
    run(
        &contrib(),
        &[
            (Focus("code", "main.rs", 0), "global", 1, 1, AUTO),
            (Poll(150), "vscode", 1, 1, AUTO),
            // The clock starts at the release, 1100.
            (Short(Left, 1000), "global", 2, 0, AUTO),
            (Poll(11_099), "global", 2, 0, AUTO),
            (Poll(11_100), "vscode", 1, 1, AUTO),
            (Short(Left, 12_000), "global", 2, 0, AUTO),
            (Short(Left, 12_200), "global", 3, -1, AUTO),
            // A key press restarts it.
            (Key(20_000), "global", 3, -1, AUTO),
            (Poll(22_300), "global", 3, -1, AUTO),
            (Key(29_000), "global", 3, -1, AUTO),
            (Poll(38_999), "global", 3, -1, AUTO),
            (Poll(39_000), "vscode", 1, 1, AUTO),
            // On the context page a key press starts nothing.
            (Key(40_000), "vscode", 1, 1, AUTO),
            (Poll(60_000), "vscode", 1, 1, AUTO),
        ],
    );
}

#[test]
fn the_shown_profiles_own_pages_stay_by_default() {
    run(
        &contrib(),
        &[
            (Focus("code", "main.rs", 0), "global", 1, 1, AUTO),
            (Poll(150), "vscode", 1, 1, AUTO),
            (Short(Right, 1000), "vscode", 2, 2, AUTO),
            (Poll(600_000), "vscode", 2, 2, AUTO),
        ],
    );
}

fn timing(global: u64, own: u64) -> Config {
    Config {
        timing: Timing {
            page_return_global: Duration::from_millis(global),
            page_return_own: Duration::from_millis(own),
            ..Timing::default()
        },
        ..Config::default()
    }
}

#[test]
fn each_side_returns_after_its_own_time_and_zero_keeps_the_page() {
    run_with(
        &timing(0, 5000),
        &contrib(),
        &[
            (Focus("code", "main.rs", 0), "global", 1, 1, AUTO),
            (Poll(150), "vscode", 1, 1, AUTO),
            (Short(Right, 1000), "vscode", 2, 2, AUTO),
            (Key(3000), "vscode", 2, 2, AUTO),
            (Poll(7999), "vscode", 2, 2, AUTO),
            (Poll(8000), "vscode", 1, 1, AUTO),
            (Short(Left, 9000), "global", 2, 0, AUTO),
            (Poll(600_000), "global", 2, 0, AUTO),
        ],
    );
}

#[test]
fn the_lock_keeps_the_page_past_the_return_time() {
    run(
        &contrib(),
        &[
            (Focus("code", "main.rs", 0), "global", 1, 1, AUTO),
            (Poll(150), "vscode", 1, 1, AUTO),
            (Short(Left, 1000), "global", 2, 0, AUTO),
            (Down(Left, 2000), "global", 2, 0, AUTO),
            (Poll(3000), "global", 2, 0, MANUAL),
            (Up(Left, 3100), "global", 2, 0, MANUAL),
            (Poll(30_000), "global", 2, 0, MANUAL),
            (Down(Left, 31_000), "global", 2, 0, MANUAL),
            (Poll(32_000), "vscode", 1, 1, AUTO),
            (Up(Left, 32_100), "vscode", 1, 1, AUTO),
        ],
    );
}

#[test]
fn the_next_deadline_includes_the_return() {
    let profiles = contrib();
    let start = Instant::now();
    let at = |ms: u64| start + Duration::from_millis(ms);
    let mut state = State::new(&timing(10_000, 5000), &profiles);
    state.focus(window("code", "main.rs"), start, &profiles);
    state.poll(at(150), &profiles);
    assert_eq!(state.next_deadline(), None);
    state.press(Left, at(1000), &profiles);
    state.release(Left, at(1100), &profiles);
    assert_eq!(state.next_deadline(), Some(at(11_100)));
    state.touch(at(2000));
    assert_eq!(state.next_deadline(), Some(at(12_000)));
    state.press(Right, at(3000), &profiles);
    state.release(Right, at(3100), &profiles);
    state.press(Right, at(4000), &profiles);
    state.release(Right, at(4100), &profiles);
    assert_eq!(state.view(&profiles), view("vscode", 2, 2, AUTO));
    assert_eq!(state.next_deadline(), Some(at(9100)));
    // A reload reads the new times.
    state.reload(&timing(10_000, 0), &profiles);
    assert_eq!(state.next_deadline(), None);
    state.poll(at(60_000), &profiles);
    assert_eq!(state.view(&profiles), view("vscode", 2, 2, AUTO));
}

#[test]
fn a_long_press_locks_the_shown_profile_and_index() {
    run(
        &contrib(),
        &[
            (Focus("code", "main.rs", 0), "global", 1, 1, AUTO),
            (Poll(150), "vscode", 1, 1, AUTO),
            (Short(Right, 1000), "vscode", 2, 2, AUTO),
            // The long press fires at the deadline, before the release.
            (Down(Left, 2000), "vscode", 2, 2, AUTO),
            (Poll(2999), "vscode", 2, 2, AUTO),
            (Poll(3000), "vscode", 2, 2, MANUAL),
            (Up(Left, 3100), "vscode", 2, 2, MANUAL),
            // Focus changes are recorded, not shown.
            (Focus("org.kde.konsole", "~", 4000), "vscode", 2, 2, MANUAL),
            (Poll(4150), "vscode", 2, 2, MANUAL),
            // The buttons walk the same list, without wrapping.
            (Short(Left, 5000), "vscode", 1, 1, MANUAL),
            (Short(Left, 5200), "global", 2, 0, MANUAL),
            (Short(Left, 5400), "global", 3, -1, MANUAL),
            (Short(Left, 5600), "global", 3, -1, MANUAL),
            // Leaving the lock shows index 1 of the focused window's profile.
            // A long press noticed only at the release counts as well.
            (Down(Right, 6000), "global", 3, -1, MANUAL),
            (Up(Right, 7500), "konsole", 1, 1, AUTO),
        ],
    );
}

#[test]
fn both_page_buttons_escape_to_the_global_context_page_in_auto_mode() {
    run(
        &contrib(),
        &[
            (Focus("code", "main.rs", 0), "global", 1, 1, AUTO),
            (Poll(150), "vscode", 1, 1, AUTO),
            (Short(Right, 500), "vscode", 2, 2, AUTO),
            (Down(Left, 1000), "vscode", 2, 2, AUTO),
            (Poll(2000), "vscode", 2, 2, MANUAL),
            (Down(Right, 2500), "global", 1, 1, AUTO),
            // Neither release nor the hold that goes on does anything more.
            (Poll(5000), "global", 1, 1, AUTO),
            (Up(Left, 5100), "global", 1, 1, AUTO),
            (Up(Right, 5200), "global", 1, 1, AUTO),
            // From a global page in auto mode, too, before any long press.
            (Short(Left, 6000), "global", 2, 0, AUTO),
            (Down(Left, 6200), "global", 2, 0, AUTO),
            (Down(Right, 6300), "global", 1, 1, AUTO),
            (Up(Left, 6400), "global", 1, 1, AUTO),
            (Up(Right, 6500), "global", 1, 1, AUTO),
            // The next focus change selects again.
            (Focus("org.kde.konsole", "~", 20_000), "global", 1, 1, AUTO),
            (Poll(20_150), "konsole", 1, 1, AUTO),
        ],
    );
}

#[test]
fn without_a_general_page_the_list_starts_at_the_context_page() {
    run(
        &Profiles::new(BTreeMap::new()),
        &[
            (Short(Left, 0), "global", 1, 1, AUTO),
            (Short(Right, 200), "global", 1, 1, AUTO),
            // The lock on a one-page list stays on it.
            (Down(Left, 400), "global", 1, 1, AUTO),
            (Poll(1400), "global", 1, 1, MANUAL),
            (Up(Left, 1500), "global", 1, 1, MANUAL),
            (Short(Right, 1600), "global", 1, 1, MANUAL),
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
    assert_eq!(state.view(&profiles), view("code", 2, 1, AUTO));
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
    assert_eq!(state.view(&profiles), view("code", 2, 1, AUTO));

    // The rules changed: the same window selects another profile.
    let mut by_id: BTreeMap<String, Profile> = profiles
        .iter()
        .map(|p| (p.profile.id.clone(), p.clone()))
        .collect();
    by_id.remove("code");
    let fewer = Profiles::new(by_id);
    state.reload(&Config::default(), &fewer);
    assert_eq!(state.view(&fewer), view("global", 1, 1, AUTO));

    // The lock on the general page, then the general page is gone.
    state.press(Left, start, &fewer);
    state.release(Left, start, &fewer);
    assert_eq!(state.view(&fewer), view("global", 2, 0, AUTO));
    state.press(Left, start, &fewer);
    state.poll(start + Duration::from_secs(2), &fewer);
    state.release(Left, start + Duration::from_secs(2), &fewer);
    assert_eq!(state.view(&fewer), view("global", 2, 0, MANUAL));
    let builtin = Profiles::new(BTreeMap::new());
    state.reload(&Config::default(), &builtin);
    assert_eq!(state.view(&builtin), view("global", 1, 1, MANUAL));
    // Back in auto mode, the general page does not come back either.
    state.press(Left, start, &builtin);
    state.poll(start + Duration::from_secs(5), &builtin);
    assert_eq!(state.view(&builtin), view("global", 1, 1, AUTO));
}

#[test]
fn a_reload_that_removes_the_shown_page_shows_the_nearest_end() {
    let start = Instant::now();
    let before = Profiles::new(BTreeMap::from([
        shaped("global", &["context", "general"]),
        shaped("app", &["context", "edit", "debug"]),
    ]));
    let mut state = State::new(&Config::default(), &before);
    state.focus(window("app", "x"), start, &before);
    state.poll(start + Duration::from_secs(1), &before);
    for _ in 0..2 {
        state.press(Right, start, &before);
        state.release(Right, start, &before);
    }
    assert_eq!(state.view(&before), view("app", 3, 3, AUTO));
    let after = Profiles::new(BTreeMap::from([
        shaped("global", &["context", "general"]),
        shaped("app", &["context", "edit"]),
    ]));
    state.reload(&Config::default(), &after);
    assert_eq!(state.view(&after), view("app", 2, 2, AUTO));

    // The lock on an app page, then the app profile is gone: the global
    // context page, still locked.
    state.press(Left, start, &after);
    state.poll(start + Duration::from_secs(2), &after);
    assert_eq!(state.view(&after), view("app", 2, 2, MANUAL));
    let gone = Profiles::new(BTreeMap::from([shaped("global", &["context", "general"])]));
    state.reload(&Config::default(), &gone);
    assert_eq!(state.view(&gone), view("global", 1, 1, MANUAL));
}

#[test]
fn a_held_window_keeps_the_profile_and_page_of_the_window_before() {
    // Without the page return, which runs on whatever window is in front.
    run_with(
        &timing(0, 0),
        &profiles(),
        &[
            (Focus("org.kde.konsole", "~", 0), "global", 1, 1, AUTO),
            (Poll(150), "konsole", 1, 1, AUTO),
            (Focus("keyvo-app", "keyvo", 1000), "konsole", 1, 1, AUTO),
            (Poll(1150), "konsole", 1, 1, AUTO),
            // The page buttons work as before while held.
            (Short(Left, 2000), "global", 2, 0, AUTO),
            // A held report is no app switch.
            (Focus("org.kde.konsole", "~", 13_000), "global", 2, 0, AUTO),
            (Poll(13_150), "global", 2, 0, AUTO),
            (Focus("keyvo-app", "keyvo", 30_000), "global", 2, 0, AUTO),
            (Poll(30_150), "global", 2, 0, AUTO),
            // Back to the window held over: nothing to apply.
            (Focus("org.kde.konsole", "~", 31_000), "global", 2, 0, AUTO),
            (Poll(31_150), "global", 2, 0, AUTO),
        ],
    );
}

#[test]
fn a_held_report_cancels_a_pending_one() {
    run(
        &profiles(),
        &[
            (Focus("org.kde.konsole", "~", 0), "global", 1, 1, AUTO),
            (Poll(150), "konsole", 1, 1, AUTO),
            (Focus("code", "main.rs", 1000), "konsole", 1, 1, AUTO),
            (Focus("Keyvo-App", "keyvo", 1050), "konsole", 1, 1, AUTO),
            (Poll(1150), "konsole", 1, 1, AUTO),
            (Poll(5000), "konsole", 1, 1, AUTO),
        ],
    );
}

#[test]
fn the_window_after_a_held_one_applies_after_the_hysteresis() {
    run(
        &profiles(),
        &[
            (Focus("org.kde.konsole", "~", 0), "global", 1, 1, AUTO),
            (Poll(150), "konsole", 1, 1, AUTO),
            (Focus("keyvo-app", "keyvo", 1000), "konsole", 1, 1, AUTO),
            (Focus("code", "main.rs", 2000), "konsole", 1, 1, AUTO),
            (Poll(2149), "konsole", 1, 1, AUTO),
            (Poll(2150), "code", 2, 1, AUTO),
        ],
    );
}

#[test]
fn a_held_window_before_any_other_keeps_the_global_profile() {
    run(
        &profiles(),
        &[
            (Focus("keyvo-app", "keyvo", 0), "global", 1, 1, AUTO),
            (Poll(150), "global", 1, 1, AUTO),
            (Poll(1000), "global", 1, 1, AUTO),
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
            (Focus("org.kde.konsole", "~", 0), "global", 1, 1, AUTO),
            (Poll(150), "konsole", 1, 1, AUTO),
            (Focus("keyvo-app", "keyvo", 1000), "konsole", 1, 1, AUTO),
            (Poll(1150), "global", 1, 1, AUTO),
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
    state.reload(&Config::default(), &profiles);
    assert_eq!(state.view(&profiles), view("konsole", 1, 1, AUTO));
    assert_eq!(state.held(), Some(&window("keyvo-app", "keyvo")));
    state.poll(start + Duration::from_secs(3), &profiles);
    assert_eq!(state.view(&profiles), view("konsole", 1, 1, AUTO));
}
