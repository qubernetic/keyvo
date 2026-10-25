//! The configuration directory (spec §7.1, pre-alpha subset):
//!
//! ```text
//! ~/.config/keyvo/
//! ├── config.toml          # [timing], [render]; optional
//! └── profiles/<id>.toml   # one profile per file; <id> is the profile id
//! ```
//!
//! A missing directory, `config.toml` or `profiles/` is not an error; the
//! global profile always exists, built in when `profiles/global.toml` does
//! not.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::config::Config;
use crate::diagnostic::Diagnostic;
use crate::focus::WindowContext;
use crate::profile::{GLOBAL_ID, Profile};

/// The configuration directory for these environment values:
/// `$XDG_CONFIG_HOME/keyvo` when it is an absolute path, else
/// `$HOME/.config/keyvo`; `None` without either.
#[must_use]
pub fn config_dir(xdg_config_home: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    absolute(xdg_config_home)
        .map(Path::to_path_buf)
        .or_else(|| absolute(home).map(|h| h.join(".config")))
        .map(|base| base.join("keyvo"))
}

/// `value` as a path when it is an absolute one (XDG Base Directory spec:
/// an empty or relative value is ignored).
fn absolute(value: Option<&OsStr>) -> Option<&Path> {
    value.map(Path::new).filter(|p| p.is_absolute())
}

/// The state directory (spec §7.1), home of the snapshots the GUI takes
/// before every write: `$XDG_STATE_HOME/keyvo` when it is an absolute
/// path, else `$HOME/.local/state/keyvo`; `None` without either.
#[must_use]
pub fn state_dir(xdg_state_home: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    absolute(xdg_state_home)
        .map(Path::to_path_buf)
        .or_else(|| absolute(home).map(|h| h.join(".local/state")))
        .map(|base| base.join("keyvo"))
}

/// The snapshot directory inside [`state_dir`].
pub const SNAPSHOTS: &str = "snapshots";

/// The daemon's runtime directory, home of the socket (spec §8) and the
/// `KWin` script (spec §6.2): `$XDG_RUNTIME_DIR/keyvo` when that is an
/// absolute path, else `keyvo-<uid>` in `temp_dir`.
#[must_use]
pub fn runtime_dir(xdg_runtime_dir: Option<&OsStr>, temp_dir: &Path, uid: u32) -> PathBuf {
    match xdg_runtime_dir.map(Path::new).filter(|p| p.is_absolute()) {
        Some(dir) => dir.join("keyvo"),
        None => temp_dir.join(format!("keyvo-{uid}")),
    }
}

/// Everything read from one configuration directory.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    /// `config.toml`, or the defaults when it does not exist.
    pub config: Config,
    /// The profiles, the global one always among them.
    pub profiles: Profiles,
}

/// The profiles by id; the global profile is always present.
#[derive(Debug, Clone, PartialEq)]
pub struct Profiles(BTreeMap<String, Profile>);

impl Profiles {
    /// The profiles from `by_id`, with the built-in global profile added when
    /// `by_id` has none.
    #[must_use]
    pub fn new(mut by_id: BTreeMap<String, Profile>) -> Self {
        by_id
            .entry(GLOBAL_ID.to_owned())
            .or_insert_with(Profile::builtin_global);
        Self(by_id)
    }

    /// The global profile.
    #[must_use]
    pub fn global(&self) -> &Profile {
        // `new` is the only constructor and always inserts it.
        &self.0[GLOBAL_ID]
    }

    /// The profile with this id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Profile> {
        self.0.get(id)
    }

    /// The profile for the focused `window` (spec §6.2): of the app
    /// profiles whose rules match, the highest `priority`; between equal
    /// priorities one with a title rule beats one without, then the first in
    /// id order. The global profile when none matches.
    #[must_use]
    pub fn select(&self, window: &WindowContext) -> &Profile {
        let mut best: Option<&Profile> = None;
        for profile in self.iter().filter(|p| p.profile.id != GLOBAL_ID) {
            let rules = &profile.profile.r#match;
            if !rules.matches(window) {
                continue;
            }
            let rank = (profile.profile.priority, rules.has_title());
            if best.is_none_or(|b| rank > (b.profile.priority, b.profile.r#match.has_title())) {
                best = Some(profile);
            }
        }
        best.unwrap_or_else(|| self.global())
    }

    /// All profiles in id order.
    pub fn iter(&self) -> impl Iterator<Item = &Profile> {
        self.0.values()
    }

    /// The number of profiles, at least 1.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Always `false`: the global profile is always there.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// One file that could not be loaded.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// The file exists but could not be read.
    #[error("{}: {source}", path.display())]
    Io {
        /// The file.
        path: PathBuf,
        /// The read error.
        source: io::Error,
    },
    /// The file was read but is not valid.
    #[error(transparent)]
    Invalid(#[from] Diagnostic),
}

/// Every file of a directory that could not be loaded, in path order.
#[derive(Debug)]
pub struct LoadErrors(pub Vec<LoadError>);

impl fmt::Display for LoadErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, error) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str("\n")?;
            }
            write!(f, "{error}")?;
        }
        Ok(())
    }
}

impl std::error::Error for LoadErrors {}

/// Loads `config.toml` and `profiles/*.toml` from `dir`. Files that are not
/// `*.toml` in `profiles/` are ignored.
///
/// # Errors
///
/// Every unreadable or invalid file, not only the first, so one run shows
/// all of them.
pub fn load(dir: &Path) -> Result<Loaded, LoadErrors> {
    let mut errors = Vec::new();

    let config_path = dir.join("config.toml");
    let config = match read_optional(&config_path) {
        Ok(Some(text)) => Config::from_toml(&text, &config_path)
            .map_err(|d| errors.push(d.into()))
            .unwrap_or_default(),
        Ok(None) => Config::default(),
        Err(e) => {
            errors.push(e);
            Config::default()
        }
    };

    let mut by_id = BTreeMap::new();
    match profile_paths(&dir.join("profiles")) {
        Ok(paths) => {
            for path in paths {
                match load_profile(&path) {
                    Ok(profile) => {
                        by_id.insert(profile.profile.id.clone(), profile);
                    }
                    Err(e) => errors.push(e),
                }
            }
        }
        Err(e) => errors.push(e),
    }

    if errors.is_empty() {
        Ok(Loaded {
            config,
            profiles: Profiles::new(by_id),
        })
    } else {
        Err(LoadErrors(errors))
    }
}

fn read_optional(path: &Path) -> Result<Option<String>, LoadError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(LoadError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// The `*.toml` paths in `profiles`, sorted; none when it does not exist.
fn profile_paths(profiles: &Path) -> Result<Vec<PathBuf>, LoadError> {
    let io_error = |source| LoadError::Io {
        path: profiles.to_path_buf(),
        source,
    };
    let entries = match std::fs::read_dir(profiles) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io_error(e)),
    };
    let mut paths = Vec::new();
    for entry in entries {
        let path = entry.map_err(io_error)?.path();
        if path.extension() == Some(OsStr::new("toml")) {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn load_profile(path: &Path) -> Result<Profile, LoadError> {
    let text = std::fs::read_to_string(path).map_err(|source| LoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let profile = Profile::from_toml(&text, path)?;
    let stem = path.file_stem().and_then(OsStr::to_str);
    if stem != Some(profile.profile.id.as_str()) {
        return Err(id_mismatch(path, &text, &profile.profile.id).into());
    }
    Ok(profile)
}

/// The diagnostic for an id that is not the file name, pointing at the id.
fn id_mismatch(path: &Path, text: &str, id: &str) -> Diagnostic {
    #[derive(Deserialize)]
    struct File {
        profile: Info,
    }
    #[derive(Deserialize)]
    struct Info {
        id: toml::Spanned<String>,
    }

    // The text already parsed as a profile, so this second pass succeeds.
    let span = toml::from_str::<File>(text)
        .ok()
        .map(|file| file.profile.id.span());
    let file = path.file_name().unwrap_or_default().to_string_lossy();
    Diagnostic::at(
        path,
        text,
        span,
        format!("profile id `{id}` does not match the file name `{file}`"),
    )
}
