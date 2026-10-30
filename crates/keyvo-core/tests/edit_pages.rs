//! Round-trip editing of a profile's pages (spec §10, ADR-0004) against the
//! shared steps of `tests/protocol-fixtures/editor/pages.json`: every step
//! applies its edit to the file before it and must give its file byte for
//! byte, and its undo must give the file before it again.

use std::path::{Path, PathBuf};

use keyvo_core::edit::{EditError, PageEdit, ProfileDocument};

#[derive(serde::Deserialize)]
struct Fixture {
    file: String,
    steps: Vec<Step>,
    refused: Vec<Refused>,
}

#[derive(serde::Deserialize)]
struct Step {
    edit: PageEdit,
    file: String,
    page: Option<usize>,
    undo: Option<PageEdit>,
}

#[derive(serde::Deserialize)]
struct Refused {
    edit: PageEdit,
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/protocol-fixtures/editor")
}

fn read(name: &str) -> String {
    std::fs::read_to_string(fixtures().join(name)).unwrap()
}

fn steps() -> Fixture {
    serde_json::from_str(&read("pages.json")).unwrap()
}

fn document(text: &str) -> ProfileDocument {
    ProfileDocument::parse(text, Path::new("konsole.toml")).unwrap()
}

#[test]
fn every_step_gives_its_file_and_its_undo_the_file_before() {
    let steps = steps();
    let mut before = read(&steps.file);
    for step in &steps.steps {
        let mut doc = document(&before);
        let done = doc.edit_page(&step.edit).unwrap();
        if std::env::var_os("KEYVO_BLESS").is_some() {
            std::fs::write(fixtures().join(&step.file), doc.to_string()).unwrap();
        }
        let after = read(&step.file);
        assert_eq!(doc.to_string(), after, "{}", step.file);
        assert_eq!(done.page, step.page, "{}", step.file);
        assert_eq!(done.undo, step.undo, "{}", step.file);
        if let Some(undo) = &done.undo {
            let mut doc = document(&after);
            doc.edit_page(undo).unwrap();
            assert_eq!(doc.to_string(), before, "undo of {}", step.file);
        }
        before = after;
    }
}

#[test]
fn the_context_page_stays_on_and_the_only_enabled_page_stays() {
    let steps = steps();
    let last = steps.steps.last().unwrap();
    let text = read(&last.file);
    for refused in &steps.refused {
        let mut doc = document(&text);
        let error = doc.edit_page(&refused.edit).unwrap_err();
        assert!(
            matches!(error, EditError::Protected { page: 1, .. }),
            "{error}"
        );
        assert_eq!(doc.to_string(), text);
    }
    // With another page on, the context page can go; the first enabled page
    // is the context page then.
    let mut doc = document(&read(&steps.steps[3].file));
    doc.edit_page(&PageEdit::Delete { page: 1 }).unwrap();
    assert_eq!(doc.profile().pages.len(), 3);
    let error = doc
        .edit_page(&PageEdit::Enable {
            page: 2,
            enabled: false,
        })
        .unwrap_err();
    assert!(
        matches!(error, EditError::Protected { page: 2, .. }),
        "{error}"
    );
}

#[test]
fn a_page_or_position_the_profile_lacks_is_refused() {
    let text = read(&steps().file);
    for edit in [
        PageEdit::Rename {
            page: 0,
            name: None,
        },
        PageEdit::Move { page: 1, to: 4 },
        PageEdit::Move { page: 4, to: 1 },
        PageEdit::Enable {
            page: 9,
            enabled: true,
        },
        PageEdit::Delete { page: 4 },
    ] {
        let mut doc = document(&text);
        let error = doc.edit_page(&edit).unwrap_err();
        assert!(
            matches!(error, EditError::NoPage { .. }),
            "{edit:?}: {error}"
        );
        assert_eq!(doc.to_string(), text);
    }
}

#[test]
fn a_page_moves_past_several_with_its_keys_and_comments() {
    let text = read(&steps().file);
    let mut doc = document(&text);
    doc.edit_page(&PageEdit::Move { page: 1, to: 3 }).unwrap();
    let names: Vec<_> = doc
        .profile()
        .pages
        .iter()
        .map(|page| page.name.as_deref())
        .collect();
    assert_eq!(names, [Some("edit"), Some("debug"), Some("context")]);
    assert!(doc.to_string().contains(
        "[[pages]]\nname = \"context\"\n\n[pages.keys.1]                # a new shell\n"
    ));
    doc.edit_page(&PageEdit::Move { page: 3, to: 1 }).unwrap();
    assert_eq!(doc.to_string(), text);
}

#[test]
fn a_page_is_added_to_a_file_without_pages_and_a_named_one_has_its_name() {
    let text = "schema_version = 1\n\n[profile]\nid = \"x\"\nname = \"X\"\n";
    let mut doc = document(text);
    let done = doc
        .edit_page(&PageEdit::Add {
            name: Some("tools".into()),
        })
        .unwrap();
    assert_eq!(done.page, Some(1));
    assert_eq!(
        doc.to_string(),
        format!("{text}\n[[pages]]\nname = \"tools\"\n")
    );
}

#[test]
fn an_inline_page_array_is_renamed_and_switched_but_not_reordered() {
    let text = "schema_version = 1\npages = [{ name = \"a\" }, { name = \"b\" }]\n\
                [profile]\nid = \"x\"\nname = \"X\"\n";
    let mut doc = document(text);
    doc.edit_page(&PageEdit::Enable {
        page: 2,
        enabled: false,
    })
    .unwrap();
    assert!(!doc.profile().pages[1].enabled);
    let error = doc
        .edit_page(&PageEdit::Move { page: 2, to: 1 })
        .unwrap_err();
    assert!(matches!(error, EditError::Shape { .. }), "{error}");
}
