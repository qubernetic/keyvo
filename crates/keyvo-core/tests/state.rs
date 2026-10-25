//! The context engine (spec §6): profile selection from a window context
//! and the mode and page state machine, on the profiles of
//! `tests/fixtures/dirs/focus/`.

use std::collections::BTreeMap;
use std::path::Path;

use keyvo_core::focus::WindowContext;
use keyvo_core::layout::{self, Profiles};
use keyvo_core::profile::Profile;

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
