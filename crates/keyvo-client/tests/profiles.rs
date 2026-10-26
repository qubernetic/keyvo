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
    assert_eq!(json["file"], serde_json::json!(true));
}

#[test]
fn open_in_editor_hands_the_file_to_the_desktop() {
    let command = profiles::opener(Path::new("/c/profiles/konsole.toml"));
    assert_eq!(command.get_program(), "xdg-open");
    let args: Vec<_> = command.get_args().collect();
    assert_eq!(args, ["/c/profiles/konsole.toml"]);
}
