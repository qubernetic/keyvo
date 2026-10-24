//! Profile files: the valid and invalid fixtures under `tests/fixtures/profiles/`.

use std::path::{Path, PathBuf};

use keyvo_core::profile::{Action, Color, KeySlot, Profile};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/profiles")
        .join(name)
}

fn parse(name: &str) -> Result<Profile, keyvo_core::diagnostic::Diagnostic> {
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
            keys: "ctrl+shift+g".into()
        })
    );
    let dirty = &commit.states["dirty"];
    assert_eq!(dirty.color, Some(Color::rgb(0xe0, 0xa8, 0x3b)));
    assert_eq!(dirty.badge.as_deref(), Some("*"));

    let general = &profile.pages[1];
    assert_eq!(general.keys[&slot(9)].label.as_deref(), Some("Mute"));
    assert_eq!(general.keys[&slot(9)].action, None);
}
