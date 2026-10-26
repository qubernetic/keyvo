//! Recipe packs: the entries of the GUI's actions catalogue
//! (`docs/design/gui-ux.md` §4.3, `docs/plans/2026-10-26-pad-screen-brief.md`
//! §5.2). One TOML file per entry, its id the file name: a group, a name,
//! an icon and a default label, and one or more variants ("What it does"),
//! each an action of the profile schema. Applying a recipe to a key writes
//! the variant's action, the label and the icon into the profile
//! ([`Recipe::apply`]); nothing in a pack runs before that.
//!
//! The packs keyvo ships live in `contrib/recipes/`; the user's in
//! `recipes/` of the configuration directory.

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use toml::Spanned;

use crate::diagnostic::Diagnostic;
use crate::profile::{Action, SchemaVersion, is_valid_id};

/// The catalogue group of a recipe, in catalogue order.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Group {
    /// Everyday actions: open a folder, a website, a new email.
    General,
    /// The desktop's controls: lock, volume, brightness, settings.
    System,
    /// One entry per application.
    Apps,
}

/// Where a recipe was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// Shipped with keyvo (`contrib/recipes/`).
    Bundled,
    /// The user's `recipes/` directory.
    User,
}

/// One recipe as the socket carries it.
///
/// Unknown fields are ignored, so a v1 client reads the recipes of a later
/// daemon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recipe {
    /// The file name without `.toml`.
    pub id: String,
    /// Where the pack was read from.
    pub source: Source,
    /// The catalogue group.
    pub group: Group,
    /// The entry's name in the catalogue.
    pub name: String,
    /// Icon reference written into the key, for example `lucide:camera`.
    pub icon: String,
    /// Label written into the key.
    pub label: String,
    /// What it does; the first is the default.
    pub variants: Vec<Variant>,
}

/// One "What it does" choice of a recipe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Variant {
    /// Unique within the recipe.
    pub id: String,
    /// The choice's name.
    pub name: String,
    /// The action written into the key.
    #[serde(deserialize_with = "crate::trace::known_action")]
    pub action: Action,
}

/// A whole recipe pack file.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(title = "keyvo recipe pack")]
struct Pack {
    /// Version of the pack format; this build reads `1`.
    #[allow(dead_code, reason = "checked while it is deserialized")]
    schema_version: SchemaVersion,
    /// The catalogue entry.
    recipe: Info,
    /// What it does; the first is the default.
    #[schemars(with = "Vec<VariantFile>", length(min = 1))]
    variants: Spanned<Vec<VariantFile>>,
}

/// The `[recipe]` table.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "RecipeInfo")]
struct Info {
    /// The catalogue group.
    group: Group,
    /// The entry's name in the catalogue.
    name: String,
    /// Icon reference written into the key, for example `lucide:camera`.
    icon: String,
    /// Label written into the key.
    label: String,
}

/// One `[[variants]]` entry.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "Variant")]
struct VariantFile {
    /// Unique within the pack: lowercase letters, digits, `.`, `_` and `-`.
    #[schemars(with = "String", regex(pattern = r"^[a-z0-9][a-z0-9._-]*$"))]
    id: Spanned<String>,
    /// The choice's name.
    name: String,
    /// The action written into the key.
    action: Action,
}

impl Recipe {
    /// Parses and validates the text of a pack file; its id is the name of
    /// `path` without `.toml`, which also labels the diagnostic.
    ///
    /// # Errors
    ///
    /// A [`Diagnostic`] with the position of the first problem; without a
    /// position when the file name is not an id.
    pub fn from_toml(text: &str, path: &Path, source: Source) -> Result<Self, Diagnostic> {
        let stem = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !is_valid_id(&stem) {
            return Err(Diagnostic::at(
                path,
                text,
                None,
                format!(
                    "the file name `{stem}` is not a recipe id: lowercase letters, digits, `.`, \
                     `_` and `-`, starting with a letter or digit"
                ),
            ));
        }
        let pack: Pack = toml::from_str(text).map_err(|e| Diagnostic::from_toml(path, text, &e))?;
        let at = |span, message| Diagnostic::at(path, text, Some(span), message);
        if pack.variants.get_ref().is_empty() {
            return Err(at(
                pack.variants.span(),
                "a recipe needs at least one variant".to_owned(),
            ));
        }
        let mut variants: Vec<Variant> = Vec::new();
        for variant in pack.variants.into_inner() {
            let span = variant.id.span();
            let id = variant.id.into_inner();
            if !is_valid_id(&id) {
                return Err(at(
                    span,
                    format!(
                        "variant id `{id}` is not an id of lowercase letters, digits, `.`, `_` \
                         and `-`, starting with a letter or digit"
                    ),
                ));
            }
            if variants.iter().any(|v| v.id == id) {
                return Err(at(span, format!("the pack defines variant `{id}` twice")));
            }
            variants.push(Variant {
                id,
                name: variant.name,
                action: variant.action,
            });
        }
        Ok(Self {
            id: stem,
            source,
            group: pack.recipe.group,
            name: pack.recipe.name,
            icon: pack.recipe.icon,
            label: pack.recipe.label,
            variants,
        })
    }
}

/// The schema of a pack file, for [`crate::schema::recipe`].
pub(crate) fn render_schema() -> String {
    crate::schema::render::<Pack>()
}
