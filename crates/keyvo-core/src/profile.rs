//! The profile model: one `profiles/<id>.toml` file (spec §7.2), schema
//! version 1, pre-alpha subset.
//!
//! Every value is validated while it is deserialized, so a bad value is
//! reported with the position of the value itself. Unknown keys are errors:
//! a typo never silently disables a key.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::num::NonZeroU32;
use std::path::Path;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::{self, Deserializer, Unexpected, Visitor};
use serde::{Deserialize, Serialize};

use crate::chord::Chord;
use crate::diagnostic::Diagnostic;
use crate::dialpad::{ButtonSlot, Roller};
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
    /// The Dialpad's bindings; the global profile's table fills in every
    /// control an app profile leaves out.
    pub dialpad: Option<Dialpad>,
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
            dialpad: None,
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
pub(crate) fn class_matches(rule: &str, class: &str) -> bool {
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
#[derive(Debug, Clone, Default, PartialEq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct Key {
    /// Icon reference, for example `lucide:hammer` or `file:icons/x.png`.
    pub icon: Option<String>,
    /// Text drawn on the key.
    pub label: Option<String>,
    /// Background color.
    pub color: Option<Color>,
    /// What a press does; a key without an action only displays. On a
    /// held modifier key it runs at a release without a turn, on a toggle
    /// key at a long press.
    pub action: Option<Action>,
    /// An action run once when `action` fails (spec §7.2); it has no
    /// `on_fail` of its own.
    pub on_fail: Option<Action>,
    /// Named looks that replace parts of the key while the state is active.
    #[schemars(default)]
    pub states: BTreeMap<String, KeyState>,
    /// How the key takes the Dialpad's dial and roller over (spec §7.2):
    /// `held` while it is held, the default with a `dial` or `roller`
    /// table, or `toggle` from one short press to the next press of any
    /// key. Needs a `dial` or `roller` table.
    pub modifier: Option<ModifierMode>,
    /// What the dial runs while the key is the active modifier, in place of
    /// the profile's `[dialpad].dial`.
    pub dial: Option<Rotary>,
    /// What the roller runs while the key is the active modifier, in place
    /// of the profile's `[dialpad].roller`.
    pub roller: Option<Rotary>,
}

impl Key {
    /// The key's modifier mode: its `modifier`, else `held` when it has a
    /// `dial` or `roller` table; `None` for an ordinary key.
    #[must_use]
    pub fn modifier_mode(&self) -> Option<ModifierMode> {
        (self.dial.is_some() || self.roller.is_some())
            .then(|| self.modifier.unwrap_or(ModifierMode::Held))
    }

    /// The key's binding of `roller` as a modifier; `None` for an ordinary
    /// key and a roller it leaves to the profile.
    #[must_use]
    pub fn rotary(&self, roller: Roller) -> Option<&Rotary> {
        self.modifier_mode()?;
        match roller {
            Roller::Dial => self.dial.as_ref(),
            Roller::Roller => self.roller.as_ref(),
        }
    }
}

/// The fields of a key as written; [`Key`] checks the rules between them.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyFields {
    icon: Option<String>,
    label: Option<String>,
    color: Option<Color>,
    action: Option<Action>,
    on_fail: Option<Action>,
    #[serde(default)]
    states: BTreeMap<String, KeyState>,
    modifier: Option<ModifierMode>,
    dial: Option<Rotary>,
    roller: Option<Rotary>,
}

impl<'de> Deserialize<'de> for Key {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let fields = KeyFields::deserialize(deserializer)?;
        // Failing after the whole table puts the diagnostic on the key.
        if fields.modifier.is_some() && fields.dial.is_none() && fields.roller.is_none() {
            return Err(de::Error::custom(
                "`modifier` needs a `dial` or `roller` table next to it",
            ));
        }
        Ok(Self {
            icon: fields.icon,
            label: fields.label,
            color: fields.color,
            action: fields.action,
            on_fail: fields.on_fail,
            states: fields.states,
            modifier: fields.modifier,
            dial: fields.dial,
            roller: fields.roller,
        })
    }
}

/// How a modifier key takes the dial and the roller over (spec §7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ModifierMode {
    /// While the key is held; its action runs at a release without a turn.
    Held,
    /// From a short press until the next press of any key, a Dialpad
    /// button, a page or profile change, the lock or
    /// `[timing].modifier_timeout`; its action runs at a long press.
    Toggle,
}

impl ModifierMode {
    /// The name in the profile, the socket and the trace.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Held => "held",
            Self::Toggle => "toggle",
        }
    }
}

/// The `[dialpad]` table (spec §7.2): the dial, the roller and the four
/// buttons of the MX Creative Dialpad.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Dialpad {
    /// The large dial.
    pub dial: Option<Rotary>,
    /// The small roller.
    pub roller: Option<Rotary>,
    /// The buttons by number, 1 to 4 in CID order.
    #[serde(default)]
    #[schemars(schema_with = "button_map_schema")]
    pub buttons: BTreeMap<ButtonSlot, DialButton>,
}

/// The binding of the dial or the roller.
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Rotary {
    /// What a clockwise turn runs (the roller: forward); a string is a
    /// chord.
    #[serde(default, deserialize_with = "short_action")]
    #[schemars(schema_with = "short_action_schema")]
    pub cw: Option<Action>,
    /// What a counter-clockwise turn runs (the roller: back); a string is a
    /// chord.
    #[serde(default, deserialize_with = "short_action")]
    #[schemars(schema_with = "short_action_schema")]
    pub ccw: Option<Action>,
    /// Detents per run of the action; a fast turn's larger delta counts as
    /// that many detents. Default 1.
    #[serde(default = "one")]
    pub step: NonZeroU32,
    /// What the app's Dialpad panel shows.
    pub label: Option<String>,
}

impl Default for Rotary {
    fn default() -> Self {
        Self {
            cw: None,
            ccw: None,
            step: one(),
            label: None,
        }
    }
}

const fn one() -> NonZeroU32 {
    NonZeroU32::MIN
}

/// A Dialpad button: a key without an icon or a colour.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DialButton {
    /// What the app's Dialpad panel shows.
    pub label: Option<String>,
    /// What a press does.
    pub action: Option<Action>,
    /// An action run once when `action` fails.
    pub on_fail: Option<Action>,
}

/// `cw = "ctrl+]"` as a chord, or a whole action table.
fn short_action<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Action>, D::Error> {
    struct ShortVisitor;

    impl<'de> Visitor<'de> for ShortVisitor {
        type Value = Action;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a chord string or an action table")
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Action, E> {
            Chord::deserialize(de::value::StrDeserializer::<E>::new(v))
                .map(|keys| Action::Chord { keys })
        }

        fn visit_map<A: de::MapAccess<'de>>(self, map: A) -> Result<Action, A::Error> {
            Action::deserialize(de::value::MapAccessDeserializer::new(map))
        }
    }

    deserializer.deserialize_any(ShortVisitor).map(Some)
}

fn short_action_schema(generator: &mut SchemaGenerator) -> Schema {
    let action = generator.subschema_for::<Action>();
    json_schema!({
        "anyOf": [
            { "type": "string", "description": "A chord, for example `ctrl+]`." },
            action
        ]
    })
}

fn button_map_schema(generator: &mut SchemaGenerator) -> Schema {
    let button = generator.subschema_for::<DialButton>();
    json_schema!({
        "type": "object",
        "patternProperties": { "^[1-4]$": button },
        "additionalProperties": false
    })
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

/// What a key press does (pre-alpha: `chord`, `exec` and `launch`).
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
    /// Open an application: run a command like `exec`, in its own session,
    /// and let it go once it outlives `[timing].launch_settle`.
    Launch {
        /// The command line.
        cmd: String,
        /// The window class the launch opens, matched like a profile's
        /// `match.class` entry; derived from a `flatpak run <id>` command
        /// when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        class: Option<String>,
        /// Whether the launch's window keeps a global page (spec §6.3).
        /// Default `true`.
        #[serde(default = "yes", skip_serializing_if = "is_yes")]
        wait_focus: bool,
    },
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

/// The window class a `flatpak run <id>` command opens: the app id, which
/// `KWin` reports as the window's `resourceClass`. `None` for any other
/// command, and where the app id is not a valid one (at least three
/// `.`-separated parts).
#[must_use]
pub fn derived_class(cmd: &str) -> Option<String> {
    let mut words = cmd.split_whitespace();
    let program = words.next()?;
    if program.rsplit('/').next() != Some("flatpak") || words.next() != Some("run") {
        return None;
    }
    let id = words.find(|word| !word.starts_with('-'))?;
    (id.split('.').filter(|part| !part.is_empty()).count() >= 3).then(|| id.to_owned())
}

impl Action {
    /// A `launch` of `cmd` without a class, waiting for its window.
    #[must_use]
    pub fn launch(cmd: &str) -> Self {
        Self::Launch {
            cmd: cmd.to_owned(),
            class: None,
            wait_focus: true,
        }
    }

    /// The window class a `launch` opens: its own `class`, else the one
    /// derived from its command ([`derived_class`]); `None` for the other
    /// actions.
    #[must_use]
    pub fn launch_class(&self) -> Option<String> {
        match self {
            Self::Launch { cmd, class, .. } => class.clone().or_else(|| derived_class(cmd)),
            Self::Chord { .. } | Self::Exec { .. } => None,
        }
    }

    /// Whether the key shows the automatic `running`, `success` and
    /// `error` states while and after the action runs (spec §7.2): `exec`
    /// and `launch` do, a chord is instant and does not.
    #[must_use]
    pub fn has_feedback(&self) -> bool {
        matches!(self, Self::Exec { .. } | Self::Launch { .. })
    }

    /// What the action does in a few words: a chord's keys, the command
    /// line of `exec` and `launch`.
    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::Chord { keys } => keys.to_string(),
            Self::Exec { cmd } | Self::Launch { cmd, .. } => cmd.clone(),
        }
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
            Launch,
        }

        struct ActionVisitor;

        impl<'de> Visitor<'de> for ActionVisitor {
            type Value = Action;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an action table")
            }

            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Action, A::Error> {
                const FIELDS: &[&str] = &["type", "keys", "cmd", "class", "wait_focus"];
                let (mut kind, mut keys, mut cmd, mut unknown) = (None, None, None, None);
                let (mut class, mut wait_focus) = (None, None);
                while let Some(name) = map.next_key::<String>()? {
                    match name.as_str() {
                        "type" => kind = Some(map.next_value::<Type>()?),
                        "keys" => keys = Some(map.next_value::<Chord>()?),
                        "cmd" => cmd = Some(map.next_value::<String>()?),
                        "class" => class = Some(map.next_value::<String>()?),
                        "wait_focus" => wait_focus = Some(map.next_value::<bool>()?),
                        // A nested `on_fail`, or one written inside the
                        // key's `action` instead of next to it.
                        "on_fail" => match map.next_value::<NestedOnFail>()? {},
                        _ => {
                            map.next_value::<de::IgnoredAny>()?;
                            unknown.get_or_insert(name);
                        }
                    }
                }
                if let Some(name) = unknown {
                    return Err(de::Error::unknown_field(&name, FIELDS));
                }
                // `class` and `wait_focus` belong to a `launch` alone.
                if let Some(other @ (Type::Chord | Type::Exec)) = &kind {
                    let expected: &[&str] = match other {
                        Type::Chord => &["type", "keys"],
                        _ => &["type", "cmd"],
                    };
                    if class.is_some() {
                        return Err(de::Error::unknown_field("class", expected));
                    }
                    if wait_focus.is_some() {
                        return Err(de::Error::unknown_field("wait_focus", expected));
                    }
                }
                match (kind, keys, cmd) {
                    (None, _, _) => Err(de::Error::missing_field("type")),
                    (Some(Type::Chord), Some(keys), None) => Ok(Action::Chord { keys }),
                    (Some(Type::Exec), None, Some(cmd)) => Ok(Action::Exec { cmd }),
                    (Some(Type::Launch), None, Some(cmd)) => Ok(Action::Launch {
                        cmd,
                        class,
                        wait_focus: wait_focus.unwrap_or(true),
                    }),
                    (Some(Type::Chord), _, Some(_)) => {
                        Err(de::Error::unknown_field("cmd", &["type", "keys"]))
                    }
                    (Some(Type::Exec | Type::Launch), Some(_), _) => {
                        Err(de::Error::unknown_field("keys", &["type", "cmd"]))
                    }
                    (Some(Type::Chord), None, None) => Err(de::Error::missing_field("keys")),
                    (Some(Type::Exec | Type::Launch), None, None) => {
                        Err(de::Error::missing_field("cmd"))
                    }
                }
            }
        }

        deserializer.deserialize_map(ActionVisitor)
    }
}

/// An `on_fail` inside an action never deserializes; failing as the value
/// puts the diagnostic on the nested table itself.
enum NestedOnFail {}

impl<'de> Deserialize<'de> for NestedOnFail {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        de::IgnoredAny::deserialize(deserializer)?;
        Err(de::Error::custom(
            "an action has no `on_fail` of its own; `on_fail` belongs to the key, next to `action`",
        ))
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

/// Whether `id` is a valid profile id: lowercase letters, digits, `.`, `_`
/// and `-`, starting with a letter or digit, so it is also a safe file name.
#[must_use]
pub fn is_valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    let valid_first = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    valid_first
        && chars
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
}

fn profile_id<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let id = String::deserialize(deserializer)?;
    if is_valid_id(&id) {
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

/// An optional [`KeySlot`] written as a JSON number, as the trace and the
/// socket protocol carry it (absent for a Dialpad control); the
/// [`Deserialize`] impl reads the TOML table key.
pub(crate) fn optional_slot_number<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<KeySlot>, D::Error> {
    let Some(n) = Option::<u8>::deserialize(deserializer)? else {
        return Ok(None);
    };
    KeySlot::new(n).map(Some).ok_or_else(|| {
        de::Error::invalid_value(Unexpected::Unsigned(n.into()), &"a key number from 1 to 9")
    })
}

/// A [`KeySlot`] written as a JSON number, like [`optional_slot_number`].
pub(crate) fn slot_number<'de, D: Deserializer<'de>>(deserializer: D) -> Result<KeySlot, D::Error> {
    let n = u8::deserialize(deserializer)?;
    KeySlot::new(n).ok_or_else(|| {
        de::Error::invalid_value(Unexpected::Unsigned(n.into()), &"a key number from 1 to 9")
    })
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

impl Serialize for Color {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
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
    fn valid_ids_are_safe_file_names() {
        for id in ["global", "org.kde.konsole", "a-b_c", "9"] {
            assert!(is_valid_id(id), "{id}");
        }
        for id in ["", "..", "../x", "a/b", ".hidden", "Upper", "-x", "é"] {
            assert!(!is_valid_id(id), "{id}");
        }
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
