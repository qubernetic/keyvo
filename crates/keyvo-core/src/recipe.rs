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
use crate::edit::KeyFields;
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

impl Recipe {
    /// The fields of a key after applying variant `variant`: its action,
    /// the recipe's label and icon; the color and the modifier with its
    /// bindings stay `key`'s. `None` when the recipe has no such variant.
    #[must_use]
    pub fn apply(&self, variant: &str, key: &KeyFields) -> Option<KeyFields> {
        let variant = self.variants.iter().find(|v| v.id == variant)?;
        Some(KeyFields {
            icon: Some(self.icon.clone()),
            label: Some(self.label.clone()),
            action: Some(variant.action.clone()),
            ..key.clone()
        })
    }
}

/// The directory of the user's packs in the configuration directory.
pub const RECIPES: &str = "recipes";

/// The largest pack file read, in bytes; a bigger one is a problem. With
/// [`USER_PACKS`] it keeps a `recipes` answer under the client's line
/// limit (1 MiB).
pub const PACK_BYTES: usize = 8 * 1024;

/// The most user packs read, in file name order; the rest are one problem.
pub const USER_PACKS: usize = 64;

/// The packs in `contrib/recipes/`, built into keyvo; a test keeps the list
/// equal to the directory.
const BUNDLED: &[(&str, &str)] = &[
    (
        "brightness-down.toml",
        include_str!("../../../contrib/recipes/brightness-down.toml"),
    ),
    (
        "brightness-up.toml",
        include_str!("../../../contrib/recipes/brightness-up.toml"),
    ),
    (
        "calculator.toml",
        include_str!("../../../contrib/recipes/calculator.toml"),
    ),
    (
        "konsole.toml",
        include_str!("../../../contrib/recipes/konsole.toml"),
    ),
    (
        "lock.toml",
        include_str!("../../../contrib/recipes/lock.toml"),
    ),
    (
        "mute.toml",
        include_str!("../../../contrib/recipes/mute.toml"),
    ),
    (
        "new-email.toml",
        include_str!("../../../contrib/recipes/new-email.toml"),
    ),
    (
        "open-folder.toml",
        include_str!("../../../contrib/recipes/open-folder.toml"),
    ),
    (
        "open-website.toml",
        include_str!("../../../contrib/recipes/open-website.toml"),
    ),
    (
        "settings-page.toml",
        include_str!("../../../contrib/recipes/settings-page.toml"),
    ),
    (
        "spectacle.toml",
        include_str!("../../../contrib/recipes/spectacle.toml"),
    ),
    (
        "volume-down.toml",
        include_str!("../../../contrib/recipes/volume-down.toml"),
    ),
    (
        "volume-up.toml",
        include_str!("../../../contrib/recipes/volume-up.toml"),
    ),
    (
        "vscode.toml",
        include_str!("../../../contrib/recipes/vscode.toml"),
    ),
];

/// Recipes and the packs that did not load.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalogue {
    /// By group, then name, then id.
    pub recipes: Vec<Recipe>,
    /// One per pack left out.
    pub problems: Vec<Diagnostic>,
}

impl Catalogue {
    fn sort(&mut self) {
        self.recipes
            .sort_by(|a, b| (a.group, &a.name, &a.id).cmp(&(b.group, &b.name, &b.id)));
    }
}

/// The packs keyvo ships.
#[must_use]
pub fn bundled() -> Catalogue {
    let mut catalogue = Catalogue::default();
    for (name, text) in BUNDLED {
        let path = Path::new("contrib").join(RECIPES).join(name);
        match Recipe::from_toml(text, &path, Source::Bundled) {
            Ok(recipe) => catalogue.recipes.push(recipe),
            Err(problem) => catalogue.problems.push(problem),
        }
    }
    catalogue.sort();
    catalogue
}

/// The packs in `dir`: every `*.toml` file, at most [`USER_PACKS`] in file
/// name order, each at most [`PACK_BYTES`] long. A missing directory has
/// none; other files and subdirectories are ignored. Reading runs nothing.
#[must_use]
pub fn load_dir(dir: &Path, source: Source) -> Catalogue {
    let mut catalogue = Catalogue::default();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return catalogue,
        Err(e) => {
            catalogue.problems.push(problem(dir, e.to_string()));
            return catalogue;
        }
    };
    let mut files: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml") && path.is_file())
        .collect();
    files.sort();
    if files.len() > USER_PACKS {
        let left_out: Vec<_> = files[USER_PACKS..]
            .iter()
            .filter_map(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .collect();
        catalogue.problems.push(problem(
            dir,
            format!(
                "more than {USER_PACKS} recipe packs; left out: {}",
                left_out.join(", ")
            ),
        ));
        files.truncate(USER_PACKS);
    }
    for path in files {
        match read(&path).and_then(|text| Recipe::from_toml(&text, &path, source)) {
            Ok(recipe) => catalogue.recipes.push(recipe),
            Err(problem) => catalogue.problems.push(problem),
        }
    }
    catalogue.sort();
    catalogue
}

/// The bundled packs and the user's in `config_dir/recipes/`; a user pack
/// replaces the bundled one of its id.
#[must_use]
pub fn catalogue(config_dir: &Path) -> Catalogue {
    let mut catalogue = bundled();
    let user = load_dir(&config_dir.join(RECIPES), Source::User);
    catalogue
        .recipes
        .retain(|bundled| !user.recipes.iter().any(|mine| mine.id == bundled.id));
    catalogue.recipes.extend(user.recipes);
    catalogue.problems.extend(user.problems);
    catalogue.sort();
    catalogue
}

fn problem(path: &Path, message: String) -> Diagnostic {
    Diagnostic::at(path, "", None, message)
}

/// The text of the pack at `path`, refused past [`PACK_BYTES`] without
/// reading the rest.
fn read(path: &Path) -> Result<String, Diagnostic> {
    use std::io::Read as _;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| {
            file.take(u64::try_from(PACK_BYTES).unwrap_or(u64::MAX) + 1)
                .read_to_end(&mut bytes)
        })
        .map_err(|e| problem(path, e.to_string()))?;
    if bytes.len() > PACK_BYTES {
        return Err(problem(
            path,
            format!("a recipe pack is never larger than {PACK_BYTES} bytes"),
        ));
    }
    String::from_utf8(bytes).map_err(|_| problem(path, "a recipe pack is UTF-8 text".to_owned()))
}

/// The schema of a pack file, for [`crate::schema::recipe`].
pub(crate) fn render_schema() -> String {
    crate::schema::render::<Pack>()
}
