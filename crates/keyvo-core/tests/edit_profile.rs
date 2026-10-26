//! Round-trip editing of the profile header (spec §7.1, ADR-0004,
//! `docs/design/gui-ux.md` §3.2) against the fixtures in
//! `tests/fixtures/edit-profile/`: every case applies `<case>.json` to
//! `edit/before.toml` and must give `<case>.toml` byte for byte, with only
//! the line of the edited field changed.

use std::path::{Path, PathBuf};

use keyvo_core::edit::{ProfileDocument, ProfileFields};
use keyvo_core::profile::{Color, Profile};

/// Every case in the directory, with the `[profile]` field it edits; a test
/// fails when a file is missing here.
const CASES: &[(&str, &str)] = &[
    ("name", "name"),
    ("accent", "accent"),
    ("accent-removed", "accent"),
];

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn read(name: &str) -> String {
    std::fs::read_to_string(fixtures().join(name)).unwrap()
}

fn before() -> String {
    read("edit/before.toml")
}

fn case(name: &str) -> ProfileFields {
    serde_json::from_str(&read(&format!("edit-profile/{name}.json"))).unwrap()
}

fn document(text: &str) -> ProfileDocument {
    ProfileDocument::parse(text, Path::new("edits.toml")).unwrap()
}

#[test]
fn every_case_gives_its_expected_file_byte_for_byte() {
    for (name, _) in CASES {
        let fields = case(name);
        let mut doc = document(&before());
        let old = doc.set_profile(&fields).unwrap();
        assert_eq!(
            doc.to_string(),
            read(&format!("edit-profile/{name}.toml")),
            "case {name}"
        );
        assert_eq!(doc.profile_fields(), fields, "case {name}");
        assert_eq!(old, document(&before()).profile_fields(), "case {name}");
    }
}

#[test]
fn every_case_changes_only_the_line_of_its_field() {
    let old: Vec<String> = before().lines().map(str::to_owned).collect();
    for (name, field) in CASES {
        let new: Vec<String> = read(&format!("edit-profile/{name}.toml"))
            .lines()
            .map(str::to_owned)
            .collect();
        let at = old
            .iter()
            .position(|line| line.starts_with(&format!("{field} ")))
            .unwrap();
        let mut expected = old.clone();
        if new.len() < old.len() {
            expected.remove(at);
        } else {
            expected[at].clone_from(&new[at]);
        }
        assert_eq!(new, expected, "case {name}");
    }
}

#[test]
fn every_case_keeps_the_pages_and_the_other_fields() {
    let parsed = |text: &str| Profile::from_toml(text, Path::new("edits.toml")).unwrap();
    let old = parsed(&before());
    for (name, _) in CASES {
        let fields = case(name);
        let mut new = parsed(&read(&format!("edit-profile/{name}.toml")));
        assert_eq!(new.profile.name, fields.name, "case {name}");
        assert_eq!(new.profile.accent, fields.accent, "case {name}");
        new.profile.name.clone_from(&old.profile.name);
        new.profile.accent = old.profile.accent;
        assert_eq!(new, old, "case {name}");
    }
}

#[test]
fn every_fixture_file_is_a_listed_case() {
    for entry in std::fs::read_dir(fixtures().join("edit-profile")).unwrap() {
        let path = entry.unwrap().path();
        let stem = path.file_stem().unwrap().to_str().unwrap();
        assert!(
            CASES.iter().any(|(name, _)| *name == stem),
            "{} is not in CASES",
            path.display()
        );
    }
}

#[test]
fn the_same_fields_change_nothing() {
    let mut doc = document(&before());
    let fields = doc.profile_fields();
    assert_eq!(doc.set_profile(&fields).unwrap(), fields);
    assert_eq!(doc.to_string(), before());
}

#[test]
fn an_accent_added_goes_to_the_end_of_the_profile_table() {
    let text =
        "schema_version = 1\n\n[profile]\nid = \"edits\"\nname = \"Plain\" # keep\n\n[[pages]]\n";
    let mut doc = document(text);
    let fields = ProfileFields {
        name: "Plain".to_owned(),
        accent: Some(Color {
            r: 0x11,
            g: 0x22,
            b: 0x33,
        }),
    };
    doc.set_profile(&fields).unwrap();
    assert_eq!(
        doc.to_string(),
        "schema_version = 1\n\n[profile]\nid = \"edits\"\nname = \"Plain\" # keep\naccent = \"#112233\"\n\n[[pages]]\n"
    );
}

#[test]
fn an_inline_profile_table_stays_inline() {
    let text = "schema_version = 1\nprofile = { id = \"edits\", name = \"Inline\" }\n";
    let mut doc = document(text);
    let fields = ProfileFields {
        name: "Renamed".to_owned(),
        accent: None,
    };
    doc.set_profile(&fields).unwrap();
    assert_eq!(
        doc.to_string(),
        "schema_version = 1\nprofile = { id = \"edits\", name = \"Renamed\" }\n"
    );
}
