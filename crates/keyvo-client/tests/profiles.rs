//! The profiles the Profiles screen lists (`docs/design/gui-ux.md` §3.2):
//! each with what its header, match rules and page tabs show and the file
//! "Open in editor" opens.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use keyvo_client::profiles::{self, MatchSummary, ProfileSummary};
use keyvo_core::layout;

fn dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../keyvo-core/tests/fixtures/dirs")
        .join(name)
}

#[test]
fn every_profile_is_listed_in_id_order_with_its_header_and_pages() {
    let dir = dir("full");
    let loaded = layout::load(&dir).unwrap();
    let list = profiles::list(&loaded.profiles, &dir);
    let ids: Vec<&str> = list.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["global", "konsole"]);
    let konsole = &list[1];
    assert_eq!(
        *konsole,
        ProfileSummary {
            id: "konsole".to_owned(),
            name: "Konsole".to_owned(),
            accent: Some("#2ecc71".to_owned()),
            priority: 5,
            r#match: MatchSummary {
                class: vec!["org.kde.konsole".to_owned(), "konsole".to_owned()],
                title: None,
            },
            pages: konsole.pages.clone(),
            disabled: Vec::new(),
            global_pages: true,
            path: dir.join("profiles/konsole.toml"),
            file: true,
        }
    );
    let profile = loaded.profiles.get("konsole").unwrap();
    let names: Vec<Option<String>> = profile.pages.iter().map(|p| p.name.clone()).collect();
    assert_eq!(konsole.pages, names);
}

#[test]
fn the_builtin_global_profile_has_no_file_yet() {
    let empty = tempfile::tempdir().unwrap();
    let loaded = layout::load(empty.path()).unwrap();
    let list = profiles::list(&loaded.profiles, empty.path());
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, "global");
    assert_eq!(list[0].name, "Global");
    assert_eq!(list[0].pages, [Some("context".to_owned())]);
    assert!(!list[0].file);
    assert_eq!(list[0].path, empty.path().join("profiles/global.toml"));
}

#[test]
fn a_title_rule_is_listed_as_written_and_absent_fields_are_left_out() {
    let dir = dir("focus");
    let loaded = layout::load(&dir).unwrap();
    let list = profiles::list(&loaded.profiles, &dir);
    let titled = list.iter().find(|p| p.id == "titled").unwrap();
    assert_eq!(titled.r#match.title.as_deref(), Some("pull request"));
    let json = serde_json::to_value(titled).unwrap();
    assert_eq!(json["match"]["class"], serde_json::json!([]));
    assert!(json.get("accent").is_none(), "{json}");
    assert!(json.get("disabled").is_none(), "{json}");
    assert!(json.get("global_pages").is_none(), "{json}");
    assert_eq!(json["file"], serde_json::json!(true));
}

#[test]
fn the_disabled_pages_and_a_profile_without_the_global_pages_are_listed() {
    let root = tempfile::tempdir().unwrap();
    let profiles = root.path().join("profiles");
    std::fs::create_dir(&profiles).unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/protocol-fixtures/editor/pages.4.toml"),
        profiles.join("konsole.toml"),
    )
    .unwrap();
    std::fs::write(
        profiles.join("code.toml"),
        "schema_version = 1\n[profile]\nid = \"code\"\nname = \"Code\"\nglobal_pages = false\n",
    )
    .unwrap();
    let loaded = layout::load(root.path()).unwrap();
    let list = profiles::list(&loaded.profiles, root.path());
    let konsole = list.iter().find(|p| p.id == "konsole").unwrap();
    assert_eq!(konsole.disabled, [2]);
    let code = serde_json::to_value(list.iter().find(|p| p.id == "code").unwrap()).unwrap();
    assert_eq!(code["global_pages"], serde_json::json!(false));
    let konsole = serde_json::to_value(konsole).unwrap();
    assert_eq!(konsole["disabled"], serde_json::json!([2]));
}

#[test]
fn open_in_editor_hands_the_file_to_the_desktop() {
    let command = profiles::opener(Path::new("/c/profiles/konsole.toml"));
    assert_eq!(command.get_program(), "xdg-open");
    let args: Vec<_> = command.get_args().collect();
    assert_eq!(args, ["/c/profiles/konsole.toml"]);
}
