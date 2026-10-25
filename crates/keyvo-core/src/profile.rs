//! The profile model: one `profiles/<id>.toml` file (spec §7.2), schema
//! version 1, pre-alpha subset.
//!
//! Every value is validated while it is deserialized, so a bad value is
//! reported with the position of the value itself. Unknown keys are errors:
//! a typo never silently disables a key.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::{self, Deserializer, Unexpected, Visitor};
use serde::{Deserialize, Serialize};

use crate::chord::Chord;
use crate::diagnostic::Diagnostic;
use crate::focus::WindowContext;

/// The id of the profile that always exists and that app profiles overlay.
pub const GLOBAL_ID: &str = "global";

/// The profile schema version this build reads and writes.
pub const SCHEMA_VERSION: u32 = 1;

/// A whole profile file.
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(title = "keyvo profile")]
pub struct Profile {
    /// Version of the profile format; this build reads `1`.
    pub schema_version: SchemaVersion,
    /// Identity, window match rules and look of the profile.
    pub profile: ProfileInfo,
    /// The pages of the profile, in order.
    #[serde(default)]
    pub pages: Vec<Page>,
}

impl Profile {
    /// Parses and validates the text of a profile file; `path` only labels
    /// the diagnostic.
    ///
    /// # Errors
    ///
    /// A [`Diagnostic`] with the position of the first problem.
    pub fn from_toml(text: &str, path: &Path) -> Result<Self, Diagnostic> {
        toml::from_str(text).map_err(|e| Diagnostic::from_toml(path, text, &e))
    }

    /// The built-in global profile used when `profiles/global.toml` does not
    /// exist: one empty page, no match rules.
    #[must_use]
    pub fn builtin_global() -> Self {
        Self {
            schema_version: SchemaVersion,
            profile: ProfileInfo {
                id: GLOBAL_ID.to_owned(),
                name: "Global".to_owned(),
                r#match: Match::default(),
                accent: None,
                priority: 0,
            },
            pages: vec![Page {
                name: Some("context".to_owned()),
                keys: BTreeMap::new(),
            }],
        }
    }
}

/// The `[profile]` table.
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProfileInfo {
    /// The profile id; equals the file name without `.toml`.
    #[serde(deserialize_with = "profile_id")]
    #[schemars(regex(pattern = r"^[a-z0-9][a-z0-9._-]*$"))]
    pub id: String,
    /// Display name.
    pub name: String,
    /// Which focused windows select this profile.
    #[serde(default)]
    pub r#match: Match,
    /// Accent color, the primary context indicator on the pad.
    pub accent: Option<Color>,
    /// Higher wins when several profiles match; never alphabetical.
    #[serde(default)]
    pub priority: i32,
}

/// Window match rules of a profile (spec §6.2).
#[derive(Debug, Clone, Default, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Match {
    /// Window classes, reverse-DNS (`org.kde.konsole`) or short (`konsole`).
    #[serde(default)]
    pub class: Vec<String>,
    /// Regular expression on the window title.
    pub title: Option<TitleRegex>,
}

impl Match {
    /// Whether `window` satisfies these rules: one of the classes, when
    /// there are any, and the title regex, when there is one. Rules without
    /// either never match. A class matches case-insensitively, the whole
    /// name or its short form: `konsole` matches the window class
    /// `org.kde.konsole` and the other way round.
    #[must_use]
    pub fn matches(&self, window: &WindowContext) -> bool {
        if self.class.is_empty() && self.title.is_none() {
            return false;
        }
        let class = self.class.is_empty()
            || self
                .class
                .iter()
                .any(|rule| class_matches(rule, &window.class));
        class
            && self
                .title
                .as_ref()
                .is_none_or(|title| title.is_match(&window.title))
    }

    /// Whether a title regex narrows the rules, which makes them more
    /// specific than a class alone.
    #[must_use]
    pub const fn has_title(&self) -> bool {
        self.title.is_some()
    }
}

/// `rule` and `class` are the same name, or one is the last `.` segment
/// of the other.
fn class_matches(rule: &str, class: &str) -> bool {
    fn short(name: &str) -> &str {
        name.rsplit('.').next().unwrap_or(name)
    }
    rule.eq_ignore_ascii_case(class)
        || (!rule.contains('.') && rule.eq_ignore_ascii_case(short(class)))
        || (!class.contains('.') && class.eq_ignore_ascii_case(short(rule)))
}

/// One `[[pages]]` entry.
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Page {
    /// Page name, for example `context` or `general`.
    pub name: Option<String>,
    /// The keys of the page by number; absent keys are blank.
    #[serde(default)]
    #[schemars(schema_with = "key_map_schema")]
    pub keys: BTreeMap<KeySlot, Key>,
}

/// One LCD key.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Key {
    /// Icon reference, for example `lucide:hammer` or `file:icons/x.png`.
    pub icon: Option<String>,
    /// Text drawn on the key.
    pub label: Option<String>,
    /// Background color.
    pub color: Option<Color>,
    /// What a press does; a key without an action only displays.
    pub action: Option<Action>,
    /// Named looks that replace parts of the key while the state is active.
    #[serde(default)]
    pub states: BTreeMap<String, KeyState>,
}

/// The overrides of one key state.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KeyState {
    /// Icon in this state.
    pub icon: Option<String>,
    /// Label in this state.
    pub label: Option<String>,
    /// Background color in this state.
    pub color: Option<Color>,
    /// Corner badge text in this state.
    pub badge: Option<String>,
}

/// What a key press does (pre-alpha: `chord` and `exec`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum Action {
    /// Send a key combination, for example `ctrl+shift+g`.
    Chord {
        /// The chord.
        keys: Chord,
    },
    /// Run a command without blocking, with `KEYVO_*` in its environment.
    Exec {
        /// The command line.
        cmd: String,
    },
}

impl Action {
    /// Whether the key shows the automatic `running`, `success` and
    /// `error` states while and after the action runs (spec §7.2): `exec`
    /// does, a chord is instant and does not.
    #[must_use]
    pub fn has_feedback(&self) -> bool {
        matches!(self, Self::Exec { .. })
    }
}

impl<'de> Deserialize<'de> for Action {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // A tagged enum buffers its fields and the values lose their
        // position; visiting the map keeps it, so a bad chord points at
        // `keys` itself. Keys may come in any order: a bad value is
        // reported where it is, an unknown key after the whole table.
        #[derive(Deserialize)]
        #[serde(rename_all = "lowercase")]
        enum Type {
            Chord,
            Exec,
        }

        struct ActionVisitor;

        impl<'de> Visitor<'de> for ActionVisitor {
            type Value = Action;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an action table")
            }

            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Action, A::Error> {
                const FIELDS: &[&str] = &["type", "keys", "cmd"];
                let (mut kind, mut keys, mut cmd, mut unknown) = (None, None, None, None);
                while let Some(name) = map.next_key::<String>()? {
                    match name.as_str() {
                        "type" => kind = Some(map.next_value::<Type>()?),
                        "keys" => keys = Some(map.next_value::<Chord>()?),
                        "cmd" => cmd = Some(map.next_value::<String>()?),
                        _ => {
                            map.next_value::<de::IgnoredAny>()?;
                            unknown.get_or_insert(name);
                        }
                    }
                }
                if let Some(name) = unknown {
                    return Err(de::Error::unknown_field(&name, FIELDS));
                }
                match (kind, keys, cmd) {
                    (None, _, _) => Err(de::Error::missing_field("type")),
                    (Some(Type::Chord), Some(keys), None) => Ok(Action::Chord { keys }),
                    (Some(Type::Exec), None, Some(cmd)) => Ok(Action::Exec { cmd }),
                    (Some(Type::Chord), _, Some(_)) => {
                        Err(de::Error::unknown_field("cmd", &["type", "keys"]))
                    }
                    (Some(Type::Exec), Some(_), _) => {
                        Err(de::Error::unknown_field("keys", &["type", "cmd"]))
                    }
                    (Some(Type::Chord), None, None) => Err(de::Error::missing_field("keys")),
                    (Some(Type::Exec), None, None) => Err(de::Error::missing_field("cmd")),
                }
            }
        }

        deserializer.deserialize_map(ActionVisitor)
    }
}

/// The schema version marker; only [`SCHEMA_VERSION`] deserializes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaVersion;

impl<'de> Deserialize<'de> for SchemaVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let version = u32::deserialize(deserializer)?;
        if version == SCHEMA_VERSION {
            Ok(Self)
        } else {
            Err(de::Error::custom(format!(
                "unsupported schema_version {version}, this keyvo reads {SCHEMA_VERSION}"
            )))
        }
    }
}

impl JsonSchema for SchemaVersion {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        "SchemaVersion".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({ "type": "integer", "const": SCHEMA_VERSION })
    }
}

fn profile_id<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let id = String::deserialize(deserializer)?;
    let mut chars = id.chars();
    let valid_first = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    let valid_rest =
        chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
    if valid_first && valid_rest {
        Ok(id)
    } else {
        Err(de::Error::invalid_value(
            Unexpected::Str(&id),
            &"an id of lowercase letters, digits, `.`, `_` and `-`, starting with a letter or digit",
        ))
    }
}

/// The number of a key, 1 to 9, row-major, top-left is 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct KeySlot(u8);

impl KeySlot {
    /// The slot `n`, or `None` outside 1..=9.
    #[must_use]
    pub fn new(n: u8) -> Option<Self> {
        (1..=9).contains(&n).then_some(Self(n))
    }

    /// The key number, 1 to 9.
    #[must_use]
    pub fn get(self) -> u8 {
        self.0
    }
}

impl<'de> Deserialize<'de> for KeySlot {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SlotVisitor;

        impl Visitor<'_> for SlotVisitor {
            type Value = KeySlot;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a key number from 1 to 9")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<KeySlot, E> {
                let mut bytes = v.bytes();
                match (bytes.next(), bytes.next()) {
                    (Some(b @ b'1'..=b'9'), None) => Ok(KeySlot(b - b'0')),
                    _ => Err(E::invalid_value(Unexpected::Str(v), &self)),
                }
            }
        }

        deserializer.deserialize_str(SlotVisitor)
    }
}

fn key_map_schema(generator: &mut SchemaGenerator) -> Schema {
    let key = generator.subschema_for::<Key>();
    json_schema!({
        "type": "object",
        "patternProperties": { "^[1-9]$": key },
        "additionalProperties": false
    })
}

/// An RGB color, written `#rrggbb` (either case).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
}

impl Color {
    /// The color with these components.
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    fn parse(s: &str) -> Option<Self> {
        let hex = s.strip_prefix('#')?;
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        Some(Self::rgb(channel(0)?, channel(2)?, channel(4)?))
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).ok_or_else(|| {
            de::Error::invalid_value(Unexpected::Str(&s), &"a color written #rrggbb")
        })
    }
}

impl JsonSchema for Color {
    fn schema_name() -> Cow<'static, str> {
        "Color".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "An RGB color, written #rrggbb (either case).",
            "type": "string",
            "pattern": "^#[0-9a-fA-F]{6}$"
        })
    }
}

/// A compiled window-title regular expression.
#[derive(Debug, Clone)]
pub struct TitleRegex(regex::Regex);

impl TitleRegex {
    /// Whether `title` contains a match.
    #[must_use]
    pub fn is_match(&self, title: &str) -> bool {
        self.0.is_match(title)
    }

    /// The source of the expression as written in the file.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl PartialEq for TitleRegex {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl<'de> Deserialize<'de> for TitleRegex {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        regex::Regex::new(&s).map(Self).map_err(|e| {
            // The syntax error is a multi-line caret diagram; its last
            // line names the problem, the diagnostic carries the place.
            let text = e.to_string();
            let reason = text.lines().last().unwrap_or_default();
            let reason = reason.strip_prefix("error: ").unwrap_or(reason);
            de::Error::custom(format!("invalid title regex: {reason}"))
        })
    }
}

impl JsonSchema for TitleRegex {
    fn schema_name() -> Cow<'static, str> {
        "TitleRegex".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "A regular expression (Rust regex syntax) searched in the window title.",
            "type": "string",
            "format": "regex"
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_slot_accepts_only_one_to_nine() {
        assert_eq!(KeySlot::new(0), None);
        assert_eq!(KeySlot::new(1).map(KeySlot::get), Some(1));
        assert_eq!(KeySlot::new(9).map(KeySlot::get), Some(9));
        assert_eq!(KeySlot::new(10), None);
    }

    #[test]
    fn color_round_trips_through_display() {
        let c = Color::parse("#4E8bD6").unwrap();
        assert_eq!(c, Color::rgb(0x4e, 0x8b, 0xd6));
        assert_eq!(c.to_string(), "#4e8bd6");
        assert_eq!(Color::parse("4e8bd6"), None);
        assert_eq!(Color::parse("#4e8bd"), None);
        assert_eq!(Color::parse("#4e8bdg"), None);
        assert_eq!(Color::parse("#é4e8b"), None);
    }

    #[test]
    fn builtin_global_has_one_empty_page_and_no_match() {
        let global = Profile::builtin_global();
        assert_eq!(global.profile.id, GLOBAL_ID);
        assert_eq!(global.profile.r#match, Match::default());
        assert_eq!(global.pages.len(), 1);
        assert!(global.pages[0].keys.is_empty());
    }
}
