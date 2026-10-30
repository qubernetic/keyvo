//! Round-trip editing of profile files (spec §7.1, ADR-0004): a key's
//! fields change through `toml_edit`, so comments, order and formatting
//! the user wrote survive, and every byte outside the edited key stays as
//! it was. The edited text is validated as a [`Profile`] before it is
//! handed out; the editor never writes a file the daemon would refuse.
//!
//! Writing the text to disk is [`crate::store`]'s job.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use toml_edit::{ArrayOfTables, Document, DocumentMut, InlineTable, Item, Table, TableLike, Value};

use crate::diagnostic::Diagnostic;
use crate::dialpad::{ButtonSlot, Roller};
use crate::profile::{
    Action, Color, DialButton, Key, KeySlot, ModifierMode, Page, Profile, Rotary,
};
use crate::state::context_page;

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

/// The fields of a key the editor changes; `on_fail`, `states` and anything
/// else in the key's table are kept as they are. An absent field is removed from
/// the file; a `dial` or `roller` without fields stays as an empty table,
/// which makes the key a modifier with nothing bound.
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
    /// The modifier mode as written; absent for the default (`held` with a
    /// `dial` or `roller`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modifier: Option<ModifierMode>,
    /// The dial's binding while the key is the active modifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dial: Option<RotaryFields>,
    /// The roller's binding while the key is the active modifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roller: Option<RotaryFields>,
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
            modifier: key.modifier,
            dial: key.dial.as_ref().map(RotaryFields::of),
            roller: key.roller.as_ref().map(RotaryFields::of),
        }
    }

    /// The fields with every `step` of 1 written as absent.
    fn normalized(&self) -> Self {
        Self {
            dial: self.dial.as_ref().map(RotaryFields::normalized),
            roller: self.roller.as_ref().map(RotaryFields::normalized),
            ..self.clone()
        }
    }
}

/// The fields of the `[profile]` table the editor changes
/// (`docs/design/gui-ux.md` §3.2); the id, the match rules and the priority
/// are kept as they are. An absent accent is removed from the file, and so
/// is `global_pages` at its default `true`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileFields {
    /// Display name.
    pub name: String,
    /// Accent color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<Color>,
    /// Whether the global pages sit left of the context page (spec §6.3).
    #[serde(default = "yes", skip_serializing_if = "is_yes")]
    pub global_pages: bool,
}

impl ProfileFields {
    /// The editable fields of `profile`.
    #[must_use]
    pub fn of(profile: &Profile) -> Self {
        Self {
            name: profile.profile.name.clone(),
            accent: profile.profile.accent,
            global_pages: profile.profile.global_pages,
        }
    }
}

const fn yes() -> bool {
    true
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if passes a reference"
)]
const fn is_yes(value: &bool) -> bool {
    *value
}

/// One change to the pages of a profile (spec §10): every page counted
/// from 1. Written as JSON with the `op` tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
pub enum PageEdit {
    /// A new page at the end, named `name` or without a name.
    Add {
        /// The page name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    /// Page `page` named `name`; no name removes it.
    Rename {
        /// The page.
        page: usize,
        /// The new name.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    /// Page `page` moved to position `to`, the pages between shifted.
    Move {
        /// The page.
        page: usize,
        /// Its position after the move.
        to: usize,
    },
    /// Page `page` switched on or off; on is written as absent.
    Enable {
        /// The page.
        page: usize,
        /// Whether it is in the page list.
        enabled: bool,
    },
    /// Page `page` removed with its keys.
    Delete {
        /// The page.
        page: usize,
    },
}

/// What a [`PageEdit`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageEdited {
    /// The edited page's number after the edit; `None` after a delete.
    pub page: Option<usize>,
    /// The edit that undoes it; `None` after a delete, whose undo is the
    /// file before it.
    pub undo: Option<PageEdit>,
}

/// The fields of the dial or the roller the editor changes (spec §7.2). An
/// absent field is removed from the file; `step` 1 is the default and is
/// written as absent. A control left without any field is removed, so the
/// global profile's binding shows through again.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RotaryFields {
    /// What a clockwise turn runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cw: Option<Action>,
    /// What a counter-clockwise turn runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ccw: Option<Action>,
    /// Detents per run; absent for the default 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<u32>,
    /// What the Dialpad panel shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl RotaryFields {
    /// The editable fields of `rotary`.
    #[must_use]
    pub fn of(rotary: &Rotary) -> Self {
        Self {
            cw: rotary.cw.clone(),
            ccw: rotary.ccw.clone(),
            step: Some(rotary.step.get()),
            label: rotary.label.clone(),
        }
        .normalized()
    }

    /// The fields with a `step` of 1 written as absent.
    #[must_use]
    fn normalized(&self) -> Self {
        Self {
            step: self.step.filter(|&step| step != 1),
            ..self.clone()
        }
    }
}

/// The fields of a Dialpad button the editor changes; `on_fail` is kept as
/// it is. A button left without any field is removed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ButtonFields {
    /// What the Dialpad panel shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// What a press does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<Action>,
}

impl ButtonFields {
    /// The editable fields of `button`.
    #[must_use]
    pub fn of(button: &DialButton) -> Self {
        Self {
            label: button.label.clone(),
            action: button.action.clone(),
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
    /// The edit would leave the profile without a page in its page list:
    /// the context page cannot be disabled, the only enabled page cannot be
    /// deleted.
    #[error("{}: page {page} of profile `{profile}` {why}", path.display())]
    Protected {
        /// The profile file.
        path: PathBuf,
        /// The profile id.
        profile: String,
        /// The page, counted from 1.
        page: usize,
        /// Why, for example `is the context page`.
        why: &'static str,
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

    /// The `on_fail` action of key `slot` on page `page` (counted from 1),
    /// which the editor shows and never writes; `None` for a key without
    /// one and a page or key the profile does not define.
    #[must_use]
    pub fn on_fail(&self, page: usize, slot: KeySlot) -> Option<Action> {
        let index = page.checked_sub(1)?;
        self.profile
            .pages
            .get(index)?
            .keys
            .get(&slot)?
            .on_fail
            .clone()
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
        let fields = &fields.normalized();
        if before == *fields {
            return Ok(before);
        }
        let mut doc = self.doc.clone();
        let (key, inline) = self.key_table(&mut doc, page, slot)?;
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
            set_action(
                key,
                "action",
                before.action.as_ref(),
                fields.action.as_ref(),
            );
        }
        if before.modifier != fields.modifier {
            set_value(
                key,
                "modifier",
                fields.modifier.map(|mode| mode.as_str().into()),
            );
        }
        for (name, before, after) in [
            ("dial", &before.dial, &fields.dial),
            ("roller", &before.roller, &fields.roller),
        ] {
            if before == after {
                continue;
            }
            let Some(after) = after else {
                key.remove(name);
                continue;
            };
            let shape = || EditError::Shape {
                path: self.path.clone(),
                what: format!("page {page} key {} {name}", slot.get()),
            };
            let before = before.clone().unwrap_or_default();
            write_rotary(key, inline, name, &before, after, true).ok_or_else(shape)?;
        }
        self.commit(doc)?;
        Ok(before)
    }

    /// Validates the edited `doc` and takes it; the document is unchanged
    /// when it does not validate.
    fn commit(&mut self, doc: DocumentMut) -> Result<(), EditError> {
        let text = doc.to_string();
        self.profile = Profile::from_toml(&text, &self.path)?;
        // Parsed again, a table the edit added gets its place in the text,
        // which a later move of its page needs.
        self.doc = text.parse().unwrap_or(doc);
        Ok(())
    }

    /// The fields of `roller` in the profile's own `[dialpad]` table; all
    /// absent when the table does not bind it.
    #[must_use]
    pub fn rotary(&self, roller: Roller) -> RotaryFields {
        let dialpad = self.profile.dialpad.as_ref();
        let rotary = match roller {
            Roller::Dial => dialpad.and_then(|d| d.dial.as_ref()),
            Roller::Roller => dialpad.and_then(|d| d.roller.as_ref()),
        };
        rotary.map(RotaryFields::of).unwrap_or_default()
    }

    /// The fields of button `slot` in the profile's own `[dialpad]` table.
    #[must_use]
    pub fn button(&self, slot: ButtonSlot) -> ButtonFields {
        self.dial_button(slot)
            .map(ButtonFields::of)
            .unwrap_or_default()
    }

    /// The `on_fail` of button `slot`, which the editor shows and never
    /// writes.
    #[must_use]
    pub fn button_on_fail(&self, slot: ButtonSlot) -> Option<Action> {
        self.dial_button(slot)?.on_fail.clone()
    }

    fn dial_button(&self, slot: ButtonSlot) -> Option<&DialButton> {
        self.profile.dialpad.as_ref()?.buttons.get(&slot)
    }

    /// Sets the fields of `roller` to `fields` as [`Self::set_key`] does
    /// for a key: only what differs is touched, a short-form chord stays
    /// short while it stays a chord, a new binding is an inline table in
    /// `[dialpad]`, and a binding left without fields is removed. Returns
    /// the fields before the edit, for undo.
    ///
    /// # Errors
    ///
    /// [`EditError::Shape`] when `dialpad` is not a table, and
    /// [`EditError::Invalid`] when the edited text does not validate; the
    /// document is unchanged after an error.
    pub fn set_rotary(
        &mut self,
        roller: Roller,
        fields: &RotaryFields,
    ) -> Result<RotaryFields, EditError> {
        let before = self.rotary(roller);
        let fields = fields.normalized();
        if before == fields {
            return Ok(before);
        }
        let mut doc = self.doc.clone();
        let (dialpad, inline) = self.dialpad_table(&mut doc)?;
        let name = roller.as_str();
        write_rotary(dialpad, inline, name, &before, &fields, false).ok_or_else(|| {
            EditError::Shape {
                path: self.path.clone(),
                what: format!("dialpad {name}"),
            }
        })?;
        drop_empty_dialpad(&mut doc);
        self.commit(doc)?;
        Ok(before)
    }

    /// Sets the fields of button `slot` to `fields`, keeping its `on_fail`;
    /// a new button is a `[dialpad.buttons.N]` table, a button left without
    /// fields is removed. Returns the fields before the edit, for undo.
    ///
    /// # Errors
    ///
    /// As [`Self::set_rotary`].
    pub fn set_button(
        &mut self,
        slot: ButtonSlot,
        fields: &ButtonFields,
    ) -> Result<ButtonFields, EditError> {
        let before = self.button(slot);
        if before == *fields {
            return Ok(before);
        }
        let mut doc = self.doc.clone();
        let (dialpad, inline) = self.dialpad_table(&mut doc)?;
        let shape = |what: &str| EditError::Shape {
            path: self.path.clone(),
            what: what.to_owned(),
        };
        let (buttons, inline) =
            child(dialpad, inline, "buttons", true).ok_or_else(|| shape("dialpad buttons"))?;
        let name = slot.get().to_string();
        let (button, _) =
            child(buttons, inline, &name, false).ok_or_else(|| shape("dialpad button"))?;
        if before.label != fields.label {
            set_value(button, "label", fields.label.as_deref().map(Value::from));
        }
        if before.action != fields.action {
            set_action(
                button,
                "action",
                before.action.as_ref(),
                fields.action.as_ref(),
            );
        }
        if button.is_empty() {
            buttons.remove(&name);
        }
        if buttons.is_empty() {
            dialpad.remove("buttons");
        }
        drop_empty_dialpad(&mut doc);
        self.commit(doc)?;
        Ok(before)
    }

    /// The `[dialpad]` table of `doc`, created at the end of the file when
    /// it is missing, and whether it is inline.
    fn dialpad_table<'d>(
        &self,
        doc: &'d mut DocumentMut,
    ) -> Result<(&'d mut dyn TableLike, bool), EditError> {
        child(doc.as_table_mut(), false, "dialpad", false).ok_or_else(|| EditError::Shape {
            path: self.path.clone(),
            what: "dialpad".into(),
        })
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
        if before.global_pages != fields.global_pages {
            set_value(
                table,
                "global_pages",
                (!fields.global_pages).then(|| false.into()),
            );
        }
        self.commit(doc)?;
        Ok(before)
    }

    /// Applies `edit` to the pages; every byte outside the edited page stays
    /// as it was, a moved page takes its comments and keys along. Returns
    /// where the page is now and the edit that undoes it.
    ///
    /// # Errors
    ///
    /// [`EditError::NoPage`] for a page or position the profile does not
    /// have, [`EditError::Protected`] for disabling the context page or
    /// deleting the only enabled page, [`EditError::Shape`] when `pages` is
    /// not an array of `[[pages]]` tables (an inline array can be renamed
    /// and switched, not added to, moved in or deleted from), and
    /// [`EditError::Invalid`] when the edited text does not validate; the
    /// document is unchanged after an error.
    pub fn edit_page(&mut self, edit: &PageEdit) -> Result<PageEdited, EditError> {
        let mut doc = self.doc.clone();
        let done = match edit {
            PageEdit::Add { name } => {
                let pages = self.page_array(&mut doc)?;
                let mut table = Table::new();
                if let Some(name) = name {
                    table.insert("name", toml_edit::value(name.as_str()));
                }
                pages.push(table);
                let page = pages.len();
                PageEdited {
                    page: Some(page),
                    undo: Some(PageEdit::Delete { page }),
                }
            }
            PageEdit::Rename { page, name } => {
                let before = self.page(*page)?.name.clone();
                set_value(
                    self.page_table(&mut doc, *page)?,
                    "name",
                    name.as_deref().map(Value::from),
                );
                PageEdited {
                    page: Some(*page),
                    undo: Some(PageEdit::Rename {
                        page: *page,
                        name: before,
                    }),
                }
            }
            PageEdit::Move { page, to } => {
                self.page(*page)?;
                self.page(*to)?;
                let pages = self.page_array(&mut doc)?;
                move_page(pages, page - 1, to - 1);
                PageEdited {
                    page: Some(*to),
                    undo: Some(PageEdit::Move {
                        page: *to,
                        to: *page,
                    }),
                }
            }
            PageEdit::Enable { page, enabled } => {
                let before = self.page(*page)?.enabled;
                if !enabled && *page == context_page(&self.profile) {
                    return Err(self.protected(*page, "is the context page"));
                }
                set_value(
                    self.page_table(&mut doc, *page)?,
                    "enabled",
                    (!enabled).then(|| false.into()),
                );
                PageEdited {
                    page: Some(*page),
                    undo: Some(PageEdit::Enable {
                        page: *page,
                        enabled: before,
                    }),
                }
            }
            PageEdit::Delete { page } => {
                self.page(*page)?;
                if self.only_enabled(*page) {
                    return Err(self.protected(*page, "is the only enabled page"));
                }
                self.page_array(&mut doc)?.remove(page - 1);
                PageEdited {
                    page: None,
                    undo: None,
                }
            }
        };
        self.commit(doc)?;
        Ok(done)
    }

    /// Page `page` (from 1) of the profile.
    fn page(&self, page: usize) -> Result<&Page, EditError> {
        let pages = &self.profile.pages;
        page.checked_sub(1)
            .and_then(|index| pages.get(index))
            .ok_or_else(|| EditError::NoPage {
                path: self.path.clone(),
                profile: self.profile.profile.id.clone(),
                page,
                pages: pages.len(),
            })
    }

    /// Whether page `page` is the context page and no other page is
    /// enabled: the one page the list would have left.
    fn only_enabled(&self, page: usize) -> bool {
        page == context_page(&self.profile)
            && !self
                .profile
                .pages
                .iter()
                .enumerate()
                .any(|(at, other)| at + 1 != page && other.enabled)
    }

    fn protected(&self, page: usize, why: &'static str) -> EditError {
        EditError::Protected {
            path: self.path.clone(),
            profile: self.profile.profile.id.clone(),
            page,
            why,
        }
    }

    /// The `[[pages]]` array of `doc`, created when the file has no page.
    fn page_array<'d>(&self, doc: &'d mut DocumentMut) -> Result<&'d mut ArrayOfTables, EditError> {
        doc.entry("pages")
            .or_insert_with(|| Item::ArrayOfTables(ArrayOfTables::new()))
            .as_array_of_tables_mut()
            .ok_or_else(|| EditError::Shape {
                path: self.path.clone(),
                what: "pages".into(),
            })
    }

    /// The table of page `page` (from 1) in `doc`, a `[[pages]]` table or
    /// an inline one.
    fn page_table<'d>(
        &self,
        doc: &'d mut DocumentMut,
        page: usize,
    ) -> Result<&'d mut dyn TableLike, EditError> {
        let shape = |what: String| EditError::Shape {
            path: self.path.clone(),
            what,
        };
        let index = page - 1;
        match doc.get_mut("pages").ok_or_else(|| shape("pages".into()))? {
            Item::ArrayOfTables(pages) => pages
                .get_mut(index)
                .map(|t| t as &mut dyn TableLike)
                .ok_or_else(|| shape(format!("page {page}"))),
            Item::Value(Value::Array(pages)) => pages
                .get_mut(index)
                .and_then(Value::as_inline_table_mut)
                .map(|t| t as &mut dyn TableLike)
                .ok_or_else(|| shape(format!("page {page}"))),
            _ => Err(shape("pages".into())),
        }
    }

    /// The table of key `slot` on page `page` in `doc`, created when the
    /// page has none, and whether it is inline.
    fn key_table<'d>(
        &self,
        doc: &'d mut DocumentMut,
        page: usize,
        slot: KeySlot,
    ) -> Result<(&'d mut dyn TableLike, bool), EditError> {
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
        child(keys, inline, &name, false).ok_or_else(|| shape(format!("page {page} key {name}")))
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

/// Moves page `from` of `pages` to `to` (both from 0), one neighbour at a
/// time.
fn move_page(pages: &mut ArrayOfTables, from: usize, to: usize) {
    let mut at = from;
    while at < to {
        swap_pages(pages, at);
        at += 1;
    }
    while at > to {
        swap_pages(pages, at - 1);
        at -= 1;
    }
}

/// Swaps pages `at` and `at + 1`. The text follows the place of each table
/// in the file, not the array, so the two pages' tables trade their places:
/// the second page's tables take the first places of the pair, in their
/// order, the first page's the rest.
fn swap_pages(pages: &mut ArrayOfTables, at: usize) {
    let (Some(first), Some(second)) = (
        pages.get_mut(at).map(places),
        pages.get_mut(at + 1).map(places),
    ) else {
        return;
    };
    let mut slots: Vec<isize> = first.iter().chain(&second).copied().collect();
    slots.sort_unstable();
    let (low, high) = slots.split_at(second.len());
    for (index, own, new) in [(at + 1, &second, low), (at, &first, high)] {
        let mut own = own.clone();
        own.sort_unstable();
        let moved: BTreeMap<isize, isize> = own.into_iter().zip(new.iter().copied()).collect();
        if let Some(table) = pages.get_mut(index) {
            visit_tables(table, &mut |table| {
                if let Some(place) = table.position().and_then(|old| moved.get(&old)) {
                    table.set_position(Some(*place));
                }
            });
        }
    }
    let table = pages.remove(at + 1);
    pages.insert(at, table);
}

/// The places in the file of `page`'s tables that have one.
fn places(page: &mut Table) -> Vec<isize> {
    let mut places = Vec::new();
    visit_tables(page, &mut |table| places.extend(table.position()));
    places
}

/// Calls `f` on `table` and every table under it.
fn visit_tables(table: &mut Table, f: &mut dyn FnMut(&mut Table)) {
    f(table);
    for (_, item) in table.iter_mut() {
        match item {
            Item::Table(child) => visit_tables(child, f),
            Item::ArrayOfTables(children) => {
                for child in children.iter_mut() {
                    visit_tables(child, f);
                }
            }
            _ => {}
        }
    }
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
        Action::Launch { cmd, .. } => ("launch", "cmd", cmd.clone()),
    }
}

/// The `class` and `wait_focus` values of a `launch`, each `None` at its
/// default (absent, `true`); both `None` for the other actions.
fn launch_values(action: &Action) -> [(&'static str, Option<Value>); 2] {
    let (class, wait_focus) = match action {
        Action::Launch {
            class, wait_focus, ..
        } => (
            class.as_deref().map(Value::from),
            (!wait_focus).then(|| false.into()),
        ),
        Action::Chord { .. } | Action::Exec { .. } => (None, None),
    };
    [("class", class), ("wait_focus", wait_focus)]
}

/// Writes the binding `name` (`dial` or `roller`) of `parent`, an inline
/// table when `inline`, from `before` to `fields`: only what differs is
/// touched, a short-form chord stays short while it stays a chord, a new
/// binding is an inline table. A binding left without fields is removed,
/// unless `keep_empty`. `None` when `name` is there and not a table.
fn write_rotary(
    parent: &mut dyn TableLike,
    inline: bool,
    name: &str,
    before: &RotaryFields,
    fields: &RotaryFields,
    keep_empty: bool,
) -> Option<()> {
    if !parent.contains_key(name) {
        parent.insert(name, Item::Value(Value::InlineTable(InlineTable::new())));
    }
    let is_inline = inline || parent.get(name).is_some_and(Item::is_inline_table);
    let table = parent.get_mut(name).and_then(Item::as_table_like_mut)?;
    let end = is_inline.then(|| inline_end(table)).flatten();
    if before.cw != fields.cw {
        set_action(table, "cw", before.cw.as_ref(), fields.cw.as_ref());
    }
    if before.ccw != fields.ccw {
        set_action(table, "ccw", before.ccw.as_ref(), fields.ccw.as_ref());
    }
    if before.label != fields.label {
        set_value(table, "label", fields.label.as_deref().map(Value::from));
    }
    if before.step != fields.step {
        set_value(
            table,
            "step",
            fields.step.map(|step| i64::from(step).into()),
        );
    }
    if let Some(end) = end {
        end.restore(table);
    }
    if table.is_empty() && !keep_empty {
        parent.remove(name);
    }
    Some(())
}

/// `[dialpad]` without anything in it is removed.
fn drop_empty_dialpad(doc: &mut DocumentMut) {
    if doc
        .get("dialpad")
        .and_then(Item::as_table_like)
        .is_some_and(TableLike::is_empty)
    {
        doc.remove("dialpad");
    }
}

/// `action` as a new inline table.
fn action_table(action: &Action) -> InlineTable {
    let (kind, field, value) = parts(action);
    let mut table = InlineTable::new();
    table.insert("type", kind.into());
    table.insert(field, value.into());
    for (name, value) in launch_values(action) {
        if let Some(value) = value {
            table.insert(name, value);
        }
    }
    table
}

/// Changes the action `name` of `key` (a key's `action`, a roller's `cw`)
/// from `before` to `after`. An action of the same type keeps its table and
/// only its value changes; another type replaces the table's entries in
/// place; a new action is an inline table. The short form of a roller (`cw
/// = "ctrl+]"`) stays a string while the action stays a chord. A `launch`
/// writes `class` and `wait_focus` only where they differ from the
/// defaults.
fn set_action(
    key: &mut dyn TableLike,
    name: &str,
    before: Option<&Action>,
    after: Option<&Action>,
) {
    let Some(after) = after else {
        key.remove(name);
        return;
    };
    if key.get(name).and_then(Item::as_str).is_some() {
        let value = match after {
            Action::Chord { keys } => keys.to_string().into(),
            _ => Value::InlineTable(action_table(after)),
        };
        set_value(key, name, Some(value));
        return;
    }
    let (kind, field, value) = parts(after);
    let inline = key.get(name).is_some_and(Item::is_inline_table);
    let Some(table) = key.get_mut(name).and_then(Item::as_table_like_mut) else {
        set_value(key, name, Some(Value::InlineTable(action_table(after))));
        return;
    };
    let end = inline.then(|| inline_end(table)).flatten();
    let old = before.map(parts);
    if old.as_ref().map(|(kind, ..)| *kind) != Some(kind) {
        set_value(table, "type", Some(kind.into()));
        // `exec` and `launch` share `cmd`; it keeps its place and decor.
        if let Some((_, old_field, _)) = old.filter(|(_, old_field, _)| *old_field != field) {
            table.remove(old_field);
        }
    }
    set_value(table, field, Some(value.into()));
    for (name, value) in launch_values(after) {
        set_value(table, name, value);
    }
    if let Some(end) = end {
        end.restore(table);
    }
}

/// The space before the `}` of an inline table, which `toml_edit` keeps as
/// the suffix of the last value: an entry added or removed at the end
/// would take it along.
struct InlineEnd {
    last: String,
    suffix: toml_edit::RawString,
}

fn inline_end(table: &dyn TableLike) -> Option<InlineEnd> {
    let (last, item) = table.iter().last()?;
    let value = item.as_value()?;
    Some(InlineEnd {
        last: last.to_owned(),
        suffix: value.decor().suffix()?.clone(),
    })
}

impl InlineEnd {
    /// Moves the space to the value that is last now.
    fn restore(self, table: &mut dyn TableLike) {
        if let Some(old) = table.get_mut(&self.last).and_then(Item::as_value_mut) {
            old.decor_mut().set_suffix("");
        }
        if let Some((_, item)) = table.iter_mut().last()
            && let Some(value) = item.as_value_mut()
        {
            value.decor_mut().set_suffix(self.suffix);
        }
    }
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
    fn a_launch_keeps_its_window_fields_only_while_they_differ_from_the_defaults() {
        let mut doc = document(&format!(
            "{HEADER}\n[[pages]]\n\n[pages.keys.1]\n\
             action = {{ type = \"launch\", cmd = \"kcalc\", class = \"kcalc\", wait_focus = false }}\n"
        ));
        let line = |doc: &ProfileDocument| doc.to_string().lines().last().map(str::to_owned);
        let action = |action: Action| KeyFields {
            action: Some(action),
            ..KeyFields::default()
        };
        doc.set_key(1, slot(1), &action(Action::launch("kcalc")))
            .unwrap();
        assert_eq!(
            line(&doc).as_deref(),
            Some("action = { type = \"launch\", cmd = \"kcalc\" }")
        );
        let waiting = Action::Launch {
            cmd: "kcalc".into(),
            class: Some("org.kde.kcalc".into()),
            wait_focus: true,
        };
        doc.set_key(1, slot(1), &action(waiting.clone())).unwrap();
        assert_eq!(
            line(&doc).as_deref(),
            Some("action = { type = \"launch\", cmd = \"kcalc\", class = \"org.kde.kcalc\" }")
        );
        assert_eq!(doc.key(1, slot(1)).unwrap().action, Some(waiting));
        // Another type drops the launch's own fields.
        doc.set_key(
            1,
            slot(1),
            &action(Action::Exec {
                cmd: "kcalc".into(),
            }),
        )
        .unwrap();
        assert_eq!(
            line(&doc).as_deref(),
            Some("action = { type = \"exec\", cmd = \"kcalc\" }")
        );
    }

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
