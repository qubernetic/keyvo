//! Bundled icons (spec §9). The pre-alpha bundles a subset of lucide 1.61.0
//! (<https://github.com/lucide-icons/lucide>, ISC licence, copied unchanged
//! but for a header comment into `assets/lucide/`, licence text in
//! `assets/lucide/LICENSE`). `phosphor:`, `simple-icons:` and `file:` come
//! with later issues.

/// The lucide icons keyvo bundles, by name.
const LUCIDE: &[(&str, &str)] = &[
    ("bug", include_str!("../../assets/lucide/bug.svg")),
    ("check", include_str!("../../assets/lucide/check.svg")),
    (
        "circle-question-mark",
        include_str!("../../assets/lucide/circle-question-mark.svg"),
    ),
    (
        "git-branch",
        include_str!("../../assets/lucide/git-branch.svg"),
    ),
    (
        "git-commit-horizontal",
        include_str!("../../assets/lucide/git-commit-horizontal.svg"),
    ),
    (
        "git-pull-request",
        include_str!("../../assets/lucide/git-pull-request.svg"),
    ),
    ("hammer", include_str!("../../assets/lucide/hammer.svg")),
    ("play", include_str!("../../assets/lucide/play.svg")),
    (
        "refresh-cw",
        include_str!("../../assets/lucide/refresh-cw.svg"),
    ),
    ("rocket", include_str!("../../assets/lucide/rocket.svg")),
    ("save", include_str!("../../assets/lucide/save.svg")),
    ("search", include_str!("../../assets/lucide/search.svg")),
    ("settings", include_str!("../../assets/lucide/settings.svg")),
    ("square", include_str!("../../assets/lucide/square.svg")),
    ("terminal", include_str!("../../assets/lucide/terminal.svg")),
    ("x", include_str!("../../assets/lucide/x.svg")),
];

/// Deprecated lucide names (from lucide's own metadata) and the icon they
/// now are; the spec's examples use `lucide:git-commit`.
const LUCIDE_ALIASES: &[(&str, &str)] = &[
    ("git-commit", "git-commit-horizontal"),
    ("circle-help", "circle-question-mark"),
    ("help-circle", "circle-question-mark"),
];

/// The icon drawn when a key's icon cannot be found.
pub const FALLBACK: &str = "lucide:circle-question-mark";

/// Why an icon reference does not resolve.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IconError {
    /// No `<source>:` prefix.
    #[error("icon `{0}` has no source; write `lucide:<name>`")]
    NoSource(String),
    /// A source this build does not bundle yet.
    #[error("icon source `{0}:` is not supported yet; use `lucide:`")]
    UnsupportedSource(String),
    /// A lucide name that is not in the bundled subset.
    #[error("no bundled icon `{0}`")]
    UnknownIcon(String),
}

/// The SVG text of the icon `reference` (`lucide:<name>`).
///
/// # Errors
///
/// [`IconError`] when the reference has no source, names a source that is
/// not bundled, or names an icon the subset lacks.
pub fn lookup(reference: &str) -> Result<&'static str, IconError> {
    let Some((source, name)) = reference.split_once(':') else {
        return Err(IconError::NoSource(reference.to_owned()));
    };
    if source != "lucide" {
        return Err(IconError::UnsupportedSource(source.to_owned()));
    }
    let name = LUCIDE_ALIASES
        .iter()
        .find(|(alias, _)| *alias == name)
        .map_or(name, |(_, current)| current);
    LUCIDE
        .iter()
        .find(|(bundled, _)| *bundled == name)
        .map(|(_, svg)| *svg)
        .ok_or_else(|| IconError::UnknownIcon(reference.to_owned()))
}

/// Every bundled icon as a reference, `lucide:<name>`, aliases excluded.
pub fn names() -> impl Iterator<Item = String> {
    LUCIDE.iter().map(|(name, _)| format!("lucide:{name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_icon_and_alias_resolves_to_an_svg() {
        for name in names() {
            assert!(lookup(&name).unwrap().contains("currentColor"), "{name}");
        }
        for (alias, _) in LUCIDE_ALIASES {
            assert!(lookup(&format!("lucide:{alias}")).is_ok(), "{alias}");
        }
        assert!(lookup(FALLBACK).is_ok());
    }
}
