//! The socket protocol, version 1 (spec §8, `docs/socket-api.md`):
//! newline-delimited JSON on `$XDG_RUNTIME_DIR/keyvo/keyvo.sock`, every
//! message tagged `"v": 1`. A client sends [`Request`]s; the daemon
//! answers each with one [`Response`] carrying the request's `id`, and
//! streams [`Event`]s to a client that subscribed.
//!
//! Pre-alpha subset: `status`, `subscribe`, `press` (with `dry_run`),
//! `reload` and `notify_context`. A request with an unknown command or
//! field is rejected, so a typo never turns a dry run into a real press.
//! The fixtures in `tests/protocol-fixtures/socket/` are shared with the
//! TypeScript client.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::press::PressKind;
use crate::profile::KeySlot;
use crate::render::{Panel, TileSize};
use crate::state::View;
use crate::trace::TraceRecord;

/// The protocol version this build speaks.
pub const VERSION: u64 = 1;

/// The socket's file name in the runtime directory.
pub const SOCKET_FILE: &str = "keyvo.sock";

/// One request line from a client.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// Any JSON value; the response echoes it.
    pub id: Option<Value>,
    /// What the client asks for.
    pub command: Command,
}

/// A command and its arguments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    /// The device, the view, the focused window and the trace size.
    Status {},
    /// Streams the events of `events` on this connection from now on,
    /// after the last `replay` trace entries of the ring buffer.
    Subscribe {
        /// The event classes; all of them when left out.
        #[serde(default = "all_classes")]
        events: Vec<EventClass>,
        /// Trace entries from the ring buffer sent first, oldest first;
        /// only with the `trace` class.
        #[serde(default)]
        replay: usize,
    },
    /// A press of `key` on the active page, as from the device.
    Press {
        /// The key, 1 to 9.
        #[serde(deserialize_with = "crate::profile::slot_number")]
        key: KeySlot,
        /// Short (the default) or long.
        #[serde(default)]
        kind: PressKind,
        /// Resolve and trace, execute nothing: the simulator's path.
        #[serde(default)]
        dry_run: bool,
    },
    /// Reloads the configuration directory now.
    Reload {},
    /// An external focus source asserts the focused window.
    NotifyContext {
        /// The window class, reverse-DNS or short.
        class: String,
        /// The window title.
        #[serde(default)]
        title: String,
    },
}

fn all_classes() -> Vec<EventClass> {
    EventClass::ALL.to_vec()
}

/// Why a line is not a request, or why the daemon could not carry it out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The line is not a JSON object.
    InvalidJson,
    /// `v` is missing or not [`VERSION`].
    UnsupportedVersion,
    /// An unknown command, an unknown field or a bad value.
    InvalidRequest,
    /// `reload`: the configuration directory does not load; the last good
    /// state stays.
    ReloadFailed,
    /// The daemon is shutting down, or serves its connection cap.
    Unavailable,
}

/// A line that is not a valid message.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{message}")]
pub struct ProtocolError {
    /// The `id` of the rejected request, when it had one.
    pub id: Option<Value>,
    /// What kind of problem.
    pub code: ErrorCode,
    /// What exactly.
    pub message: String,
}

impl ProtocolError {
    fn new(id: Option<Value>, code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            id,
            code,
            message: message.into(),
        }
    }
}

/// The object of `line`, its `v` checked; `id` is only for the error.
fn envelope(line: &str) -> Result<Map<String, Value>, ProtocolError> {
    let mut map = match serde_json::from_str::<Value>(line) {
        Ok(Value::Object(map)) => map,
        Ok(_) => {
            return Err(ProtocolError::new(
                None,
                ErrorCode::InvalidJson,
                "a message is one JSON object",
            ));
        }
        Err(error) => {
            return Err(ProtocolError::new(
                None,
                ErrorCode::InvalidJson,
                error.to_string(),
            ));
        }
    };
    let id = map.get("id").cloned();
    match map.remove("v") {
        Some(v) if v == VERSION => Ok(map),
        Some(v) => Err(ProtocolError::new(
            id,
            ErrorCode::UnsupportedVersion,
            format!("protocol version {v} is not supported; this daemon speaks {VERSION}"),
        )),
        None => Err(ProtocolError::new(
            id,
            ErrorCode::UnsupportedVersion,
            format!("a message needs \"v\": {VERSION}"),
        )),
    }
}

/// `body` as one line with `v` (and `id`) in front.
fn line(id: Option<&Value>, body: Value) -> String {
    let mut map = Map::new();
    map.insert("v".into(), VERSION.into());
    if let Some(id) = id {
        map.insert("id".into(), id.clone());
    }
    if let Value::Object(fields) = body {
        map.extend(fields);
    }
    Value::Object(map).to_string()
}

impl Request {
    /// A request without an id.
    #[must_use]
    pub const fn new(command: Command) -> Self {
        Self { id: None, command }
    }

    /// Parses one request line.
    ///
    /// # Errors
    ///
    /// A [`ProtocolError`] with the request's `id` when it had one.
    pub fn parse(line: &str) -> Result<Self, ProtocolError> {
        let mut map = envelope(line)?;
        let id = map.remove("id");
        let command = Command::deserialize(Value::Object(map)).map_err(|error| {
            ProtocolError::new(id.clone(), ErrorCode::InvalidRequest, error.to_string())
        })?;
        Ok(Self { id, command })
    }

    /// The request as one line, without the newline.
    #[must_use]
    pub fn to_line(&self) -> String {
        line(
            self.id.as_ref(),
            serde_json::to_value(&self.command).unwrap_or_default(),
        )
    }
}

/// The answer to one request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(into = "WireResponse", try_from = "WireResponse")]
pub struct Response {
    /// The request's `id`.
    pub id: Option<Value>,
    /// The result, or why there is none.
    pub result: Result<Reply, ErrorBody>,
}

/// The result of a command that succeeded.
///
/// Unknown fields are ignored, so a v1 client reads the replies of a later
/// daemon. The variants differ in their required fields and are tried in
/// order; `Done`, which takes any object, comes last.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Reply {
    /// `status`.
    Status(Status),
    /// `subscribe`.
    Subscribed(Subscribed),
    /// `press`.
    Pressed(Pressed),
    /// `reload` and `notify_context`: an empty object.
    Done {},
}

/// The error of a command that failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    /// What kind of problem.
    pub code: ErrorCode,
    /// What exactly, for a human.
    pub message: String,
}

#[derive(Serialize, Deserialize)]
struct WireResponse {
    v: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    result: Option<Reply>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<ErrorBody>,
}

impl From<Response> for WireResponse {
    fn from(response: Response) -> Self {
        let (ok, result, error) = match response.result {
            Ok(reply) => (true, Some(reply), None),
            Err(error) => (false, None, Some(error)),
        };
        Self {
            v: VERSION,
            id: response.id,
            ok,
            result,
            error,
        }
    }
}

impl TryFrom<WireResponse> for Response {
    type Error = String;

    fn try_from(wire: WireResponse) -> Result<Self, String> {
        if wire.v != VERSION {
            return Err(format!("protocol version {} is not supported", wire.v));
        }
        let result = match (wire.ok, wire.result, wire.error) {
            (true, Some(reply), None) => Ok(reply),
            (false, None, Some(error)) => Err(error),
            _ => return Err("a response has `result` when ok, else `error`".into()),
        };
        Ok(Self {
            id: wire.id,
            result,
        })
    }
}

impl Response {
    /// The successful answer to the request `id`.
    #[must_use]
    pub const fn ok(id: Option<Value>, reply: Reply) -> Self {
        Self {
            id,
            result: Ok(reply),
        }
    }

    /// The failed answer to the request `id`.
    #[must_use]
    pub fn error(id: Option<Value>, code: ErrorCode, message: &str) -> Self {
        Self {
            id,
            result: Err(ErrorBody {
                code,
                message: message.to_owned(),
            }),
        }
    }

    /// The response as one line, without the newline.
    #[must_use]
    pub fn to_line(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// The `status` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    /// The daemon's version.
    pub version: String,
    /// The configuration directory the daemon loads (`--config-dir` or
    /// the default, spec §7.1); a client edits this tree.
    pub config_dir: String,
    /// The Keypad.
    pub device: DeviceStatus,
    /// What the pad shows.
    pub view: View,
    /// The focused window as last reported, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<Context>,
    /// The entries in the trace ring buffer.
    pub trace: usize,
}

/// The `subscribe` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subscribed {
    /// The event classes that will stream.
    pub events: Vec<EventClass>,
    /// What the pad shows now; `profile`, `page` and `mode` events change
    /// it from here.
    pub view: View,
}

/// The `press` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pressed {
    /// The press's trace entry; an `exec` is still `pending`, its result
    /// follows as a `trace` event.
    pub entry: TraceRecord,
}

/// The Keypad's connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceStatus {
    /// A Keypad is claimed.
    pub connected: bool,
    /// Its key count, when connected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<u8>,
    /// Its display as it publishes it, when connected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<Geometry>,
    /// Why there is none, when the daemon knows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The Keypad's display as the device publishes it (spec §5.4): what a
/// client needs to lay out and render a mirror of the keys, so no client
/// hard-codes the key count or the tile size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Geometry {
    /// The panel size.
    pub panel: TileSize,
    /// The size the keys are rendered at: key 1's rectangle (every key of
    /// the Keypad has the same), 0x0 without keys.
    pub tile: TileSize,
    /// The key rectangles on the panel, key 1 first.
    pub rects: Vec<Rect>,
}

impl Geometry {
    /// The key count, `None` past 255 keys.
    #[must_use]
    pub fn key_count(&self) -> Option<u8> {
        u8::try_from(self.rects.len()).ok()
    }

    /// The size the keys are rendered at.
    #[must_use]
    pub const fn tile_size(&self) -> TileSize {
        self.tile
    }
}

impl From<&Panel> for Geometry {
    fn from(panel: &Panel) -> Self {
        Self {
            panel: panel.size,
            tile: panel.keys.first().map_or(
                TileSize {
                    width: 0,
                    height: 0,
                },
                |key| key.size,
            ),
            rects: panel
                .keys
                .iter()
                .map(|key| Rect {
                    x: key.x,
                    y: key.y,
                    width: key.size.width,
                    height: key.size.height,
                })
                .collect(),
        }
    }
}

/// One key's rectangle on the panel, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    /// Left edge.
    pub x: u16,
    /// Top edge.
    pub y: u16,
    /// Width.
    pub width: u16,
    /// Height.
    pub height: u16,
}

/// A focus report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Context {
    /// The window class.
    pub class: String,
    /// The window title.
    pub title: String,
    /// The window's process, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// Who reported it.
    pub source: ContextSource,
    /// The class is in `[focus].hold_classes`: the context and the view
    /// stay those of the window before. Absent when `false`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub held: bool,
}

/// Where a focus report came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextSource {
    /// The daemon's window watcher (`KWin`).
    Watcher,
    /// A client's `notify_context`.
    NotifyContext,
}

/// The result of a configuration reload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigStatus {
    /// The new configuration applies.
    pub reloaded: bool,
    /// Why it did not load; the last good state stays.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A key going down or up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Edge {
    /// Pressed.
    Press,
    /// Released.
    Release,
}

/// The classes a client subscribes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventClass {
    /// Key presses and releases on the device.
    Key,
    /// Another profile shows.
    Profile,
    /// Another page shows.
    Page,
    /// The mode changed.
    Mode,
    /// The Keypad connected or disconnected.
    Device,
    /// A focus report.
    Context,
    /// A trace entry.
    Trace,
    /// A configuration reload.
    Config,
}

impl EventClass {
    /// Every class, in protocol order.
    pub const ALL: [Self; 8] = [
        Self::Key,
        Self::Profile,
        Self::Page,
        Self::Mode,
        Self::Device,
        Self::Context,
        Self::Trace,
        Self::Config,
    ];
}

/// One event line to a subscriber.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// A key on the device went down or up.
    Key {
        /// The key, 1 to 9.
        #[serde(deserialize_with = "crate::profile::slot_number")]
        key: KeySlot,
        /// Which way.
        edge: Edge,
    },
    /// Another profile shows; the whole view.
    Profile(View),
    /// Another page of the profile shows; the whole view.
    Page(View),
    /// The mode changed; the whole view.
    Mode(View),
    /// The Keypad connected or disconnected.
    Device(DeviceStatus),
    /// A focus source reported a window.
    Context(Context),
    /// A press was resolved or its action ended.
    Trace {
        /// The entry.
        entry: TraceRecord,
    },
    /// The configuration was reloaded, or did not load.
    Config(ConfigStatus),
}

impl Event {
    /// The class a subscriber chooses this event by.
    #[must_use]
    pub const fn class(&self) -> EventClass {
        match self {
            Self::Key { .. } => EventClass::Key,
            Self::Profile(_) => EventClass::Profile,
            Self::Page(_) => EventClass::Page,
            Self::Mode(_) => EventClass::Mode,
            Self::Device(_) => EventClass::Device,
            Self::Context(_) => EventClass::Context,
            Self::Trace { .. } => EventClass::Trace,
            Self::Config(_) => EventClass::Config,
        }
    }

    /// The events of a change from `before` to `after`: one per changed
    /// class, each with the whole new view.
    #[must_use]
    pub fn switched(before: &View, after: &View) -> Vec<Self> {
        let mut events = Vec::new();
        if before.profile != after.profile {
            events.push(Self::Profile(after.clone()));
        }
        if before.page != after.page {
            events.push(Self::Page(after.clone()));
        }
        if before.mode != after.mode {
            events.push(Self::Mode(after.clone()));
        }
        events
    }

    /// The event as one line, without the newline.
    #[must_use]
    pub fn to_line(&self) -> String {
        line(None, serde_json::to_value(self).unwrap_or_default())
    }
}

/// A line from the daemon: an answer or an event.
#[derive(Debug, Clone, PartialEq)]
pub enum ServerMessage {
    /// The answer to a request.
    Response(Response),
    /// An event of a subscription.
    Event(Event),
}

impl ServerMessage {
    /// Parses one line from the daemon.
    ///
    /// # Errors
    ///
    /// A [`ProtocolError`] when the line is not a v1 response or event.
    pub fn parse(text: &str) -> Result<Self, ProtocolError> {
        let map = envelope(text)?;
        let invalid = |error: serde_json::Error| {
            ProtocolError::new(None, ErrorCode::InvalidRequest, error.to_string())
        };
        if map.contains_key("event") {
            Event::deserialize(Value::Object(map))
                .map(Self::Event)
                .map_err(invalid)
        } else {
            let mut map = map;
            map.insert("v".into(), VERSION.into());
            Response::deserialize(Value::Object(map))
                .map(Self::Response)
                .map_err(invalid)
        }
    }
}
