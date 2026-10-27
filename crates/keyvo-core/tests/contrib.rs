//! The example profiles under `contrib/profiles/`, which
//! `docs/pre-alpha-test.md` installs into `~/.config/keyvo/profiles/`, and
//! the recipe packs under `contrib/recipes/`, which keyvo bundles.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use keyvo_core::focus::WindowContext;
use keyvo_core::layout::{Loaded, load};
use keyvo_core::profile::{Action, Key, Profile};
use keyvo_core::recipe::{self, Group, Recipe, Source};
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
fn the_global_profile_has_a_context_a_general_and_a_third_page() {
    let loaded = loaded();
    let pages: Vec<_> = loaded
        .profiles
        .global()
        .pages
        .iter()
        .map(|p| p.name.as_deref())
        .collect();
    assert_eq!(pages, [Some("context"), Some("general"), Some("third")]);
}

/// The recipe packs under `contrib/recipes/`, which the daemon serves as
/// the bundled ones.
fn packs() -> Vec<Recipe> {
    let catalogue = recipe::load_dir(&contrib().join("recipes"), Source::Bundled);
    assert!(catalogue.problems.is_empty(), "{:?}", catalogue.problems);
    catalogue.recipes
}

#[test]
fn every_recipe_pack_validates_and_is_the_bundled_one() {
    let packs = packs();
    let bundled = recipe::bundled();
    assert!(bundled.problems.is_empty(), "{:?}", bundled.problems);
    assert_eq!(bundled.recipes, packs, "recipe::BUNDLED lists every pack");
}

#[test]
fn the_v1_catalogue_has_its_entries_in_their_groups() {
    let names = |group: Group| -> Vec<String> {
        packs()
            .into_iter()
            .filter(|r| r.group == group)
            .map(|r| r.name)
            .collect()
    };
    assert_eq!(
        names(Group::General),
        ["New email", "Open folder", "Open website"]
    );
    assert_eq!(
        names(Group::System),
        [
            "Brightness down",
            "Brightness up",
            "Lock the desktop",
            "Mute",
            "Open a settings page",
            "Volume down",
            "Volume up"
        ]
    );
    assert_eq!(
        names(Group::Apps),
        ["Calculator", "Konsole", "Spectacle", "VS Code"]
    );
}

#[test]
fn the_app_recipes_offer_what_they_do() {
    let variants = |id: &str| -> Vec<String> {
        let pack = packs().into_iter().find(|r| r.id == id).unwrap();
        pack.variants.into_iter().map(|v| v.id).collect()
    };
    assert_eq!(variants("spectacle"), ["fullscreen", "region", "recording"]);
    assert_eq!(variants("vscode"), ["palette", "build", "run"]);
    assert_eq!(variants("konsole"), ["new-tab", "find"]);
}

#[test]
fn every_recipe_has_a_bundled_icon_and_a_label() {
    for pack in packs() {
        assert!(
            icon::lookup(&pack.icon).is_ok(),
            "{}: {}",
            pack.id,
            pack.icon
        );
        assert!(!pack.label.trim().is_empty(), "{}", pack.id);
    }
}

/// A variant that opens an application or a settings page, or records
/// until it is stopped, stays open past `[timing].exec_timeout`, so it is a
/// `launch` (#155); a command that exits soon stays an `exec`.
#[test]
fn the_variants_that_stay_open_are_launches() {
    let launches: BTreeSet<String> = packs()
        .into_iter()
        .flat_map(|pack| {
            pack.variants
                .into_iter()
                .filter(|v| matches!(v.action, Action::Launch { .. }))
                .map(move |v| format!("{}/{}", pack.id, v.id))
        })
        .collect();
    let expected: BTreeSet<String> = [
        "calculator/flatpak",
        "calculator/kcalc",
        "settings-page/bluetooth",
        "settings-page/display",
        "settings-page/keyboard",
        "settings-page/network",
        "settings-page/power",
        "settings-page/sound",
        "spectacle/recording",
    ]
    .map(str::to_owned)
    .into();
    assert_eq!(launches, expected);
}

#[test]
fn the_third_global_page_has_a_launch_key() {
    let loaded = loaded();
    let third = &loaded.profiles.global().pages[2];
    assert!(
        third.keys.values().any(|key| key.action
            == Some(Action::Launch {
                cmd: "sleep 60".into()
            })),
        "{third:?}"
    );
}

/// The user test of #119 presses "Volume up" and expects the success look,
/// which only an `exec` shows.
#[test]
fn the_default_volume_variant_shows_feedback() {
    let volume = packs().into_iter().find(|r| r.id == "volume-up").unwrap();
    assert!(volume.variants[0].action.has_feedback());
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
