//! The chord parser (spec §7.2): `ctrl+shift+g`, `ctrl+?`, `KEY_F13` or a
//! sequence `ctrl+k, ctrl+s` becomes evdev key codes an `InputInjector`
//! presses in order.
//!
//! Grammar: steps separated by `,`; a step is modifiers joined with `+` and
//! one key last. Names are case-insensitive and may have spaces around them.
//! Where a key is expected, `+` and `,` are the keys themselves (`ctrl++`,
//! `ctrl+,`). Modifiers: `ctrl` (`control`), `shift`, `alt`, `altgr`,
//! `super` (`meta`); a modifier alone is pressed as its key. A key is a
//! symbol, a friendly alias (`escape`, `pgup`) or any evdev name with or
//! without its `KEY_` prefix. Shifted symbols (`?`, `+`, `{`) add `shift`
//! and press the unshifted key of the US layout.

mod keycodes;

use std::borrow::Cow;
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize, Serializer};

/// An evdev key code (`linux/input-event-codes.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct KeyCode(u16);

impl KeyCode {
    /// The code `code`.
    #[must_use]
    pub const fn new(code: u16) -> Self {
        Self(code)
    }

    /// The code as a number.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }

    /// The code of an evdev name, case-insensitive, with or without the
    /// `KEY_` prefix: `leftctrl`, `KEY_F13`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        let name = name.to_ascii_lowercase();
        let name = name.strip_prefix("key_").unwrap_or(&name);
        keycodes::KEYCODES
            .binary_search_by(|(entry, _)| (*entry).cmp(name))
            .ok()
            .map(|index| Self(keycodes::KEYCODES[index].1))
    }

    /// Every code of the table, ascending, each once: the keys an
    /// `InputInjector` device declares so it can press any chord.
    pub fn all() -> impl Iterator<Item = Self> {
        let mut codes: Vec<u16> = keycodes::KEYCODES.iter().map(|(_, code)| *code).collect();
        codes.sort_unstable();
        codes.dedup();
        codes.into_iter().map(Self)
    }
}

/// One edge of a key: pressed (`down`) or released.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct KeyEvent {
    /// The key.
    pub code: KeyCode,
    /// `true` for a press, `false` for a release.
    pub down: bool,
}

/// One modifier key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Modifier {
    Ctrl,
    Shift,
    Alt,
    AltGr,
    Super,
}

impl Modifier {
    /// In press order.
    const ALL: [Self; 5] = [Self::Ctrl, Self::Shift, Self::Alt, Self::AltGr, Self::Super];

    fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => Some(Self::Ctrl),
            "shift" => Some(Self::Shift),
            "alt" => Some(Self::Alt),
            "altgr" => Some(Self::AltGr),
            "super" | "meta" => Some(Self::Super),
            _ => None,
        }
    }

    const fn code(self) -> KeyCode {
        KeyCode(match self {
            Self::Ctrl => 29,
            Self::Shift => 42,
            Self::Alt => 56,
            Self::AltGr => 100,
            Self::Super => 125,
        })
    }

    const fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// The modifiers held during one step.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers(u8);

impl Modifiers {
    fn with(self, modifier: Modifier) -> Self {
        Self(self.0 | modifier.bit())
    }

    /// The key codes of the held modifiers, in press order: ctrl, shift,
    /// alt, altgr, super (left keys, altgr is the right alt).
    pub fn codes(self) -> impl DoubleEndedIterator<Item = KeyCode> {
        Modifier::ALL
            .into_iter()
            .filter(move |m| self.0 & m.bit() != 0)
            .map(Modifier::code)
    }
}

/// One step of a chord: modifiers held around one key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    modifiers: Modifiers,
    key: KeyCode,
}

impl Step {
    /// The held modifiers.
    #[must_use]
    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    /// The key pressed while they are held.
    #[must_use]
    pub fn key(&self) -> KeyCode {
        self.key
    }

    fn push_events(self, events: &mut Vec<KeyEvent>) {
        let edge = |code, down| KeyEvent { code, down };
        events.extend(self.modifiers.codes().map(|code| edge(code, true)));
        events.push(edge(self.key, true));
        events.push(edge(self.key, false));
        events.extend(self.modifiers.codes().rev().map(|code| edge(code, false)));
    }
}

/// Why a chord does not parse.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChordError {
    /// Nothing but spaces.
    #[error("empty chord")]
    Empty,
    /// A `+` or `,` at the end.
    #[error("a `+` or `,` must be followed by a key")]
    MissingKey,
    /// A name that is no key.
    #[error("unknown key `{0}`")]
    UnknownKey(String),
    /// A key before a `+`.
    #[error("`{0}` is not a modifier (ctrl, shift, alt, altgr, super)")]
    NotAModifier(String),
    /// Something other than `+`, `,` or the end after a `+` or `,` key.
    #[error("expected `+`, `,` or the end after `{0}`")]
    ExpectedSeparator(String),
}

/// A parsed chord with the text it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chord {
    source: String,
    steps: Vec<Step>,
}

impl Chord {
    /// Parses `text`.
    ///
    /// # Errors
    ///
    /// A [`ChordError`] naming the first problem.
    pub fn parse(text: &str) -> Result<Self, ChordError> {
        if text.trim().is_empty() {
            return Err(ChordError::Empty);
        }
        let mut steps = Vec::new();
        let mut names = Vec::new();
        let mut rest = text;
        loop {
            let (name, after) = next_name(rest)?;
            names.push(name);
            match after.chars().next() {
                None => {
                    steps.push(step(&names)?);
                    break;
                }
                Some('+') => rest = &after[1..],
                Some(',') => {
                    steps.push(step(&names)?);
                    names.clear();
                    rest = &after[1..];
                }
                Some(_) => return Err(ChordError::ExpectedSeparator(name.to_owned())),
            }
        }
        Ok(Self {
            source: text.to_owned(),
            steps,
        })
    }

    /// The text the chord was parsed from.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.source
    }

    /// The steps, in order.
    #[must_use]
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// Every press and release of the chord, in order: per step the
    /// modifiers, the key, then the releases in reverse.
    #[must_use]
    pub fn events(&self) -> Vec<KeyEvent> {
        let mut events = Vec::new();
        for step in &self.steps {
            step.push_events(&mut events);
        }
        events
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.source)
    }
}

/// The next name in `text` and what follows it. A `+` or `,` where a name
/// starts is the name itself.
fn next_name(text: &str) -> Result<(&str, &str), ChordError> {
    let text = text.trim_start();
    match text.chars().next() {
        None => Err(ChordError::MissingKey),
        Some(c @ ('+' | ',')) => {
            let (name, after) = text.split_at(c.len_utf8());
            Ok((name, after.trim_start()))
        }
        Some(_) => {
            let end = text.find(['+', ',']).unwrap_or(text.len());
            let (name, after) = text.split_at(end);
            Ok((name.trim_end(), after))
        }
    }
}

/// One step from its names: modifiers, then the key.
fn step(names: &[&str]) -> Result<Step, ChordError> {
    let Some((last, modifier_names)) = names.split_last() else {
        return Err(ChordError::MissingKey);
    };
    let mut modifiers = Modifiers::default();
    for name in modifier_names {
        let modifier = Modifier::from_name(name)
            .ok_or_else(|| ChordError::NotAModifier((*name).to_owned()))?;
        modifiers = modifiers.with(modifier);
    }
    let (shift, key) = key(last).ok_or_else(|| ChordError::UnknownKey((*last).to_owned()))?;
    if shift {
        modifiers = modifiers.with(Modifier::Shift);
    }
    Ok(Step { modifiers, key })
}

/// The key of `name` and whether it needs `shift`.
fn key(name: &str) -> Option<(bool, KeyCode)> {
    if let Some(modifier) = Modifier::from_name(name) {
        return Some((false, modifier.code()));
    }
    let mut chars = name.chars();
    if let (Some(c), None) = (chars.next(), chars.next())
        && let Some(found) = symbol(c)
    {
        return Some(found);
    }
    let evdev = match name.to_ascii_lowercase().as_str() {
        "escape" => "esc",
        "return" => "enter",
        "del" => "delete",
        "ins" => "insert",
        "pgup" => "pageup",
        "pgdn" => "pagedown",
        "period" => "dot",
        "quote" => "apostrophe",
        "backtick" => "grave",
        _ => name,
    };
    KeyCode::from_name(evdev).map(|code| (false, code))
}

/// A symbol on the US layout: whether it needs `shift`, and its key.
fn symbol(c: char) -> Option<(bool, KeyCode)> {
    let (shift, name) = match c {
        '-' => (false, "minus"),
        '=' => (false, "equal"),
        '[' => (false, "leftbrace"),
        ']' => (false, "rightbrace"),
        '\\' => (false, "backslash"),
        ';' => (false, "semicolon"),
        '\'' => (false, "apostrophe"),
        '`' => (false, "grave"),
        ',' => (false, "comma"),
        '.' => (false, "dot"),
        '/' => (false, "slash"),
        '!' => (true, "1"),
        '@' => (true, "2"),
        '#' => (true, "3"),
        '$' => (true, "4"),
        '%' => (true, "5"),
        '^' => (true, "6"),
        '&' => (true, "7"),
        '*' => (true, "8"),
        '(' => (true, "9"),
        ')' => (true, "0"),
        '_' => (true, "minus"),
        '+' => (true, "equal"),
        '{' => (true, "leftbrace"),
        '}' => (true, "rightbrace"),
        '|' => (true, "backslash"),
        ':' => (true, "semicolon"),
        '"' => (true, "apostrophe"),
        '~' => (true, "grave"),
        '<' => (true, "comma"),
        '>' => (true, "dot"),
        '?' => (true, "slash"),
        _ => return None,
    };
    KeyCode::from_name(name).map(|code| (shift, code))
}

impl<'de> Deserialize<'de> for Chord {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(|e| de::Error::custom(format!("invalid chord: {e}")))
    }
}

impl Serialize for Chord {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.source)
    }
}

impl JsonSchema for Chord {
    fn schema_name() -> Cow<'static, str> {
        "Chord".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "Modifiers and a key joined with `+`, steps separated by `,`: `ctrl+shift+g`, `ctrl+?`, `KEY_F13`, `ctrl+k, ctrl+s`.",
            "type": "string",
            "minLength": 1
        })
    }
}
