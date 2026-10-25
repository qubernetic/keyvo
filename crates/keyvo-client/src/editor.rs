//! The key editor's file side (spec §10, `docs/design/gui-ux.md` §4.1):
//! a key of a profile file read and written through `keyvo-core`'s
//! round-trip edit, a snapshot of the file before every write, the write
//! atomic. The daemon's inotify watch then reloads and repaints.

use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use keyvo_core::diagnostic::Diagnostic;
use keyvo_core::edit::{BUILTIN_GLOBAL, EditError, KeyFields, ProfileDocument};
use keyvo_core::layout::SNAPSHOTS;
use keyvo_core::profile::{GLOBAL_ID, KeySlot, is_valid_id};
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
    /// The copy of the file taken before the write; none when nothing was
    /// written or the file did not exist yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<PathBuf>,
}

/// Reads and writes keys of the profiles in one configuration directory.
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
        Ok(KeyView { path, fields })
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
        let mut edited = Edited {
            path,
            before,
            fields: fields.clone(),
            snapshot: None,
        };
        if edited.before == edited.fields {
            return Ok(edited);
        }
        let io = |path: &Path| {
            let path = path.to_path_buf();
            move |source| EditorError::Io { path, source }
        };
        edited.snapshot = store::snapshot(&edited.path, &self.snapshots, now, SNAPSHOTS_KEPT)
            .map_err(io(&self.snapshots))?;
        // The first edit of the built-in global profile on a fresh install,
        // where neither the configuration directory nor `profiles/` exists.
        let profiles = self.config_dir.join("profiles");
        std::fs::create_dir_all(&profiles).map_err(io(&profiles))?;
        store::write(&edited.path, &doc.to_string()).map_err(io(&edited.path))?;
        Ok(edited)
    }

    /// The file of profile `id` and its document.
    fn open(&self, id: &str) -> Result<(PathBuf, ProfileDocument), EditorError> {
        if !is_valid_id(id) {
            return Err(EditorError::BadId(id.to_owned()));
        }
        let path = self.config_dir.join("profiles").join(format!("{id}.toml"));
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
