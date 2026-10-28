//! Round-trip editing (spec §7.1, ADR-0004) against the fixtures in
//! `tests/fixtures/edit/`: every case applies `<case>.json` to
//! `before.toml` and must give `<case>.toml` byte for byte, with every line
//! outside the edited key unchanged.

use std::ops::Range;
use std::path::{Path, PathBuf};

use keyvo_core::edit::{EditError, KeyFields, ProfileDocument};
use keyvo_core::profile::{KeySlot, Profile};
use serde::Deserialize;

/// Every case in the directory; a test fails when a file is missing here.
const CASES: &[&str] = &[
    "label",
    "label-removed",
    "color-added",
    "color-changed",
    "action-subtable",
    "action-compact",
    "action-type",
    "action-launch",
    "action-launch-window",
    "action-launch-subtable",
    "action-removed",
    "key-added",
    "second-page",
];

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/edit")
}

fn read(name: &str) -> String {
    std::fs::read_to_string(fixtures().join(name)).unwrap()
}

#[derive(Deserialize)]
struct Case {
    page: usize,
    key: u8,
    fields: KeyFields,
}

fn case(name: &str) -> Case {
    serde_json::from_str(&read(&format!("{name}.json"))).unwrap()
}

fn document(text: &str) -> ProfileDocument {
    ProfileDocument::parse(text, Path::new("edits.toml")).unwrap()
}

/// The lines of key `key` on page `page` (counted from 1): its header and
/// every line up to the last value before the next header, with its
/// sub-tables (`[pages.keys.N.action]`). Empty, at the end of the page,
/// when the page has no such key.
fn key_lines(text: &str, page: usize, key: u8) -> Range<usize> {
    let lines: Vec<&str> = text.lines().collect();
    let header = |line: &str| line.trim_start().starts_with('[');
    let pages: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i].trim_start().starts_with("[[pages]]"))
        .collect();
    let start = pages[page - 1];
    let end = pages.get(page).copied().unwrap_or(lines.len());
    let own = |line: &str| {
        let line = line.trim_start();
        line.starts_with(&format!("[pages.keys.{key}]"))
            || line.starts_with(&format!("[pages.keys.{key}."))
    };
    let mut range: Option<Range<usize>> = None;
    let mut inside = false;
    for (i, line) in lines.iter().enumerate().take(end).skip(start) {
        let line = line.trim();
        if header(line) {
            inside = own(line);
        }
        let value = !line.is_empty() && !line.starts_with('#');
        if inside && value {
            let first = range.as_ref().map_or(i, |r| r.start);
            range = Some(first..i + 1);
        }
    }
    range.unwrap_or_else(|| {
        // The end of the page's last value: where a new key goes.
        let last = (start..end)
            .rev()
            .find(|&i| {
                let line = lines[i].trim();
                !line.is_empty() && !line.starts_with('#')
            })
            .map_or(end, |i| i + 1);
        last..last
    })
}

/// Lines of `before` and `after` that differ, after the common head and
/// tail. Blank lines at either end are left out: which of two equal blank
/// lines a removal took is not visible in the bytes.
fn changed(before: &str, after: &str) -> (Range<usize>, Range<usize>) {
    let b: Vec<&str> = before.lines().collect();
    let a: Vec<&str> = after.lines().collect();
    let head = b.iter().zip(&a).take_while(|(x, y)| x == y).count();
    let tail = b[head..]
        .iter()
        .rev()
        .zip(a[head..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let trim = |lines: &[&str], mut range: Range<usize>| {
        while range.start < range.end && lines[range.start].trim().is_empty() {
            range.start += 1;
        }
        while range.start < range.end && lines[range.end - 1].trim().is_empty() {
            range.end -= 1;
        }
        range
    };
    (
        trim(&b, head..b.len() - tail),
        trim(&a, head..a.len() - tail),
    )
}

/// Whether the changed lines `inner` are all in `outer`. No changed lines
/// is a pure insertion or removal; the other file's range places it.
fn within(inner: &Range<usize>, outer: &Range<usize>) -> bool {
    inner.is_empty() || (outer.start <= inner.start && inner.end <= outer.end)
}

#[test]
fn every_case_gives_its_expected_file_byte_for_byte() {
    let before = read("before.toml");
    for name in CASES {
        let case = case(name);
        let slot = KeySlot::new(case.key).unwrap();
        let mut doc = document(&before);
        doc.set_key(case.page, slot, &case.fields).unwrap();
        let after = doc.to_string();
        assert_eq!(after, read(&format!("{name}.toml")), "case {name}");
        assert_eq!(
            doc.key(case.page, slot).unwrap(),
            case.fields,
            "case {name}"
        );
    }
}

#[test]
fn every_case_changes_only_lines_of_the_edited_key() {
    let before = read("before.toml");
    for name in CASES {
        let case = case(name);
        let after = read(&format!("{name}.toml"));
        let (old, new) = changed(&before, &after);
        let old_key = key_lines(&before, case.page, case.key);
        let new_key = key_lines(&after, case.page, case.key);
        assert!(
            within(&old, &old_key),
            "case {name}: lines {old:?} of before.toml changed, key is {old_key:?}"
        );
        assert!(
            within(&new, &new_key),
            "case {name}: lines {new:?} of {name}.toml changed, key is {new_key:?}"
        );
    }
}

#[test]
fn every_case_keeps_the_other_keys_and_the_profile() {
    let before = read("before.toml");
    let parsed = |text: &str| Profile::from_toml(text, Path::new("edits.toml")).unwrap();
    let old = parsed(&before);
    for name in CASES {
        let case = case(name);
        let slot = KeySlot::new(case.key).unwrap();
        let mut new = parsed(&read(&format!("{name}.toml")));
        assert_eq!(new.profile, old.profile, "case {name}");
        let mut old = old.clone();
        old.pages[case.page - 1].keys.remove(&slot);
        let edited = new.pages[case.page - 1]
            .keys
            .remove(&slot)
            .unwrap_or_default();
        assert_eq!(new, old, "case {name}");
        assert_eq!(KeyFields::of(&edited), case.fields, "case {name}");
    }
}

#[test]
fn every_fixture_file_is_a_listed_case() {
    for entry in std::fs::read_dir(fixtures()).unwrap() {
        let path = entry.unwrap().path();
        let stem = path.file_stem().unwrap().to_str().unwrap();
        if stem == "before" || path.extension().is_some_and(|e| e == "md") {
            continue;
        }
        assert!(CASES.contains(&stem), "{} is not in CASES", path.display());
    }
}

#[test]
fn an_unedited_document_is_written_back_byte_for_byte() {
    let profiles = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/profiles/valid");
    let mut texts = vec![read("before.toml")];
    for entry in std::fs::read_dir(profiles).unwrap() {
        texts.push(std::fs::read_to_string(entry.unwrap().path()).unwrap());
    }
    for text in texts {
        assert_eq!(document(&text).to_string(), text);
    }
}

#[test]
fn an_edit_to_the_same_fields_changes_nothing() {
    let before = read("before.toml");
    let mut doc = document(&before);
    let page = 1;
    for n in 1..=9 {
        let slot = KeySlot::new(n).unwrap();
        let fields = doc.key(page, slot).unwrap();
        assert_eq!(doc.set_key(page, slot, &fields).unwrap(), fields);
    }
    assert_eq!(doc.to_string(), before);
}

#[test]
fn inline_pages_and_keys_stay_inline() {
    let text = "schema_version = 1\n\
                pages = [{ name = \"context\", keys = { 1 = { label = \"A\" } } }, { name = \"general\" }]\n\
                [profile]\n\
                id = \"edits\"\n\
                name = \"Inline\"\n";
    let mut doc = document(text);
    let label = |s: &str| KeyFields {
        label: Some(s.to_owned()),
        ..KeyFields::default()
    };
    doc.set_key(1, KeySlot::new(1).unwrap(), &label("B"))
        .unwrap();
    doc.set_key(1, KeySlot::new(2).unwrap(), &label("C"))
        .unwrap();
    doc.set_key(2, KeySlot::new(9).unwrap(), &label("D"))
        .unwrap();
    assert_eq!(
        doc.to_string(),
        "schema_version = 1\n\
         pages = [{ name = \"context\", keys = { 1 = { label = \"B\" } , 2 = { label = \"C\" } } }, { name = \"general\" , keys = { 9 = { label = \"D\" } } }]\n\
         [profile]\n\
         id = \"edits\"\n\
         name = \"Inline\"\n"
    );
    assert_eq!(doc.profile().pages[1].keys.len(), 1);
}

#[test]
fn a_page_past_the_last_is_an_error_naming_the_count() {
    let mut doc = document(&read("before.toml"));
    let slot = KeySlot::new(1).unwrap();
    let error = doc.set_key(3, slot, &KeyFields::default()).unwrap_err();
    assert!(
        matches!(
            error,
            EditError::NoPage {
                page: 3,
                pages: 2,
                ..
            }
        ),
        "{error:?}"
    );
    assert_eq!(
        error.to_string(),
        "edits.toml: profile `edits` has no page 3; it has 2"
    );
    assert!(matches!(
        doc.key(0, slot),
        Err(EditError::NoPage { page: 0, .. })
    ));
}

#[test]
fn an_invalid_file_is_a_diagnostic_with_its_line() {
    let text = "schema_version = 1\n[profile]\nid = \"edits\"\nname = 7\n";
    let Err(diagnostic) = ProfileDocument::parse(text, Path::new("edits.toml")) else {
        panic!("an invalid profile parsed");
    };
    assert_eq!(diagnostic.position.map(|p| p.line), Some(4));
}
