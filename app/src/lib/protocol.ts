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
  | { cmd: "notify_context"; class: string; title?: string };

/** A command with the id its response echoes. */
export type Request = Command & { id?: unknown };

export interface DeviceStatus {
  connected: boolean;
  keys?: number;
  error?: string;
}

export interface Context {
  class: string;
  title: string;
  pid?: number;
  source: "watcher" | "notify_context";
}

export interface Status {
  version: string;
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

/** One trace entry (spec §8). */
export interface TraceRecord {
  timestamp_ms: number;
  trigger: { press: PressKind; simulated: boolean };
  profile: string;
  page: number;
  key: number;
  action: Action | null;
  result: { status: string; [field: string]: unknown };
}

export interface Pressed {
  entry: TraceRecord;
}

export type Reply = Status | Subscribed | Pressed | Record<string, never>;

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

const COMMANDS = new Set(["status", "subscribe", "press", "reload", "notify_context"]);

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
