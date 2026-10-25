//! The example profiles under `contrib/profiles/`, which
//! `docs/pre-alpha-test.md` installs into `~/.config/keyvo/profiles/`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use keyvo_core::focus::WindowContext;
use keyvo_core::layout::{Loaded, load};
use keyvo_core::profile::{Action, Key, Profile};
use keyvo_core::render::icon;

/// `contrib/`, laid out like a configuration directory: `profiles/*.toml`
/// and no `config.toml`, so the defaults apply as on a fresh install.
fn contrib() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contrib")
}

fn loaded() -> Loaded {
    load(&contrib()).unwrap_or_else(|errors| panic!("{errors}"))
}

fn keys(profile: &Profile) -> impl Iterator<Item = &Key> {
    profile.pages.iter().flat_map(|page| page.keys.values())
}

fn all_keys(loaded: &Loaded) -> Vec<&Key> {
    loaded.profiles.iter().flat_map(keys).collect()
}

fn exec(cmd: &str) -> Action {
    Action::Exec { cmd: cmd.into() }
}

#[test]
fn every_example_profile_loads_and_each_file_is_its_own_profile() {
    let loaded = loaded();
    let ids: Vec<_> = loaded
        .profiles
        .iter()
        .map(|p| p.profile.id.as_str())
        .collect();
    assert_eq!(ids, ["global", "konsole", "vscode"]);
    let files = std::fs::read_dir(contrib().join("profiles"))
        .unwrap()
        .filter(|entry| {
            let path = entry.as_ref().unwrap().path();
            path.extension().is_some_and(|ext| ext == "toml")
        })
        .count();
    assert_eq!(files, ids.len(), "no file shadowed by the built-in global");
}

#[test]
fn the_app_profiles_match_their_window_classes_and_nothing_else() {
    let loaded = loaded();
    let shown = |class: &str| {
        loaded
            .profiles
            .select(&WindowContext::new(class, "x"))
            .profile
            .id
            .clone()
    };
    assert_eq!(shown("org.kde.konsole"), "konsole");
    assert_eq!(shown("konsole"), "konsole");
    assert_eq!(shown("code"), "vscode");
    assert_eq!(shown("com.visualstudio.code"), "vscode");
    assert_eq!(shown("org.kde.dolphin"), "global");
}

#[test]
fn every_profile_has_its_own_accent() {
    let loaded = loaded();
    let accents: Vec<_> = loaded.profiles.iter().map(|p| p.profile.accent).collect();
    assert!(accents.iter().all(Option::is_some), "{accents:?}");
    let distinct: BTreeSet<_> = accents.iter().map(|a| format!("{a:?}")).collect();
    assert_eq!(distinct.len(), accents.len(), "{accents:?}");
}

#[test]
fn the_keys_cover_a_chord_and_the_running_success_and_error_looks() {
    let loaded = loaded();
    let actions: Vec<_> = all_keys(&loaded)
        .iter()
        .filter_map(|k| k.action.clone())
        .collect();
    assert!(actions.iter().any(|a| matches!(a, Action::Chord { .. })));
    // Success, error, and a run past `[render].long_run_threshold` (10 s)
    // that still ends before `[timing].exec_timeout` (30 s).
    for cmd in ["true", "exit 1", "sleep 15"] {
        assert!(actions.contains(&exec(cmd)), "no exec key `{cmd}`");
    }
}

#[test]
fn the_global_profile_has_a_context_and_a_general_page() {
    let loaded = loaded();
    let pages: Vec<_> = loaded
        .profiles
        .global()
        .pages
        .iter()
        .map(|p| p.name.as_deref())
        .collect();
    assert_eq!(pages, [Some("context"), Some("general")]);
}

#[test]
fn every_icon_is_bundled_and_every_key_has_a_label() {
    for key in all_keys(&loaded()) {
        if let Some(reference) = &key.icon {
            assert!(icon::lookup(reference).is_ok(), "{reference}");
        }
        assert!(key.label.is_some(), "{key:?}");
    }
}
