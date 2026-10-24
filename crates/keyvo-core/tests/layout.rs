//! Loading a whole configuration directory: the trees under
//! `tests/fixtures/dirs/`.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use keyvo_core::config::Config;
use keyvo_core::diagnostic::Position;
use keyvo_core::layout::{LoadError, config_dir, load};
use keyvo_core::profile::{GLOBAL_ID, Profile};

fn dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/dirs")
        .join(name)
}

#[test]
fn config_dir_follows_xdg_config_home_then_home() {
    let some = |s: &'static str| Some(OsStr::new(s));
    assert_eq!(
        config_dir(some("/x/cfg"), some("/home/u")),
        Some(PathBuf::from("/x/cfg/keyvo"))
    );
    // Empty or relative XDG_CONFIG_HOME is ignored (XDG Base Directory spec).
    for xdg in [None, some(""), some("relative/cfg")] {
        assert_eq!(
            config_dir(xdg, some("/home/u")),
            Some(PathBuf::from("/home/u/.config/keyvo"))
        );
    }
    assert_eq!(config_dir(None, None), None);
    assert_eq!(config_dir(None, some("")), None);
}

#[test]
fn a_missing_directory_is_the_defaults_and_the_builtin_global() {
    let tmp = tempfile::tempdir().unwrap();
    let loaded = load(&tmp.path().join("keyvo")).unwrap();
    assert_eq!(loaded.config, Config::default());
    assert_eq!(loaded.profiles.len(), 1);
    assert_eq!(loaded.profiles.global(), &Profile::builtin_global());
}

#[test]
fn a_full_directory_loads_config_and_every_toml_profile() {
    let loaded = load(&dir("full")).unwrap();
    assert_eq!(loaded.config.timing.long_press, Duration::from_millis(700));

    let ids: Vec<&str> = loaded
        .profiles
        .iter()
        .map(|p| p.profile.id.as_str())
        .collect();
    assert_eq!(ids, ["global", "konsole"]);
    assert_eq!(loaded.profiles.global().profile.name, "Everywhere");
    assert_eq!(loaded.profiles.get("konsole").unwrap().profile.priority, 5);
    assert_eq!(loaded.profiles.get("README"), None);
}

#[test]
fn without_global_toml_the_builtin_global_is_added() {
    let tmp = tempfile::tempdir().unwrap();
    let profiles = tmp.path().join("profiles");
    std::fs::create_dir(&profiles).unwrap();
    std::fs::copy(
        dir("full").join("profiles/konsole.toml"),
        profiles.join("konsole.toml"),
    )
    .unwrap();

    let loaded = load(tmp.path()).unwrap();
    assert_eq!(loaded.profiles.global().profile.id, GLOBAL_ID);
    assert_eq!(loaded.profiles.global(), &Profile::builtin_global());
    assert!(loaded.profiles.get("konsole").is_some());
}

#[test]
fn every_broken_file_is_reported_in_path_order() {
    let base = dir("broken");
    let errors = load(&base).unwrap_err();
    let found: Vec<(PathBuf, Option<Position>, String)> = errors
        .0
        .iter()
        .map(|e| match e {
            LoadError::Invalid(d) => (d.path.clone(), d.position, d.message.clone()),
            LoadError::Io { path, source } => panic!("{}: {source}", path.display()),
        })
        .collect();
    let at = |line, column| Some(Position { line, column });
    assert_eq!(
        found,
        [
            (
                base.join("config.toml"),
                at(2, 17),
                "invalid type: string \"2s\", expected a duration in milliseconds".to_owned()
            ),
            (
                base.join("profiles/broken.toml"),
                at(6, 10),
                "invalid value: string \"green\", expected a color written #rrggbb".to_owned()
            ),
            (
                base.join("profiles/terminal.toml"),
                at(4, 8),
                "profile id `konsole` does not match the file name `terminal.toml`".to_owned()
            ),
        ]
    );
    let text = errors.to_string();
    assert_eq!(text.lines().count(), 3, "{text}");
    assert!(text.contains("terminal.toml:4:8: profile id"), "{text}");
}

#[test]
fn an_unreadable_profile_is_an_io_error_with_its_path() {
    let tmp = tempfile::tempdir().unwrap();
    let profiles = tmp.path().join("profiles");
    // A directory named like a profile cannot be read as a file.
    std::fs::create_dir_all(profiles.join("odd.toml")).unwrap();

    let errors = load(tmp.path()).unwrap_err();
    match errors.0.as_slice() {
        [LoadError::Io { path, .. }] => assert_eq!(path, &profiles.join("odd.toml")),
        other => panic!("{other:?}"),
    }
    assert!(
        errors
            .to_string()
            .starts_with(&profiles.join("odd.toml").display().to_string())
    );
}
