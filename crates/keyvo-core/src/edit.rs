//! Round-trip editing of profile files (spec §7.1, ADR-0004): a key's
//! fields change through `toml_edit`, so comments, order and formatting
//! the user wrote survive, and every byte outside the edited key stays as
//! it was. The edited text is validated as a [`Profile`] before it is
//! handed out; the editor never writes a file the daemon would refuse.
//!
//! Writing the text to disk is [`crate::store`]'s job.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use toml_edit::{Document, DocumentMut, InlineTable, Item, Table, TableLike, Value};

use crate::diagnostic::Diagnostic;
use crate::profile::{Action, Color, Key, KeySlot, Profile};

/// The text of [`Profile::builtin_global`], the file the editor starts
/// from when `profiles/global.toml` does not exist yet.
pub const BUILTIN_GLOBAL: &str = "\
schema_version = 1

[profile]
id = \"global\"
name = \"Global\"

[[pages]]
name = \"context\"
";

/// The fields of a key the editor changes; `states` and anything else in
/// the key's table are kept as they are. An absent field is removed from
/// the file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyFields {
    /// Icon reference, for example `lucide:hammer`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Text drawn on the key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Background color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// What a press does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<Action>,
}

impl KeyFields {
    /// The editable fields of `key`.
    #[must_use]
    pub fn of(key: &Key) -> Self {
        Self {
            icon: key.icon.clone(),
            label: key.label.clone(),
            color: key.color,
            action: key.action.clone(),
        }
    }
}

/// The fields of the `[profile]` table the editor changes
/// (`docs/design/gui-ux.md` §3.2); the id, the match rules and the priority
/// are kept as they are. An absent accent is removed from the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileFields {
    /// Display name.
    pub name: String,
    /// Accent color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<Color>,
}

impl ProfileFields {
    /// The editable fields of `profile`.
    #[must_use]
    pub fn of(profile: &Profile) -> Self {
        Self {
            name: profile.profile.name.clone(),
            accent: profile.profile.accent,
        }
    }
}

/// Why a key cannot be edited.
#[derive(Debug, thiserror::Error)]
pub enum EditError {
    /// The page is not in the profile.
    #[error("{}: profile `{profile}` has no page {page}; it has {pages}", path.display())]
    NoPage {
        /// The profile file.
        path: PathBuf,
        /// The profile id.
        profile: String,
        /// The page asked for, counted from 1.
        page: usize,
        /// The number of pages.
        pages: usize,
    },
    /// The file spells a part of the key in a form the editor cannot change
    /// without rewriting it, for example `pages` as an array of strings.
    #[error("{}: {what} is not a table; edit this key in a text editor", path.display())]
    Shape {
        /// The profile file.
        path: PathBuf,
        /// The part, for example `page 1 keys`.
        what: String,
    },
    /// The edited text is not a valid profile.
    #[error(transparent)]
    Invalid(#[from] Diagnostic),
}

/// A profile file open for editing: its text as `toml_edit` holds it and
/// the profile it validates to.
#[derive(Debug, Clone)]
pub struct ProfileDocument {
    path: PathBuf,
    doc: DocumentMut,
    profile: Profile,
}

impl ProfileDocument {
    /// Opens the text of a profile file; `path` labels the diagnostics.
    ///
    /// # Errors
    ///
    /// A [`Diagnostic`] with the position of the first problem, as
    /// [`Profile::from_toml`] reports it.
    pub fn parse(text: &str, path: &Path) -> Result<Self, Diagnostic> {
        let profile = Profile::from_toml(text, path)?;
        let doc = text
            .parse::<DocumentMut>()
            .map_err(|e| Diagnostic::at(path, text, e.span(), e.message().trim_end().to_owned()))?;
        Ok(Self {
            path: path.to_path_buf(),
            doc,
            profile,
        })
    }

    /// The profile the text validates to.
    #[must_use]
    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// The fields of key `slot` on page `page` (counted from 1); all absent
    /// for a key the page does not define.
    ///
    /// # Errors
    ///
    /// [`EditError::NoPage`] when the profile has no such page.
    pub fn key(&self, page: usize, slot: KeySlot) -> Result<KeyFields, EditError> {
        let pages = &self.profile.pages;
        let found = page
            .checked_sub(1)
            .and_then(|index| pages.get(index))
            .ok_or_else(|| EditError::NoPage {
                path: self.path.clone(),
                profile: self.profile.profile.id.clone(),
                page,
                pages: pages.len(),
            })?;
        Ok(found.keys.get(&slot).map(KeyFields::of).unwrap_or_default())
    }

    /// The line, counted from 1, on which the table of key `slot` on page
    /// `page` starts in the text: its `[pages.keys.N]` header, or for a key
    /// written as an inline table its value. `None` for a key the page does
    /// not define.
    #[must_use]
    pub fn key_line(&self, page: usize, slot: KeySlot) -> Option<usize> {
        // A `DocumentMut` keeps no spans, so the text is parsed again.
        let text = self.doc.to_string();
        let doc = Document::parse(text.as_str()).ok()?;
        let index = page.checked_sub(1)?;
        let page_table: &dyn TableLike = match doc.get("pages")? {
            Item::ArrayOfTables(pages) => pages.get(index)?,
            Item::Value(Value::Array(pages)) => pages.get(index)?.as_inline_table()?,
            _ => return None,
        };
        let keys = page_table.get("keys")?.as_table_like()?;
        let name = slot.get().to_string();
        let span = match keys.get(&name)? {
            Item::Table(table) => table.span(),
            Item::Value(value) => value.span(),
            _ => None,
        }
        // A dotted key (`keys.2.label = ...`) has no span of its own; the
        // first place its name is written stands for it.
        .or_else(|| keys.key(&name)?.span())?;
        Some(text[..span.start].matches('\n').count() + 1)
    }

    /// Sets the fields of key `slot` on page `page` to `fields`; only the
    /// values that differ are touched, a field absent in `fields` is
    /// removed, and a key the page does not define is added at the end of
    /// the page. Returns the fields before the edit, for undo.
    ///
    /// # Errors
    ///
    /// [`EditError::NoPage`] for a page the profile does not have,
    /// [`EditError::Shape`] when the key cannot be reached as a table, and
    /// [`EditError::Invalid`] when the edited text does not validate; the
    /// document is unchanged after an error.
    pub fn set_key(
        &mut self,
        page: usize,
        slot: KeySlot,
        fields: &KeyFields,
    ) -> Result<KeyFields, EditError> {
        let before = self.key(page, slot)?;
        if before == *fields {
            return Ok(before);
        }
        let mut doc = self.doc.clone();
        let key = self.key_table(&mut doc, page, slot)?;
        if before.icon != fields.icon {
            set_value(key, "icon", fields.icon.as_deref().map(Value::from));
        }
        if before.label != fields.label {
            set_value(key, "label", fields.label.as_deref().map(Value::from));
        }
        if before.color != fields.color {
            set_value(key, "color", fields.color.map(|c| c.to_string().into()));
        }
        if before.action != fields.action {
            set_action(key, before.action.as_ref(), fields.action.as_ref());
        }
        let text = doc.to_string();
        self.profile = Profile::from_toml(&text, &self.path)?;
        self.doc = doc;
        Ok(before)
    }

    /// The fields of the `[profile]` table.
    #[must_use]
    pub fn profile_fields(&self) -> ProfileFields {
        ProfileFields::of(&self.profile)
    }

    /// Sets the name and the accent of the `[profile]` table to `fields`;
    /// only the values that differ are touched, an absent accent is
    /// removed, a new one goes to the end of the table. Returns the fields
    /// before the edit, for undo.
    ///
    /// # Errors
    ///
    /// [`EditError::Shape`] when `profile` is not a table, and
    /// [`EditError::Invalid`] when the edited text does not validate; the
    /// document is unchanged after an error.
    pub fn set_profile(&mut self, fields: &ProfileFields) -> Result<ProfileFields, EditError> {
        let before = self.profile_fields();
        if before == *fields {
            return Ok(before);
        }
        let mut doc = self.doc.clone();
        let table = doc
            .get_mut("profile")
            .and_then(Item::as_table_like_mut)
            .ok_or_else(|| EditError::Shape {
                path: self.path.clone(),
                what: "profile".into(),
            })?;
        if before.name != fields.name {
            set_value(table, "name", Some(fields.name.as_str().into()));
        }
        if before.accent != fields.accent {
            set_value(table, "accent", fields.accent.map(|c| c.to_string().into()));
        }
        let text = doc.to_string();
        self.profile = Profile::from_toml(&text, &self.path)?;
        self.doc = doc;
        Ok(before)
    }

    /// The table of key `slot` on page `page` in `doc`, created when the
    /// page has none.
    fn key_table<'d>(
        &self,
        doc: &'d mut DocumentMut,
        page: usize,
        slot: KeySlot,
    ) -> Result<&'d mut dyn TableLike, EditError> {
        let shape = |what: String| EditError::Shape {
            path: self.path.clone(),
            what,
        };
        let index = page - 1;
        let (page_table, inline): (&mut dyn TableLike, bool) =
            match doc.get_mut("pages").ok_or_else(|| shape("pages".into()))? {
                Item::ArrayOfTables(pages) => pages
                    .get_mut(index)
                    .map(|t| (t as &mut dyn TableLike, false))
                    .ok_or_else(|| shape(format!("page {page}")))?,
                Item::Value(Value::Array(pages)) => pages
                    .get_mut(index)
                    .and_then(Value::as_inline_table_mut)
                    .map(|t| (t as &mut dyn TableLike, true))
                    .ok_or_else(|| shape(format!("page {page}")))?,
                _ => return Err(shape("pages".into())),
            };
        let (keys, inline) = child(page_table, inline, "keys", true)
            .ok_or_else(|| shape(format!("page {page} keys")))?;
        let name = slot.get().to_string();
        child(keys, inline, &name, false)
            .map(|(key, _)| key)
            .ok_or_else(|| shape(format!("page {page} key {name}")))
    }
}

impl fmt::Display for ProfileDocument {
    /// The text of the file, as it is written back.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.doc)
    }
}

/// The table `name` in `parent`, created when it is missing: an inline
/// table inside an inline one, else a `[header]` table, without its own
/// header when `implicit` (`[pages.keys]` is never written). The flag
/// says whether the child is inline.
fn child<'p>(
    parent: &'p mut dyn TableLike,
    inline: bool,
    name: &str,
    implicit: bool,
) -> Option<(&'p mut dyn TableLike, bool)> {
    if !parent.contains_key(name) {
        let item = if inline {
            Item::Value(Value::InlineTable(InlineTable::new()))
        } else {
            let mut table = Table::new();
            table.set_implicit(implicit);
            Item::Table(table)
        };
        parent.insert(name, item);
    }
    let item = parent.get_mut(name)?;
    let inline = item.is_inline_table();
    item.as_table_like_mut().map(|table| (table, inline))
}

/// Sets `name` in `table` to `value` keeping the whitespace and comments
/// around the old value, or removes it for `None`.
fn set_value(table: &mut dyn TableLike, name: &str, value: Option<Value>) {
    let Some(mut value) = value else {
        table.remove(name);
        return;
    };
    match table.get_mut(name).and_then(Item::as_value_mut) {
        Some(old) => {
            *value.decor_mut() = old.decor().clone();
            *old = value;
        }
        None => {
            table.insert(name, Item::Value(value));
        }
    }
}

/// The type name, the field and the value of `action`.
fn parts(action: &Action) -> (&'static str, &'static str, String) {
    match action {
        Action::Chord { keys } => ("chord", "keys", keys.to_string()),
        Action::Exec { cmd } => ("exec", "cmd", cmd.clone()),
        Action::Launch { cmd } => ("launch", "cmd", cmd.clone()),
    }
}

/// Changes the key's `action` from `before` to `after`. An action of the
/// same type keeps its table and only its value changes; another type
/// replaces the table's entries in place; a new action is an inline table.
fn set_action(key: &mut dyn TableLike, before: Option<&Action>, after: Option<&Action>) {
    let Some(after) = after else {
        key.remove("action");
        return;
    };
    let (kind, field, value) = parts(after);
    let Some(table) = key.get_mut("action").and_then(Item::as_table_like_mut) else {
        let mut table = InlineTable::new();
        table.insert("type", kind.into());
        table.insert(field, value.into());
        set_value(key, "action", Some(Value::InlineTable(table)));
        return;
    };
    let old = before.map(parts);
    if old.as_ref().map(|(kind, ..)| *kind) != Some(kind) {
        set_value(table, "type", Some(kind.into()));
        // `exec` and `launch` share `cmd`; it keeps its place and decor.
        if let Some((_, old_field, _)) = old.filter(|(_, old_field, _)| *old_field != field) {
            table.remove(old_field);
        }
    }
    set_value(table, field, Some(value.into()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_global_text_is_the_builtin_global_profile() {
        let doc = ProfileDocument::parse(BUILTIN_GLOBAL, Path::new("global.toml")).unwrap();
        assert_eq!(*doc.profile(), Profile::builtin_global());
    }

    fn slot(n: u8) -> KeySlot {
        KeySlot::new(n).unwrap()
    }

    fn document(text: &str) -> ProfileDocument {
        ProfileDocument::parse(text, Path::new("lines.toml")).unwrap()
    }

    const HEADER: &str = "\
schema_version = 1

[profile]
id = \"lines\"
name = \"Lines\"
";

    #[test]
    fn the_line_of_a_key_table_is_its_header() {
        let doc = document(&format!(
            "{HEADER}\n[[pages]]\nname = \"one\"\n\n[pages.keys.1]\nlabel = \"A\"\n\n\
             # the second key\n[pages.keys.2]   # copy\nlabel = \"B\"\n"
        ));
        assert_eq!(doc.key_line(1, slot(1)), Some(10));
        assert_eq!(doc.key_line(1, slot(2)), Some(14));
    }

    #[test]
    fn the_line_of_an_inline_key_is_its_value() {
        let doc = document(&format!(
            "{HEADER}\n[[pages]]\nname = \"one\"\nkeys = {{ 2 = {{ label = \"B\" }} }}\n\n\
             [[pages]]\nname = \"two\"\n\n[pages.keys]\n1 = {{ label = \"C\" }}\n"
        ));
        assert_eq!(doc.key_line(1, slot(2)), Some(9));
        assert_eq!(doc.key_line(2, slot(1)), Some(15));
    }

    #[test]
    fn the_line_of_a_dotted_key_is_where_its_name_is_first_written() {
        let doc = document(&format!(
            "{HEADER}\n[[pages]]\nname = \"one\"\nkeys.3.label = \"C\"\nkeys.3.icon = \"lucide:copy\"\n"
        ));
        assert_eq!(doc.key_line(1, slot(3)), Some(9));
    }

    #[test]
    fn a_key_the_page_does_not_define_has_no_line() {
        let doc = document(&format!(
            "{HEADER}\n[[pages]]\nname = \"one\"\n\n[pages.keys.1]\nlabel = \"A\"\n\n[[pages]]\n"
        ));
        assert_eq!(doc.key_line(1, slot(2)), None);
        assert_eq!(doc.key_line(2, slot(1)), None);
        assert_eq!(doc.key_line(3, slot(1)), None);
        assert_eq!(doc.key_line(0, slot(1)), None);
    }

    #[test]
    fn a_key_set_key_adds_has_a_line_at_the_end_of_its_page() {
        let mut doc = document(&format!(
            "{HEADER}\n[[pages]]\nname = \"one\"\n\n[pages.keys.1]\nlabel = \"A\"\n\n\
             [[pages]]\nname = \"two\"\n"
        ));
        assert_eq!(doc.key_line(1, slot(5)), None);
        let fields = KeyFields {
            label: Some("E".into()),
            ..KeyFields::default()
        };
        doc.set_key(1, slot(5), &fields).unwrap();
        let text = doc.to_string();
        let line = doc.key_line(1, slot(5)).unwrap();
        assert_eq!(text.lines().nth(line - 1), Some("[pages.keys.5]"), "{text}");
        assert!(line > 11 && line < 15, "{line}: {text}");
        assert_eq!(doc.key_line(1, slot(1)), Some(10));
    }
}
