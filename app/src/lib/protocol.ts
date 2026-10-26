// The socket protocol, version 1 (docs/socket-api.md), on the TypeScript
// side: the same messages as keyvo_core::socket, checked against the shared
// fixtures in tests/protocol-fixtures/socket/. Responses and events keep
// fields this client does not know, so a later v1 daemon may add some;
// requests are written with known fields only, as the daemon rejects others.

/** The protocol version this client speaks. */
export const VERSION = 1;

export type EventClass =
  | "key"
  | "profile"
  | "page"
  | "mode"
  | "device"
  | "context"
  | "trace"
  | "config";

export const EVENT_CLASSES: readonly EventClass[] = [
  "key",
  "profile",
  "page",
  "mode",
  "device",
  "context",
  "trace",
  "config",
];

export type Mode = "auto" | "manual";
export type PressKind = "short" | "long";

/** What the pad shows. */
export interface View {
  profile: string;
  /** Counted from 1. */
  page: number;
  mode: Mode;
}

export type Command =
  | { cmd: "status" }
  | { cmd: "subscribe"; events?: EventClass[]; replay?: number }
  | { cmd: "press"; key: number; kind?: PressKind; dry_run?: boolean }
  | { cmd: "reload" }
  | { cmd: "notify_context"; class: string; title?: string }
  | { cmd: "recipes" };

/** A command with the id its response echoes. */
export type Request = Command & { id?: unknown };

/** Width and height in pixels. */
export interface Size {
  width: number;
  height: number;
}

/** One key's rectangle on the panel. */
export interface Rect extends Size {
  x: number;
  y: number;
}

/** The Keypad's display as the device publishes it (spec §5.4). */
export interface Geometry {
  panel: Size;
  /** The size the keys are rendered at. */
  tile: Size;
  /** Key 1 first. */
  rects: Rect[];
}

export interface DeviceStatus {
  connected: boolean;
  keys?: number;
  /** Present while a Keypad is connected. */
  geometry?: Geometry;
  error?: string;
}

export interface Context {
  class: string;
  title: string;
  pid?: number;
  source: "watcher" | "notify_context";
  /** The class is in `[focus].hold_classes`: the context and the view stay
   * those of the window before. Absent when false. */
  held?: boolean;
}

export interface Status {
  version: string;
  /** The configuration directory the daemon loads; the app edits this tree. */
  config_dir: string;
  device: DeviceStatus;
  view: View;
  context?: Context;
  /** Entries in the daemon's trace ring buffer. */
  trace: number;
}

export interface Subscribed {
  events: EventClass[];
  view: View;
}

export type Action = { type: "chord"; keys: string } | { type: "exec"; cmd: string };

/** The layer of the stack that supplied a key (spec §6.1). */
export type Layer = "global" | "app";

/** One trace entry (spec §8). */
export interface TraceRecord {
  timestamp_ms: number;
  trigger: { press: PressKind; simulated: boolean };
  profile: string;
  page: number;
  key: number;
  /** Absent when no layer defines the key. */
  layer?: Layer;
  action: Action | null;
  result: { status: string; [field: string]: unknown };
}

export interface Pressed {
  entry: TraceRecord;
}

/** A catalogue group (spec §7.5). */
export type RecipeGroup = "general" | "system" | "apps";

/** One "What it does" of a recipe pack. */
export interface RecipeVariant {
  /** Unique in the pack. */
  id: string;
  name: string;
  action: Action;
}

/** A recipe pack of the actions catalogue (socket-api.md, `recipes`). */
export interface Recipe {
  /** The pack's file name without `.toml`. */
  id: string;
  source: "bundled" | "user";
  group: RecipeGroup;
  /** The entry's name in the catalogue. */
  name: string;
  /** The icon written into a key, `lucide:<name>`. */
  icon: string;
  /** The label written into a key. */
  label: string;
  /** At least one; the first is the default. */
  variants: RecipeVariant[];
}

/** The `recipes` reply. */
export interface Recipes {
  /** By group, then name, then id. */
  recipes: Recipe[];
  /** One `file:line:column: message` per user pack left out; absent when
   * every pack loaded. */
  problems?: string[];
}

export type Reply = Status | Subscribed | Pressed | Recipes | Record<string, never>;

export interface ErrorBody {
  code: string;
  message: string;
}

export type Response =
  | { id?: unknown; ok: true; result: Reply }
  | { id?: unknown; ok: false; error: ErrorBody };

export type Event =
  | { event: "key"; key: number; edge: "press" | "release" }
  | ({ event: "profile" | "page" | "mode" } & View)
  | ({ event: "device" } & DeviceStatus)
  | ({ event: "context" } & Context)
  | { event: "trace"; entry: TraceRecord }
  | { event: "config"; reloaded: boolean; error?: string };

/** A line from the daemon. */
export type ServerMessage =
  | { kind: "response"; response: Response }
  | { kind: "event"; event: Event };

/** Why a line is not a v1 message. */
export type ProtocolErrorCode = "invalid_json" | "unsupported_version" | "invalid_message";

export class ProtocolError extends Error {
  constructor(
    readonly code: ProtocolErrorCode,
    message: string,
  ) {
    super(message);
    this.name = "ProtocolError";
  }
}

type Fields = Record<string, unknown>;

function isObject(value: unknown): value is Fields {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** The fields of `line` without `v`, its version checked. */
function envelope(line: string): Fields {
  let value: unknown;
  try {
    value = JSON.parse(line);
  } catch (error) {
    throw new ProtocolError("invalid_json", String(error));
  }
  if (!isObject(value)) {
    throw new ProtocolError("invalid_json", "a message is one JSON object");
  }
  const { v, ...fields } = value;
  if (v !== VERSION) {
    throw new ProtocolError(
      "unsupported_version",
      v === undefined
        ? `a message needs "v": ${VERSION}`
        : `protocol version ${String(v)} is not supported; this client speaks ${VERSION}`,
    );
  }
  return fields;
}

const COMMANDS = new Set(["status", "subscribe", "press", "reload", "notify_context", "recipes"]);

/** Parses one request line, as the daemon reads it. */
export function parseRequest(line: string): Request {
  const fields = envelope(line);
  if (typeof fields.cmd !== "string" || !COMMANDS.has(fields.cmd)) {
    throw new ProtocolError("invalid_message", `unknown command ${JSON.stringify(fields.cmd)}`);
  }
  return fields as Request;
}

/** The request as one line, without the newline. */
export function requestLine(request: Request): string {
  return JSON.stringify({ v: VERSION, ...request });
}

/** Parses one line from the daemon: a response or an event. */
export function parseServerMessage(line: string): ServerMessage {
  const fields = envelope(line);
  if ("event" in fields) {
    if (typeof fields.event !== "string" || !(EVENT_CLASSES as string[]).includes(fields.event)) {
      throw new ProtocolError("invalid_message", `unknown event ${JSON.stringify(fields.event)}`);
    }
    return { kind: "event", event: fields as Event };
  }
  if (fields.ok === true && isObject(fields.result) && !("error" in fields)) {
    return { kind: "response", response: fields as Response };
  }
  if (
    fields.ok === false &&
    isObject(fields.error) &&
    typeof fields.error.code === "string" &&
    !("result" in fields)
  ) {
    return { kind: "response", response: fields as Response };
  }
  throw new ProtocolError("invalid_message", "a response has `result` when ok, else `error`");
}
