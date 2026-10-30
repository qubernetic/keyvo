//! The key editor on a configuration directory on disk, driven by the
//! shared steps in `tests/protocol-fixtures/editor/`, which the GUI tests
//! replay too: every step writes its expected file byte for byte after a
//! snapshot of the one before, and undoing the steps restores the files.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use keyvo_client::editor::{ControlFields, Editor, EditorError, Problem};
use keyvo_core::chord::Chord;
use keyvo_core::dialpad::{ButtonSlot, Control};
use keyvo_core::edit::{
    BUILTIN_GLOBAL, ButtonFields, EditError, KeyFields, PageEdit, ProfileFields, RotaryFields,
};
use keyvo_core::profile::{Action, KeySlot};
use keyvo_core::recipe::{self, Recipe};
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
    line: usize,
    steps: Vec<Step>,
}

#[derive(Deserialize)]
struct Step {
    fields: KeyFields,
    file: String,
    line: usize,
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
    assert_eq!(view.line, Some(steps.line));

    let mut files = vec![steps.file.clone()];
    let mut before = steps.fields.clone();
    for (n, step) in steps.steps.iter().enumerate() {
        let ms = u64::try_from(n).unwrap();
        let edited = editor
            .set_key(&steps.profile, steps.page, key, &step.fields, at(ms))
            .unwrap();
        assert_eq!(edited.before, before, "step {n}");
        assert_eq!(edited.fields, step.fields, "step {n}");
        assert_eq!(edited.line, Some(step.line), "step {n}");
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

/// `recipe.json`: a bundled recipe applied to a key of `terminal.toml`.
#[derive(Deserialize)]
struct RecipeFixture {
    profile: String,
    page: usize,
    key: u8,
    file: String,
    path: PathBuf,
    recipe: String,
    variant: String,
    before: KeyFields,
    fields: KeyFields,
    expected: String,
    line: usize,
}

fn recipe_fixture() -> RecipeFixture {
    serde_json::from_str(&fixture("recipe.json")).unwrap()
}

fn bundled(id: &str) -> Recipe {
    recipe::bundled()
        .recipes
        .into_iter()
        .find(|r| r.id == id)
        .unwrap()
}

#[test]
fn applying_the_shared_recipe_writes_its_file_and_undo_restores_it() {
    let f = recipe_fixture();
    let dirs = Dirs::new(&[(&f.profile, &fixture(&f.file))]);
    let editor = dirs.editor();
    let recipe = bundled(&f.recipe);

    let edited = editor
        .apply_recipe(&f.profile, f.page, slot(f.key), &recipe, &f.variant, at(1))
        .unwrap();
    assert_eq!(edited.path, dirs.config().join(&f.path));
    assert_eq!(edited.before, f.before);
    assert_eq!(edited.fields, f.fields);
    assert_eq!(edited.line, Some(f.line));
    assert_eq!(dirs.profile(&f.profile), fixture(&f.expected));
    assert_eq!(
        std::fs::read_to_string(edited.snapshot.unwrap()).unwrap(),
        fixture(&f.file)
    );

    // Undo writes `before` back; the key's emptied table stays, as after
    // any edit that clears every field.
    editor
        .set_key(&f.profile, f.page, slot(f.key), &edited.before, at(2))
        .unwrap();
    let view = editor.key(&f.profile, f.page, slot(f.key)).unwrap();
    assert_eq!(view.fields, f.before);
}

#[test]
fn an_unknown_variant_writes_nothing() {
    let f = recipe_fixture();
    let dirs = Dirs::new(&[(&f.profile, &fixture(&f.file))]);
    let error = dirs
        .editor()
        .apply_recipe(
            &f.profile,
            f.page,
            slot(f.key),
            &bundled(&f.recipe),
            "nope",
            at(1),
        )
        .unwrap_err();
    assert!(
        matches!(&error, EditorError::NoVariant { recipe, variant } if recipe == "spectacle" && variant == "nope"),
        "{error:?}"
    );
    assert_eq!(
        error.to_string(),
        "recipe `spectacle` has no variant `nope`"
    );
    assert_eq!(dirs.profile(&f.profile), fixture(&f.file));
    assert!(dirs.snapshots().is_empty());
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
    let view = editor.key("global", 1, slot(3)).unwrap();
    assert_eq!(view.fields, KeyFields::default());
    assert_eq!(view.line, None);
    let fields = KeyFields {
        label: Some("Build".into()),
        ..KeyFields::default()
    };
    let edited = editor
        .set_key("global", 1, slot(3), &fields, at(1))
        .unwrap();
    assert_eq!(edited.snapshot, None);
    // The key added at the end of the page has a line after the write.
    assert_eq!(edited.line, Some(10));
    assert_eq!(editor.key("global", 1, slot(3)).unwrap().line, Some(10));
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
fn a_key_view_serializes_its_line_only_when_the_key_has_one() {
    let steps = steps();
    let dirs = Dirs::new(&[(&steps.profile, &fixture(&steps.file))]);
    let editor = dirs.editor();
    let defined = serde_json::to_value(editor.key(&steps.profile, 1, slot(2)).unwrap()).unwrap();
    assert_eq!(defined["line"], serde_json::json!(steps.line));
    let blank = serde_json::to_value(editor.key(&steps.profile, 1, slot(3)).unwrap()).unwrap();
    assert_eq!(blank.get("line"), None, "{blank}");
}

#[test]
fn a_key_view_shows_the_on_fail_action_it_does_not_write() {
    let text = "schema_version = 1\n[profile]\nid = \"t\"\nname = \"T\"\n[[pages]]\n\
                [pages.keys.1]\nlabel = \"Build\"\naction = { type = \"exec\", cmd = \"make\" }\n\
                on_fail = { type = \"exec\", cmd = \"notify-send failed\" }\n";
    let dirs = Dirs::new(&[("t", text)]);
    let editor = dirs.editor();
    let view = editor.key("t", 1, slot(1)).unwrap();
    assert_eq!(
        view.on_fail,
        Some(keyvo_core::profile::Action::Exec {
            cmd: "notify-send failed".into()
        })
    );
    assert_eq!(
        serde_json::to_value(&view).unwrap()["on_fail"],
        serde_json::json!({ "type": "exec", "cmd": "notify-send failed" })
    );
    let mut fields = view.fields;
    fields.label = Some("Make".into());
    editor
        .set_key("t", 1, slot(1), &fields, SystemTime::UNIX_EPOCH)
        .unwrap();
    assert_eq!(dirs.profile("t"), text.replace("\"Build\"", "\"Make\""));
    let blank = serde_json::to_value(editor.key("t", 1, slot(2)).unwrap()).unwrap();
    assert_eq!(blank.get("on_fail"), None, "{blank}");
}

#[test]
fn an_unreadable_file_is_an_io_error_naming_it() {
    let dirs = Dirs::new(&[]);
    std::fs::create_dir(dirs.config().join("profiles/terminal.toml")).unwrap();
    let error = dirs.editor().key("terminal", 1, slot(1)).unwrap_err();
    assert!(matches!(error, EditorError::Io { .. }), "{error:?}");
    assert!(error.to_string().contains("terminal.toml"), "{error}");
}

#[derive(Deserialize)]
struct ProfileFixture {
    profile: String,
    file: String,
    path: PathBuf,
    fields: ProfileFields,
    steps: Vec<ProfileStep>,
}

#[derive(Deserialize)]
struct ProfileStep {
    fields: ProfileFields,
    file: String,
}

#[test]
fn the_shared_profile_steps_write_their_files_and_undo_restores_them() {
    let steps: ProfileFixture = serde_json::from_str(&fixture("profile.json")).unwrap();
    let dirs = Dirs::new(&[(&steps.profile, &fixture(&steps.file))]);
    let editor = dirs.editor();

    let view = editor.profile(&steps.profile).unwrap();
    assert_eq!(view.path, dirs.config().join(&steps.path));
    assert_eq!(view.fields, steps.fields);

    let mut before = steps.fields.clone();
    let mut previous = steps.file.clone();
    for (n, step) in steps.steps.iter().enumerate() {
        let ms = u64::try_from(n).unwrap();
        let edited = editor
            .set_profile(&steps.profile, &step.fields, at(ms))
            .unwrap();
        assert_eq!(edited.before, before, "step {n}");
        assert_eq!(edited.fields, step.fields, "step {n}");
        assert_eq!(
            dirs.profile(&steps.profile),
            fixture(&step.file),
            "step {n}"
        );
        let snapshot = edited.snapshot.unwrap();
        assert_eq!(
            std::fs::read_to_string(snapshot).unwrap(),
            fixture(&previous)
        );
        before = step.fields.clone();
        previous = step.file.clone();
    }

    let edited = editor
        .set_profile(&steps.profile, &steps.fields, at(100))
        .unwrap();
    assert_eq!(edited.before, before);
    assert_eq!(dirs.profile(&steps.profile), fixture(&steps.file));
}

#[test]
fn the_same_profile_fields_write_nothing() {
    let steps: ProfileFixture = serde_json::from_str(&fixture("profile.json")).unwrap();
    let dirs = Dirs::new(&[(&steps.profile, &fixture(&steps.file))]);
    let edited = dirs
        .editor()
        .set_profile(&steps.profile, &steps.fields, at(1))
        .unwrap();
    assert_eq!(edited.snapshot, None);
    assert_eq!(dirs.snapshots(), Vec::<PathBuf>::new());
}

#[test]
fn renaming_the_missing_global_profile_creates_its_file() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config");
    let editor = Editor::new(config.clone(), &root.path().join("state"));
    let fields = ProfileFields {
        name: "Everywhere".to_owned(),
        accent: None,
        global_pages: true,
    };
    let edited = editor.set_profile("global", &fields, at(1)).unwrap();
    assert_eq!(edited.path, config.join("profiles/global.toml"));
    assert_eq!(
        std::fs::read_to_string(&edited.path).unwrap(),
        BUILTIN_GLOBAL.replace("\"Global\"", "\"Everywhere\"")
    );
}

#[derive(Deserialize)]
struct PageFixture {
    profile: String,
    file: String,
    path: PathBuf,
    steps: Vec<PageStep>,
}

#[derive(Deserialize)]
struct PageStep {
    edit: PageEdit,
    file: String,
    page: Option<usize>,
    undo: Option<PageEdit>,
}

#[test]
fn the_shared_page_steps_write_their_files_and_undo_restores_them() {
    let steps: PageFixture = serde_json::from_str(&fixture("pages.json")).unwrap();
    let dirs = Dirs::new(&[(&steps.profile, &fixture(&steps.file))]);
    let editor = dirs.editor();
    let mut previous = steps.file.clone();
    for (n, step) in steps.steps.iter().enumerate() {
        let ms = u64::try_from(n).unwrap() * 10;
        let edited = editor
            .edit_page(&steps.profile, &step.edit, at(ms))
            .unwrap();
        assert_eq!(edited.path, dirs.config().join(&steps.path), "step {n}");
        assert_eq!(edited.edit, step.edit, "step {n}");
        assert_eq!(edited.page, step.page, "step {n}");
        assert_eq!(edited.undo, step.undo, "step {n}");
        assert_eq!(
            dirs.profile(&steps.profile),
            fixture(&step.file),
            "step {n}"
        );
        let snapshot = edited.snapshot.unwrap();
        assert_eq!(
            std::fs::read_to_string(&snapshot).unwrap(),
            fixture(&previous)
        );
        // The undo: the inverse edit, or for a delete the snapshot.
        if let Some(undo) = &edited.undo {
            editor.edit_page(&steps.profile, undo, at(ms + 1)).unwrap();
        } else {
            let restored = editor
                .restore(&steps.profile, &snapshot, at(ms + 1))
                .unwrap();
            assert!(restored.snapshot.is_some(), "step {n}");
        }
        assert_eq!(dirs.profile(&steps.profile), fixture(&previous), "undo {n}");
        editor
            .edit_page(&steps.profile, &step.edit, at(ms + 2))
            .unwrap();
        previous = step.file.clone();
    }
}

#[test]
fn a_refused_page_edit_writes_nothing() {
    let dirs = Dirs::new(&[("konsole", &fixture("pages.6.toml"))]);
    let error = dirs
        .editor()
        .edit_page("konsole", &PageEdit::Delete { page: 1 }, at(1))
        .unwrap_err();
    assert!(
        matches!(error, EditorError::Edit(EditError::Protected { .. })),
        "{error:?}"
    );
    assert!(error.to_string().contains("only enabled page"), "{error}");
    assert_eq!(dirs.profile("konsole"), fixture("pages.6.toml"));
    assert_eq!(dirs.snapshots(), Vec::<PathBuf>::new());
}

#[test]
fn only_a_snapshot_of_the_same_profile_is_restored() {
    let dirs = Dirs::new(&[
        ("konsole", &fixture("pages.toml")),
        ("terminal", &fixture("terminal.toml")),
    ]);
    let editor = dirs.editor();
    let edited = editor
        .edit_page("konsole", &PageEdit::Delete { page: 3 }, at(1))
        .unwrap();
    let snapshot = edited.snapshot.unwrap();
    let error = editor.restore("terminal", &snapshot, at(2)).unwrap_err();
    assert!(matches!(error, EditorError::Fields(_)), "{error:?}");
    let outside = dirs.config().join("profiles/terminal.toml");
    let error = editor.restore("konsole", &outside, at(3)).unwrap_err();
    assert!(matches!(error, EditorError::Fields(_)), "{error:?}");
    assert_eq!(dirs.profile("terminal"), fixture("terminal.toml"));
}

#[test]
fn the_path_of_a_profile_is_its_file_in_the_profiles_directory() {
    let dirs = Dirs::new(&[]);
    let editor = dirs.editor();
    assert_eq!(
        editor.path("terminal").unwrap(),
        dirs.config().join("profiles/terminal.toml")
    );
    assert!(matches!(editor.path("../x"), Err(EditorError::BadId(_))));
}

#[test]
fn a_dialpad_control_is_read_and_written_with_a_snapshot_and_undone() {
    let global = format!("{BUILTIN_GLOBAL}\n[dialpad]\ndial = {{ cw = \"KEY_VOLUMEUP\" }}\n");
    let dirs = Dirs::new(&[("global", &global)]);
    let editor = dirs.editor();
    let view = editor.control("global", Control::Dial).unwrap();
    assert_eq!(
        serde_json::to_value(&view).unwrap(),
        serde_json::json!({
            "path": dirs.config().join("profiles/global.toml"),
            "fields": { "cw": { "type": "chord", "keys": "KEY_VOLUMEUP" } }
        })
    );
    let fields = ControlFields::Rotary(RotaryFields {
        cw: Some(Action::Chord {
            keys: Chord::parse("KEY_BRIGHTNESSUP").unwrap(),
        }),
        ..RotaryFields::default()
    });
    let edited = editor
        .set_control("global", Control::Dial, &fields, at(1))
        .unwrap();
    assert!(edited.snapshot.is_some());
    assert!(
        dirs.profile("global")
            .contains("dial = { cw = \"KEY_BRIGHTNESSUP\" }")
    );
    editor
        .set_control("global", Control::Dial, &edited.before, at(2))
        .unwrap();
    assert_eq!(dirs.profile("global"), global);
    // A button keeps its on_fail, which the view shows.
    let button = Control::Button(ButtonSlot::new(1).unwrap());
    let fields = ControlFields::Button(ButtonFields {
        label: Some("Mute".into()),
        action: None,
    });
    editor
        .set_control("global", button, &fields, at(3))
        .unwrap();
    assert_eq!(editor.control("global", button).unwrap().fields, fields);
    // The fields of a button do not fit the dial.
    let error = editor
        .set_control("global", Control::Dial, &fields, at(4))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "`dial` takes the fields of a dial or a roller"
    );
}

#[test]
fn control_fields_read_as_the_control_takes_them() {
    let json = serde_json::json!({ "cw": "x" });
    assert!(ControlFields::parse(Control::Dial, json).is_err());
    let json = serde_json::json!({ "step": 3, "label": "Zoom" });
    assert_eq!(
        ControlFields::parse(Control::Roller, json).unwrap(),
        ControlFields::Rotary(RotaryFields {
            step: Some(3),
            label: Some("Zoom".into()),
            ..RotaryFields::default()
        })
    );
    let json = serde_json::json!({ "step": 3 });
    let button = Control::Button(ButtonSlot::new(2).unwrap());
    assert!(ControlFields::parse(button, json).is_err());
}
