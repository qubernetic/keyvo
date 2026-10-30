//! The editors' file side (spec §10, `docs/design/gui-ux.md` §3.2, §3.3,
//! §4.1): a key, a Dialpad control or the profile header (name, accent) of
//! a profile file read and
//! written through `keyvo-core`'s round-trip edit, a snapshot of the file
//! before every write, the write atomic. The daemon's inotify watch then reloads and repaints.

use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use keyvo_core::diagnostic::Diagnostic;
use keyvo_core::dialpad::Control;
use keyvo_core::edit::{
    BUILTIN_GLOBAL, ButtonFields, EditError, KeyFields, PageEdit, ProfileDocument, ProfileFields,
    RotaryFields,
};
use keyvo_core::layout::SNAPSHOTS;
use keyvo_core::profile::{Action, GLOBAL_ID, KeySlot, is_valid_id};
use keyvo_core::recipe::Recipe;
use keyvo_core::store::{self, SNAPSHOTS_KEPT};
use serde::Serialize;

/// Why a key cannot be read or written.
#[derive(Debug, thiserror::Error)]
pub enum EditorError {
    /// The id is not a profile id, so it is not a file name either.
    #[error("`{0}` is not a profile id")]
    BadId(String),
    /// No file for an app profile; only the global one is created.
    #[error("no profile `{id}`: {} does not exist", path.display())]
    NoProfile {
        /// The profile id.
        id: String,
        /// The file that was looked for.
        path: PathBuf,
    },
    /// The file could not be read, snapshotted or written.
    #[error("{}: {source}", path.display())]
    Io {
        /// The file.
        path: PathBuf,
        /// The error.
        source: io::Error,
    },
    /// The file on disk is not a valid profile; it is left as it is.
    #[error(transparent)]
    Invalid(#[from] Diagnostic),
    /// The key cannot be edited.
    #[error(transparent)]
    Edit(#[from] EditError),
    /// The fields do not fit the control.
    #[error("{0}")]
    Fields(String),
    /// The recipe has no variant of that id.
    #[error("recipe `{recipe}` has no variant `{variant}`")]
    NoVariant {
        /// The recipe id.
        recipe: String,
        /// The variant asked for.
        variant: String,
    },
}

/// An [`EditorError`] as the GUI shows it: the message, and the file, line
/// and column when the error has a place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Problem {
    /// What is wrong; without the place when `path` is set.
    pub message: String,
    /// The file the problem is in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    /// The line, from 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// The column in characters, from 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
}

impl Problem {
    /// A problem without a place.
    #[must_use]
    pub fn message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            path: None,
            line: None,
            column: None,
        }
    }
}

impl From<&EditorError> for Problem {
    fn from(error: &EditorError) -> Self {
        match error {
            EditorError::Invalid(diagnostic)
            | EditorError::Edit(EditError::Invalid(diagnostic)) => Self {
                message: diagnostic.message.clone(),
                path: Some(diagnostic.path.clone()),
                line: diagnostic.position.map(|p| p.line),
                column: diagnostic.position.map(|p| p.column),
            },
            other => Self::message(other.to_string()),
        }
    }
}

/// A key as the editor shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KeyView {
    /// The profile file.
    pub path: PathBuf,
    /// The key's fields.
    pub fields: KeyFields,
    /// The line, from 1, on which the key's table starts in the file; none
    /// for a key the page does not define.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// The key's `on_fail` action, shown read-only: the editor never
    /// writes it, a write of the fields keeps it as it is in the file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_fail: Option<Action>,
}

/// What a write did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Edited {
    /// The profile file.
    pub path: PathBuf,
    /// The fields before the write, for undo.
    pub before: KeyFields,
    /// The fields written.
    pub fields: KeyFields,
    /// The line, from 1, on which the key's table starts in the written
    /// file; none when the key is left without a table.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// The copy of the file taken before the write; none when nothing was
    /// written or the file did not exist yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<PathBuf>,
}

/// A profile header as the editor shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileView {
    /// The profile file.
    pub path: PathBuf,
    /// The header's name and accent.
    pub fields: ProfileFields,
}

/// What a write of a profile header did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileEdited {
    /// The profile file.
    pub path: PathBuf,
    /// The fields before the write, for undo.
    pub before: ProfileFields,
    /// The fields written.
    pub fields: ProfileFields,
    /// The copy of the file taken before the write; none when nothing was
    /// written or the file did not exist yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<PathBuf>,
}

/// What a page edit did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageEdited {
    /// The profile file.
    pub path: PathBuf,
    /// The edit.
    pub edit: PageEdit,
    /// The edited page's number after the edit; none after a delete.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<usize>,
    /// The edit that undoes it; none after a delete, whose undo is
    /// [`Editor::restore`] of `snapshot`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub undo: Option<PageEdit>,
    /// The copy of the file taken before the write; none when the file did
    /// not exist yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<PathBuf>,
}

/// What a restore did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Restored {
    /// The profile file.
    pub path: PathBuf,
    /// The copy of the file taken before the write.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<PathBuf>,
}

/// The editable fields of a Dialpad control: the dial's and the roller's,
/// or a button's. Written as the inner fields alone; [`ControlFields::parse`]
/// reads them as the control takes them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum ControlFields {
    /// `cw`, `ccw`, `step`, `label` of the dial or the roller.
    Rotary(RotaryFields),
    /// `label` and `action` of a button.
    Button(ButtonFields),
}

impl ControlFields {
    /// `value` as the fields `control` takes.
    ///
    /// # Errors
    ///
    /// When `value` is not such fields, with the reason.
    pub fn parse(control: Control, value: serde_json::Value) -> Result<Self, String> {
        let invalid = |error: serde_json::Error| format!("invalid {control} fields: {error}");
        if control.roller().is_some() {
            serde_json::from_value(value)
                .map(Self::Rotary)
                .map_err(invalid)
        } else {
            serde_json::from_value(value)
                .map(Self::Button)
                .map_err(invalid)
        }
    }
}

/// A Dialpad control of a profile's `[dialpad]` table as the editor shows
/// it: the profile's own binding, never the global one under it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ControlView {
    /// The profile file.
    pub path: PathBuf,
    /// The control's fields; all absent when the table does not bind it.
    pub fields: ControlFields,
    /// A button's `on_fail`, shown read-only like a key's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_fail: Option<Action>,
}

/// What a write of a Dialpad control did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ControlEdited {
    /// The profile file.
    pub path: PathBuf,
    /// The fields before the write, for undo.
    pub before: ControlFields,
    /// The fields written.
    pub fields: ControlFields,
    /// The copy of the file taken before the write; none when nothing was
    /// written or the file did not exist yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<PathBuf>,
}

/// Reads and writes keys and headers of the profiles in one configuration directory.
#[derive(Debug, Clone)]
pub struct Editor {
    config_dir: PathBuf,
    snapshots: PathBuf,
}

impl Editor {
    /// The editor of the profiles in `config_dir`, its snapshots in
    /// `state_dir` (`keyvo_core::layout::state_dir`).
    #[must_use]
    pub fn new(config_dir: PathBuf, state_dir: &Path) -> Self {
        Self {
            config_dir,
            snapshots: state_dir.join(SNAPSHOTS),
        }
    }

    /// The fields of key `slot` on page `page` (from 1) of profile `id`.
    ///
    /// # Errors
    ///
    /// As [`Editor::set_key`], without the write errors.
    pub fn key(&self, id: &str, page: usize, slot: KeySlot) -> Result<KeyView, EditorError> {
        let (path, doc) = self.open(id)?;
        let fields = doc.key(page, slot)?;
        let line = doc.key_line(page, slot);
        let on_fail = doc.on_fail(page, slot);
        Ok(KeyView {
            path,
            fields,
            line,
            on_fail,
        })
    }

    /// Sets the fields of key `slot` on page `page` of profile `id`: a
    /// snapshot of the file, then the edited text written atomically.
    /// Nothing is written when the fields are already these. A missing
    /// `global.toml` is created from the built-in global profile.
    ///
    /// # Errors
    ///
    /// When the id is not a profile id, an app profile has no file, the
    /// file is unreadable or invalid, the key cannot be edited, or the
    /// snapshot or the write fails.
    pub fn set_key(
        &self,
        id: &str,
        page: usize,
        slot: KeySlot,
        fields: &KeyFields,
        now: SystemTime,
    ) -> Result<Edited, EditorError> {
        let (path, mut doc) = self.open(id)?;
        let before = doc.set_key(page, slot, fields)?;
        let snapshot = if before == *fields {
            None
        } else {
            self.write(&path, &doc, now)?
        };
        Ok(Edited {
            line: doc.key_line(page, slot),
            path,
            before,
            fields: fields.clone(),
            snapshot,
        })
    }

    /// Applies variant `variant` of `recipe` to key `slot` on page `page`
    /// of profile `id`: the variant's action and the recipe's label and
    /// icon are written like [`Editor::set_key`] writes them, the key's
    /// color and states stay. The answer's `before` is the undo.
    ///
    /// # Errors
    ///
    /// [`EditorError::NoVariant`] when the recipe has no such variant,
    /// else as [`Editor::set_key`]; nothing is written after an error.
    pub fn apply_recipe(
        &self,
        id: &str,
        page: usize,
        slot: KeySlot,
        recipe: &Recipe,
        variant: &str,
        now: SystemTime,
    ) -> Result<Edited, EditorError> {
        let (_, doc) = self.open(id)?;
        let fields = recipe
            .apply(variant, &doc.key(page, slot)?)
            .ok_or_else(|| EditorError::NoVariant {
                recipe: recipe.id.clone(),
                variant: variant.to_owned(),
            })?;
        self.set_key(id, page, slot, &fields, now)
    }

    /// The name and accent of profile `id`.
    ///
    /// # Errors
    ///
    /// As [`Editor::set_profile`], without the write errors.
    pub fn profile(&self, id: &str) -> Result<ProfileView, EditorError> {
        let (path, doc) = self.open(id)?;
        let fields = doc.profile_fields();
        Ok(ProfileView { path, fields })
    }

    /// Sets the name and accent of profile `id` like [`Editor::set_key`]
    /// sets a key: a snapshot, then the edited text written atomically;
    /// nothing is written when the fields are already these.
    ///
    /// # Errors
    ///
    /// As [`Editor::set_key`].
    pub fn set_profile(
        &self,
        id: &str,
        fields: &ProfileFields,
        now: SystemTime,
    ) -> Result<ProfileEdited, EditorError> {
        let (path, mut doc) = self.open(id)?;
        let before = doc.set_profile(fields)?;
        let snapshot = if before == *fields {
            None
        } else {
            self.write(&path, &doc, now)?
        };
        Ok(ProfileEdited {
            path,
            before,
            fields: fields.clone(),
            snapshot,
        })
    }

    /// The fields of `control` in the `[dialpad]` table of profile `id`.
    ///
    /// # Errors
    ///
    /// As [`Editor::set_control`], without the write errors.
    pub fn control(&self, id: &str, control: Control) -> Result<ControlView, EditorError> {
        let (path, doc) = self.open(id)?;
        let (fields, on_fail) = match (control.roller(), control) {
            (Some(roller), _) => (ControlFields::Rotary(doc.rotary(roller)), None),
            (None, Control::Button(slot)) => (
                ControlFields::Button(doc.button(slot)),
                doc.button_on_fail(slot),
            ),
            (None, _) => (ControlFields::Button(ButtonFields::default()), None),
        };
        Ok(ControlView {
            path,
            fields,
            on_fail,
        })
    }

    /// Sets the fields of `control` in the `[dialpad]` table of profile
    /// `id` like [`Editor::set_key`] sets a key's.
    ///
    /// # Errors
    ///
    /// [`EditorError::Fields`] when `fields` are not the control's kind,
    /// else as [`Editor::set_key`].
    pub fn set_control(
        &self,
        id: &str,
        control: Control,
        fields: &ControlFields,
        now: SystemTime,
    ) -> Result<ControlEdited, EditorError> {
        let (path, mut doc) = self.open(id)?;
        let before = match (control.roller(), control, fields) {
            (Some(roller), _, ControlFields::Rotary(fields)) => {
                ControlFields::Rotary(doc.set_rotary(roller, fields)?)
            }
            (None, Control::Button(slot), ControlFields::Button(fields)) => {
                ControlFields::Button(doc.set_button(slot, fields)?)
            }
            (Some(_), ..) => {
                return Err(EditorError::Fields(format!(
                    "`{control}` takes the fields of a dial or a roller"
                )));
            }
            (None, ..) => {
                return Err(EditorError::Fields(format!(
                    "`{control}` takes the fields of a button"
                )));
            }
        };
        let snapshot = if before == *fields {
            None
        } else {
            self.write(&path, &doc, now)?
        };
        Ok(ControlEdited {
            path,
            before,
            fields: fields.clone(),
            snapshot,
        })
    }

    /// Applies `edit` to the pages of profile `id` like [`Editor::set_key`]
    /// writes a key: a snapshot of the file, then the edited text written
    /// atomically.
    ///
    /// # Errors
    ///
    /// As [`Editor::set_key`], and [`EditError::Protected`] for disabling
    /// the context page or deleting the only enabled page.
    pub fn edit_page(
        &self,
        id: &str,
        edit: &PageEdit,
        now: SystemTime,
    ) -> Result<PageEdited, EditorError> {
        let (path, mut doc) = self.open(id)?;
        let done = doc.edit_page(edit)?;
        let snapshot = self.write(&path, &doc, now)?;
        Ok(PageEdited {
            path,
            edit: edit.clone(),
            page: done.page,
            undo: done.undo,
            snapshot,
        })
    }

    /// Writes the text of `snapshot`, a copy this editor took, back as the
    /// file of profile `id`, after a snapshot of the file as it is: the
    /// undo of a page delete.
    ///
    /// # Errors
    ///
    /// [`EditorError::Fields`] when `snapshot` is not in the snapshot
    /// directory, [`EditorError::Io`] when it cannot be read, else as
    /// [`Editor::set_key`]; nothing is written for a snapshot that is not a
    /// valid profile `id`.
    pub fn restore(
        &self,
        id: &str,
        snapshot: &Path,
        now: SystemTime,
    ) -> Result<Restored, EditorError> {
        let path = self.path(id)?;
        if snapshot.parent() != Some(self.snapshots.as_path()) {
            return Err(EditorError::Fields(format!(
                "{} is not a snapshot of the editor",
                snapshot.display()
            )));
        }
        let text = std::fs::read_to_string(snapshot).map_err(|source| EditorError::Io {
            path: snapshot.to_path_buf(),
            source,
        })?;
        let doc = ProfileDocument::parse(&text, &path)?;
        if doc.profile().profile.id != id {
            return Err(EditorError::Fields(format!(
                "{} is not a snapshot of profile `{id}`",
                snapshot.display()
            )));
        }
        let snapshot = self.write(&path, &doc, now)?;
        Ok(Restored { path, snapshot })
    }

    /// The file of profile `id`, whether it exists or not.
    ///
    /// # Errors
    ///
    /// [`EditorError::BadId`] when `id` is not a profile id.
    pub fn path(&self, id: &str) -> Result<PathBuf, EditorError> {
        if !is_valid_id(id) {
            return Err(EditorError::BadId(id.to_owned()));
        }
        Ok(self.config_dir.join("profiles").join(format!("{id}.toml")))
    }

    /// Writes `doc` to `path` after a snapshot of the file; the snapshot's
    /// path, none when the file did not exist yet.
    fn write(
        &self,
        path: &Path,
        doc: &ProfileDocument,
        now: SystemTime,
    ) -> Result<Option<PathBuf>, EditorError> {
        let io = |path: &Path| {
            let path = path.to_path_buf();
            move |source| EditorError::Io { path, source }
        };
        let snapshot = store::snapshot(path, &self.snapshots, now, SNAPSHOTS_KEPT)
            .map_err(io(&self.snapshots))?;
        // The first edit of the built-in global profile on a fresh install,
        // where neither the configuration directory nor `profiles/` exists.
        let profiles = self.config_dir.join("profiles");
        std::fs::create_dir_all(&profiles).map_err(io(&profiles))?;
        store::write(path, &doc.to_string()).map_err(io(path))?;
        Ok(snapshot)
    }

    /// The file of profile `id` and its document.
    fn open(&self, id: &str) -> Result<(PathBuf, ProfileDocument), EditorError> {
        let path = self.path(id)?;
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound && id == GLOBAL_ID => {
                BUILTIN_GLOBAL.to_owned()
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(EditorError::NoProfile {
                    id: id.to_owned(),
                    path,
                });
            }
            Err(source) => return Err(EditorError::Io { path, source }),
        };
        let doc = ProfileDocument::parse(&text, &path)?;
        Ok((path, doc))
    }
}
