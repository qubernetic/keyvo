//! The key editor on a configuration directory on disk, driven by the
//! shared steps in `tests/protocol-fixtures/editor/`, which the GUI tests
//! replay too: every step writes its expected file byte for byte after a
//! snapshot of the one before, and undoing the steps restores the files.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use keyvo_client::editor::{Editor, EditorError, Problem};
use keyvo_core::edit::{BUILTIN_GLOBAL, EditError, KeyFields};
use keyvo_core::profile::KeySlot;
use serde::Deserialize;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/protocol-fixtures/editor")
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(fixtures().join(name)).unwrap()
}

#[derive(Deserialize)]
struct Fixture {
    profile: String,
    page: usize,
    key: u8,
    file: String,
    path: PathBuf,
    fields: KeyFields,
    steps: Vec<Step>,
}

#[derive(Deserialize)]
struct Step {
    fields: KeyFields,
    file: String,
}

fn steps() -> Fixture {
    serde_json::from_str(&fixture("steps.json")).unwrap()
}

/// A configuration directory with `profiles` as `(id, text)` and a state
/// directory beside it.
struct Dirs {
    root: tempfile::TempDir,
}

impl Dirs {
    fn new(profiles: &[(&str, &str)]) -> Self {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("config/profiles")).unwrap();
        for (id, text) in profiles {
            let path = root.path().join(format!("config/profiles/{id}.toml"));
            std::fs::write(path, text).unwrap();
        }
        Self { root }
    }

    fn editor(&self) -> Editor {
        Editor::new(self.config(), &self.root.path().join("state"))
    }

    fn config(&self) -> PathBuf {
        self.root.path().join("config")
    }

    fn profile(&self, id: &str) -> String {
        std::fs::read_to_string(self.config().join(format!("profiles/{id}.toml"))).unwrap()
    }

    fn snapshots(&self) -> Vec<PathBuf> {
        let dir = self.root.path().join("state/snapshots");
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut paths: Vec<PathBuf> = entries.map(|e| e.unwrap().path()).collect();
        paths.sort();
        paths
    }
}

fn at(ms: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_millis(ms)
}

fn slot(n: u8) -> KeySlot {
    KeySlot::new(n).unwrap()
}

#[test]
fn the_shared_steps_write_their_files_and_undo_restores_them() {
    let steps = steps();
    let dirs = Dirs::new(&[(&steps.profile, &fixture(&steps.file))]);
    let editor = dirs.editor();
    let key = slot(steps.key);

    let view = editor.key(&steps.profile, steps.page, key).unwrap();
    assert_eq!(view.path, dirs.config().join(&steps.path));
    assert_eq!(view.fields, steps.fields);

    let mut files = vec![steps.file.clone()];
    let mut before = steps.fields.clone();
    for (n, step) in steps.steps.iter().enumerate() {
        let ms = u64::try_from(n).unwrap();
        let edited = editor
            .set_key(&steps.profile, steps.page, key, &step.fields, at(ms))
            .unwrap();
        assert_eq!(edited.before, before, "step {n}");
        assert_eq!(edited.fields, step.fields, "step {n}");
        assert_eq!(
            dirs.profile(&steps.profile),
            fixture(&step.file),
            "step {n}"
        );
        let snapshot = edited.snapshot.unwrap();
        let previous = files.last().unwrap();
        assert_eq!(
            std::fs::read_to_string(snapshot).unwrap(),
            fixture(previous)
        );
        files.push(step.file.clone());
        before = step.fields.clone();
    }

    // Undo replays each step's `before` through the same write path.
    files.pop();
    for (n, step) in steps.steps.iter().enumerate().rev() {
        let restore = if n == 0 {
            steps.fields.clone()
        } else {
            steps.steps[n - 1].fields.clone()
        };
        let edited = editor
            .set_key(&steps.profile, steps.page, key, &restore, at(100))
            .unwrap();
        assert_eq!(edited.before, step.fields);
        let expected = files.pop().unwrap();
        assert_eq!(dirs.profile(&steps.profile), fixture(&expected), "undo {n}");
    }
    assert_eq!(dirs.snapshots().len(), 2 * steps.steps.len());
}

#[test]
fn the_same_fields_write_nothing_and_take_no_snapshot() {
    let steps = steps();
    let dirs = Dirs::new(&[(&steps.profile, &fixture(&steps.file))]);
    let edited = dirs
        .editor()
        .set_key(&steps.profile, 1, slot(2), &steps.fields, at(1))
        .unwrap();
    assert_eq!(edited.snapshot, None);
    assert_eq!(dirs.snapshots(), Vec::<PathBuf>::new());
    assert_eq!(dirs.profile(&steps.profile), fixture(&steps.file));
}

#[test]
fn a_missing_global_profile_is_created_from_the_builtin_one() {
    let dirs = Dirs::new(&[]);
    let editor = dirs.editor();
    assert_eq!(
        editor.key("global", 1, slot(3)).unwrap().fields,
        KeyFields::default()
    );
    let fields = KeyFields {
        label: Some("Build".into()),
        ..KeyFields::default()
    };
    let edited = editor
        .set_key("global", 1, slot(3), &fields, at(1))
        .unwrap();
    assert_eq!(edited.snapshot, None);
    assert_eq!(
        dirs.profile("global"),
        format!("{BUILTIN_GLOBAL}\n[pages.keys.3]\nlabel = \"Build\"\n")
    );
}

#[test]
fn the_first_global_edit_creates_the_missing_directories() {
    let fields = KeyFields {
        label: Some("Hi".into()),
        ..KeyFields::default()
    };
    let expected = format!("{BUILTIN_GLOBAL}\n[pages.keys.1]\nlabel = \"Hi\"\n");
    for config in ["config", "config/nested"] {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("config")).unwrap();
        let config = root.path().join(config);
        let editor = Editor::new(config.clone(), &root.path().join("state"));
        let edited = editor
            .set_key("global", 1, slot(1), &fields, at(1))
            .unwrap();
        assert_eq!(edited.snapshot, None);
        let path = config.join("profiles/global.toml");
        assert_eq!(edited.path, path);
        assert_eq!(std::fs::read_to_string(path).unwrap(), expected);
    }
}

#[test]
fn a_missing_app_profile_is_an_error() {
    let dirs = Dirs::new(&[]);
    let error = dirs.editor().key("konsole", 1, slot(1)).unwrap_err();
    assert!(matches!(error, EditorError::NoProfile { .. }), "{error:?}");
    assert!(
        error.to_string().starts_with("no profile `konsole`: "),
        "{error}"
    );
}

#[test]
fn an_id_that_is_not_a_file_name_is_refused() {
    let dirs = Dirs::new(&[]);
    for id in ["../config", "a/b", ""] {
        let error = dirs.editor().key(id, 1, slot(1)).unwrap_err();
        assert!(matches!(error, EditorError::BadId(_)), "{id}: {error:?}");
    }
}

#[test]
fn an_invalid_file_is_a_problem_with_file_line_and_column() {
    let text = "schema_version = 1\n[profile]\nid = \"terminal\"\nname = 7\n";
    let dirs = Dirs::new(&[("terminal", text)]);
    let editor = dirs.editor();
    let error = editor
        .set_key("terminal", 1, slot(1), &KeyFields::default(), at(1))
        .unwrap_err();
    let problem = Problem::from(&error);
    assert_eq!(
        problem.path.as_deref(),
        Some(dirs.config().join("profiles/terminal.toml").as_path())
    );
    assert_eq!((problem.line, problem.column), (Some(4), Some(8)));
    assert!(
        !problem.message.contains("terminal.toml"),
        "{}",
        problem.message
    );
    // The broken file is left as it was.
    assert_eq!(dirs.profile("terminal"), text);
    assert_eq!(dirs.snapshots(), Vec::<PathBuf>::new());
}

#[test]
fn a_page_past_the_last_is_a_problem_with_the_whole_message() {
    let steps = steps();
    let dirs = Dirs::new(&[(&steps.profile, &fixture(&steps.file))]);
    let error = dirs.editor().key(&steps.profile, 2, slot(1)).unwrap_err();
    assert!(matches!(error, EditorError::Edit(EditError::NoPage { .. })));
    let problem = Problem::from(&error);
    assert_eq!(problem.path, None);
    assert_eq!(problem.message, error.to_string());
    assert_eq!(
        serde_json::to_value(&problem).unwrap(),
        serde_json::json!({ "message": error.to_string() })
    );
}

#[test]
fn a_problem_serializes_its_position() {
    let problem = Problem {
        message: "bad".into(),
        path: Some("/c/profiles/t.toml".into()),
        line: Some(4),
        column: Some(8),
    };
    assert_eq!(
        serde_json::to_value(&problem).unwrap(),
        serde_json::json!({ "message": "bad", "path": "/c/profiles/t.toml", "line": 4, "column": 8 })
    );
}

#[test]
fn an_unreadable_file_is_an_io_error_naming_it() {
    let dirs = Dirs::new(&[]);
    std::fs::create_dir(dirs.config().join("profiles/terminal.toml")).unwrap();
    let error = dirs.editor().key("terminal", 1, slot(1)).unwrap_err();
    assert!(matches!(error, EditorError::Io { .. }), "{error:?}");
    assert!(error.to_string().contains("terminal.toml"), "{error}");
}
