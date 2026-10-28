//! Profile files: the valid and invalid fixtures under `tests/fixtures/profiles/`.

use std::path::{Path, PathBuf};

use keyvo_core::chord::Chord;
use keyvo_core::diagnostic::{Diagnostic, Position};
use keyvo_core::profile::{Action, Color, KeySlot, Profile, derived_class};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/profiles")
        .join(name)
}

fn parse(name: &str) -> Result<Profile, Diagnostic> {
    let path = fixture(name);
    let text = std::fs::read_to_string(&path).unwrap();
    Profile::from_toml(&text, &path)
}

fn slot(n: u8) -> KeySlot {
    KeySlot::new(n).unwrap()
}

#[test]
fn full_profile_parses_every_field() {
    let profile = parse("valid/vscode.toml").unwrap();

    assert_eq!(profile.profile.id, "vscode");
    assert_eq!(profile.profile.name, "VS Code");
    assert_eq!(
        profile.profile.r#match.class,
        ["code", "com.visualstudio.code"]
    );
    let title = profile.profile.r#match.title.as_ref().unwrap();
    assert!(title.is_match("main.rs - keyvo"));
    assert!(!title.is_match("README.md - keyvo"));
    assert_eq!(profile.profile.accent, Some(Color::rgb(0x4e, 0x8b, 0xd6)));
    assert_eq!(profile.profile.priority, 10);

    assert_eq!(profile.pages.len(), 2);
    let context = &profile.pages[0];
    assert_eq!(context.name.as_deref(), Some("context"));
    assert_eq!(
        context.keys.keys().copied().collect::<Vec<_>>(),
        [slot(1), slot(3)]
    );

    let build = &context.keys[&slot(1)];
    assert_eq!(build.icon.as_deref(), Some("lucide:hammer"));
    assert_eq!(build.label.as_deref(), Some("Build"));
    assert_eq!(build.color, None);
    assert_eq!(
        build.action,
        Some(Action::Exec {
            cmd: "cargo build".into()
        })
    );
    assert_eq!(build.states["error"].badge.as_deref(), Some("!"));

    let commit = &context.keys[&slot(3)];
    assert_eq!(commit.color, Some(Color::rgb(0x4e, 0x8b, 0xd6)));
    assert_eq!(
        commit.action,
        Some(Action::Chord {
            keys: Chord::parse("ctrl+shift+g").unwrap()
        })
    );
    let dirty = &commit.states["dirty"];
    assert_eq!(dirty.color, Some(Color::rgb(0xe0, 0xa8, 0x3b)));
    assert_eq!(dirty.badge.as_deref(), Some("*"));

    let general = &profile.pages[1];
    assert_eq!(general.name.as_deref(), Some("general"));
    assert_eq!(general.keys[&slot(9)].label.as_deref(), Some("Mute"));
    assert_eq!(general.keys[&slot(9)].action, None);
}

/// Every invalid fixture with the position and the message of its diagnostic.
const INVALID: &[(&str, usize, usize, &str)] = &[
    ("bad-chord.toml", 9, 35, "invalid chord: unknown key `nope`"),
    (
        "bad-color.toml",
        9,
        9,
        "invalid value: string \"blue\", expected a color written #rrggbb",
    ),
    (
        "bad-id.toml",
        4,
        8,
        "invalid value: string \"VS Code\", expected an id of lowercase letters",
    ),
    (
        "bad-title-regex.toml",
        6,
        37,
        "invalid title regex: unclosed group",
    ),
    ("chord-without-keys.toml", 9, 10, "missing field `keys`"),
    (
        "key-out-of-range.toml",
        8,
        13,
        "invalid value: string \"10\", expected a key number from 1 to 9",
    ),
    ("missing-name.toml", 3, 1, "missing field `name`"),
    (
        "nested-on-fail.toml",
        10,
        66,
        "an action has no `on_fail` of its own",
    ),
    (
        "priority-not-integer.toml",
        7,
        12,
        "invalid type: string \"high\", expected i32",
    ),
    ("syntax-error.toml", 9, 22, "invalid basic string"),
    (
        "unknown-action.toml",
        9,
        19,
        "unknown variant `recipe`, expected one of `chord`, `exec`, `launch`",
    ),
    (
        "unknown-field.toml",
        10,
        1,
        "unknown field `colour`, expected one of",
    ),
    (
        "wrong-schema-version.toml",
        1,
        18,
        "unsupported schema_version 2, this keyvo reads 1",
    ),
];

#[test]
fn invalid_fixtures_report_file_line_and_column() {
    for &(name, line, column, message) in INVALID {
        let diagnostic = parse(&format!("invalid/{name}")).unwrap_err();
        assert_eq!(
            diagnostic.path,
            fixture(&format!("invalid/{name}")),
            "{name}"
        );
        assert_eq!(
            diagnostic.position,
            Some(Position { line, column }),
            "{name}: {diagnostic}"
        );
        assert!(
            diagnostic.message.starts_with(message),
            "{name}: {}",
            diagnostic.message
        );
        assert!(
            diagnostic
                .to_string()
                .contains(&format!("{name}:{line}:{column}: ")),
            "{diagnostic}"
        );
    }
}

#[test]
fn every_invalid_fixture_is_in_the_table() {
    let mut files: Vec<String> = std::fs::read_dir(fixture("invalid"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    files.sort();
    let table: Vec<&str> = INVALID.iter().map(|row| row.0).collect();
    assert_eq!(files, table);
}

#[test]
fn an_action_table_needs_the_fields_of_its_type() {
    let table = [
        (r#"{ keys = "a" }"#, "missing field `type`"),
        (r#"{ type = "chord" }"#, "missing field `keys`"),
        (r#"{ type = "exec" }"#, "missing field `cmd`"),
        (
            r#"{ type = "chord", keys = "a", cmd = "true" }"#,
            "unknown field `cmd`",
        ),
        (
            r#"{ type = "exec", cmd = "true", keys = "a" }"#,
            "unknown field `keys`",
        ),
        (
            r#"{ type = "exec", cmd = "true", when = 1 }"#,
            "unknown field `when`",
        ),
        (
            r#""ctrl+s""#,
            "invalid type: string \"ctrl+s\", expected an action table",
        ),
    ];
    for (action, message) in table {
        let text = format!(
            "schema_version = 1\n[profile]\nid = \"t\"\nname = \"T\"\n[[pages]]\n[pages.keys.1]\naction = {action}\n"
        );
        let diagnostic = Profile::from_toml(&text, Path::new("t.toml")).unwrap_err();
        assert!(
            diagnostic.message.starts_with(message),
            "{action}: {diagnostic}"
        );
    }
    let text = "schema_version = 1\n[profile]\nid = \"t\"\nname = \"T\"\n[[pages]]\n[pages.keys.1]\naction = { cmd = \"make\", type = \"exec\" }\n";
    let profile = Profile::from_toml(text, Path::new("t.toml")).unwrap();
    assert_eq!(
        profile.pages[0].keys[&KeySlot::new(1).unwrap()].action,
        Some(Action::Exec { cmd: "make".into() })
    );
}

#[test]
fn a_launch_action_is_a_command_line_like_exec() {
    let table = [
        (r#"{ type = "launch" }"#, "missing field `cmd`"),
        (
            r#"{ type = "launch", cmd = "kcalc", keys = "a" }"#,
            "unknown field `keys`",
        ),
    ];
    for (action, message) in table {
        let text = format!(
            "schema_version = 1\n[profile]\nid = \"t\"\nname = \"T\"\n[[pages]]\n[pages.keys.1]\naction = {action}\n"
        );
        let diagnostic = Profile::from_toml(&text, Path::new("t.toml")).unwrap_err();
        assert!(
            diagnostic.message.starts_with(message),
            "{action}: {diagnostic}"
        );
    }
    let text = "schema_version = 1\n[profile]\nid = \"t\"\nname = \"T\"\n[[pages]]\n[pages.keys.1]\naction = { type = \"launch\", cmd = \"flatpak run org.kde.kcalc\" }\n";
    let profile = Profile::from_toml(text, Path::new("t.toml")).unwrap();
    let launch = Action::launch("flatpak run org.kde.kcalc");
    assert_eq!(
        profile.pages[0].keys[&KeySlot::new(1).unwrap()].action,
        Some(launch.clone())
    );
    assert_eq!(
        serde_json::to_value(&launch).unwrap(),
        serde_json::json!({ "type": "launch", "cmd": "flatpak run org.kde.kcalc" })
    );
}

fn action_of(action: &str) -> Result<Option<Action>, Diagnostic> {
    let text = format!(
        "schema_version = 1\n[profile]\nid = \"t\"\nname = \"T\"\n[[pages]]\n[pages.keys.1]\naction = {action}\n"
    );
    Profile::from_toml(&text, Path::new("t.toml")).map(|profile| {
        profile.pages[0].keys[&KeySlot::new(1).unwrap()]
            .action
            .clone()
    })
}

#[test]
fn a_launch_names_its_window_class_and_whether_it_waits_for_the_window() {
    let action = action_of(
        r#"{ type = "launch", wait_focus = false, cmd = "kcalc", class = "org.kde.kcalc" }"#,
    )
    .unwrap();
    let launch = Action::Launch {
        cmd: "kcalc".into(),
        class: Some("org.kde.kcalc".into()),
        wait_focus: false,
    };
    assert_eq!(action, Some(launch.clone()));
    assert_eq!(
        serde_json::to_value(&launch).unwrap(),
        serde_json::json!({
            "type": "launch",
            "cmd": "kcalc",
            "class": "org.kde.kcalc",
            "wait_focus": false
        })
    );
    // Both fields are optional: no class, waiting for the window.
    assert_eq!(
        action_of(r#"{ type = "launch", cmd = "kcalc", wait_focus = true }"#).unwrap(),
        Some(Action::launch("kcalc"))
    );
    let table = [
        (
            r#"{ type = "exec", cmd = "true", class = "x" }"#,
            "unknown field `class`",
        ),
        (
            r#"{ type = "exec", cmd = "true", wait_focus = false }"#,
            "unknown field `wait_focus`",
        ),
        (
            r#"{ type = "chord", keys = "a", class = "x" }"#,
            "unknown field `class`",
        ),
        (
            r#"{ type = "chord", keys = "a", wait_focus = true }"#,
            "unknown field `wait_focus`",
        ),
        (
            r#"{ type = "launch", cmd = "kcalc", wait_focus = 1 }"#,
            "invalid type: integer `1`, expected a boolean",
        ),
        (
            r#"{ type = "launch", cmd = "kcalc", class = 1 }"#,
            "invalid type: integer `1`, expected a string",
        ),
    ];
    for (action, message) in table {
        let diagnostic = action_of(action).unwrap_err();
        assert!(
            diagnostic.message.starts_with(message),
            "{action}: {diagnostic}"
        );
    }
}

fn on_fail_of(on_fail: &str) -> Result<Option<Action>, Diagnostic> {
    let text = format!(
        "schema_version = 1\n[profile]\nid = \"t\"\nname = \"T\"\n[[pages]]\n[pages.keys.1]\n\
         action = {{ type = \"exec\", cmd = \"make\" }}\non_fail = {on_fail}\n"
    );
    Profile::from_toml(&text, Path::new("t.toml")).map(|profile| {
        profile.pages[0].keys[&KeySlot::new(1).unwrap()]
            .on_fail
            .clone()
    })
}

#[test]
fn a_key_runs_any_action_on_fail() {
    let table = [
        (
            r#"{ type = "exec", cmd = "notify-send failed" }"#,
            Action::Exec {
                cmd: "notify-send failed".into(),
            },
        ),
        (
            r#"{ type = "launch", cmd = "konsole" }"#,
            Action::launch("konsole"),
        ),
        (
            r#"{ type = "chord", keys = "ctrl+z" }"#,
            Action::Chord {
                keys: Chord::parse("ctrl+z").unwrap(),
            },
        ),
    ];
    for (on_fail, action) in table {
        assert_eq!(on_fail_of(on_fail).unwrap(), Some(action), "{on_fail}");
    }
    assert_eq!(
        action_of(r#"{ type = "exec", cmd = "make" }"#)
            .unwrap()
            .map(|a| a.summary()),
        Some("make".to_owned())
    );
    let profile = parse("valid/vscode.toml").unwrap();
    assert_eq!(profile.pages[0].keys[&slot(3)].on_fail, None);
}

#[test]
fn an_action_has_no_on_fail_of_its_own() {
    for on_fail in [
        r#"{ type = "exec", cmd = "a", on_fail = { type = "exec", cmd = "b" } }"#,
        r#"{ on_fail = { type = "exec", cmd = "b" }, type = "exec", cmd = "a" }"#,
    ] {
        let diagnostic = on_fail_of(on_fail).unwrap_err();
        assert!(
            diagnostic
                .message
                .starts_with("an action has no `on_fail` of its own"),
            "{on_fail}: {diagnostic}"
        );
    }
}

#[test]
fn a_flatpak_launch_derives_its_class_from_the_app_id() {
    let table = [
        ("flatpak run org.kde.kcalc", Some("org.kde.kcalc")),
        (
            "flatpak run --branch=stable org.kde.kcalc",
            Some("org.kde.kcalc"),
        ),
        (
            "/usr/bin/flatpak run org.kde.kcalc --help",
            Some("org.kde.kcalc"),
        ),
        (
            "flatpak  run\tcom.spotify.Client",
            Some("com.spotify.Client"),
        ),
        // Not a Flatpak launch, or no app id where one belongs.
        ("kcalc", None),
        ("flatpak list", None),
        ("flatpak run", None),
        ("flatpak run --branch stable", None),
        ("flatpak run kcalc", None),
        ("echo flatpak run org.kde.kcalc", None),
    ];
    for (cmd, class) in table {
        assert_eq!(derived_class(cmd).as_deref(), class, "{cmd}");
    }
    // A launch's class is its own, else the derived one.
    let derived = Action::launch("flatpak run org.kde.kcalc");
    assert_eq!(derived.launch_class().as_deref(), Some("org.kde.kcalc"));
    let own = Action::Launch {
        cmd: "flatpak run org.kde.kcalc".into(),
        class: Some("kcalc".into()),
        wait_focus: true,
    };
    assert_eq!(own.launch_class().as_deref(), Some("kcalc"));
    assert_eq!(Action::launch("kcalc").launch_class(), None);
    assert_eq!(Action::Exec { cmd: "x".into() }.launch_class(), None);
}
