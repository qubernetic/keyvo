//! The profiles for the GUI's Profiles screen (`docs/design/gui-ux.md`
//! §3.2): what the list column, the profile header, the match rules and the
//! page tabs show, and the file "Open in editor" opens. The GUI orders the
//! list itself (global first, then by priority).

use std::path::{Path, PathBuf};
use std::process::Command;

use keyvo_core::layout::Profiles;
use keyvo_core::profile::Profile;
use serde::Serialize;

/// The window match rules of a profile as written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MatchSummary {
    /// Window classes.
    pub class: Vec<String>,
    /// The title regular expression.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// One profile of the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileSummary {
    /// The profile id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// The accent as `#rrggbb`; none when the profile has no accent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    /// Higher wins when several profiles match.
    pub priority: i32,
    /// The match rules.
    pub r#match: MatchSummary,
    /// The name of each page, in order; page numbers count from 1.
    pub pages: Vec<Option<String>>,
    /// The profile file.
    pub path: PathBuf,
    /// Whether the file exists; false for the built-in global profile until
    /// its first edit.
    pub file: bool,
}

impl ProfileSummary {
    /// The summary of `profile`, its file under `config_dir`.
    #[must_use]
    pub fn of(profile: &Profile, config_dir: &Path) -> Self {
        let info = &profile.profile;
        let path = config_dir
            .join("profiles")
            .join(format!("{}.toml", info.id));
        Self {
            id: info.id.clone(),
            name: info.name.clone(),
            accent: info.accent.map(|color| color.to_string()),
            priority: info.priority,
            r#match: MatchSummary {
                class: info.r#match.class.clone(),
                title: info.r#match.title.as_ref().map(|t| t.as_str().to_owned()),
            },
            pages: profile.pages.iter().map(|page| page.name.clone()).collect(),
            file: path.is_file(),
            path,
        }
    }
}

/// The command that opens `path` in the desktop's application for it,
/// for "Open in editor" (§3.2). The external editor command of Settings
/// (§3.7) replaces it once Settings exist.
#[must_use]
pub fn opener(path: &Path) -> Command {
    let mut command = Command::new("xdg-open");
    command.arg(path);
    command
}

/// Every profile of `profiles` in id order, their files under `config_dir`.
#[must_use]
pub fn list(profiles: &Profiles, config_dir: &Path) -> Vec<ProfileSummary> {
    profiles
        .iter()
        .map(|profile| ProfileSummary::of(profile, config_dir))
        .collect()
}
