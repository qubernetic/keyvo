// The TypeScript client against the shared socket fixtures in
// tests/protocol-fixtures/socket/: every line parses and writes back to the
// same JSON, as the Rust check in crates/keyvo-core/tests/socket.rs does.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  EVENT_CLASSES,
  ProtocolError,
  parseRequest,
  parseServerMessage,
  requestLine,
  type Recipes,
  type ServerMessage,
  type Status,
} from "./protocol";

const FIXTURES = join(
  dirname(fileURLToPath(import.meta.url)),
  "../../../tests/protocol-fixtures/socket",
);

function fixture(name: string): string[] {
  return readFileSync(join(FIXTURES, name), "utf8")
    .split("\n")
    .filter((line) => line.trim() !== "");
}

function json(line: string): unknown {
  return JSON.parse(line);
}

describe("requests", () => {
  it("every request fixture round-trips", () => {
    for (const line of fixture("requests.ndjson")) {
      const request = parseRequest(line);
      expect(json(requestLine(request)), line).toEqual(json(line));
    }
  });

  it("a request line carries v and no newline", () => {
    const line = requestLine({ id: 9, cmd: "press", key: 5, dry_run: true });
    expect(line).not.toContain("\n");
    expect(json(line)).toEqual({ v: 1, id: 9, cmd: "press", key: 5, dry_run: true });
  });
});

describe("server messages", () => {
  it("every response fixture is a response and round-trips", () => {
    const codes: string[] = [];
    for (const line of fixture("responses.ndjson")) {
      const message = parseServerMessage(line);
      expect(message.kind, line).toBe("response");
      if (message.kind === "response" && !message.response.ok) {
        codes.push(message.response.error.code);
      }
      expect(json(serverLine(message)), line).toEqual(json(line));
    }
    expect(codes).toEqual([
      "reload_failed",
      "invalid_json",
      "unsupported_version",
      "invalid_request",
      "unavailable",
    ]);
  });

  it("every event fixture is an event and round-trips, every class once at least", () => {
    const classes = new Set<string>();
    for (const line of fixture("events.ndjson")) {
      const message = parseServerMessage(line);
      expect(message.kind, line).toBe("event");
      if (message.kind === "event") {
        classes.add(message.event.event);
      }
      expect(json(serverLine(message)), line).toEqual(json(line));
    }
    expect([...classes].sort()).toEqual(
      ["config", "context", "device", "key", "mode", "page", "profile", "session", "trace"],
    );
  });

  it("views carry locked, a lock is a session event, a blocked press says why", () => {
    const events = fixture("events.ndjson").map(parseServerMessage);
    const views = events.flatMap((message) =>
      message.kind === "event" && "index" in message.event ? [message.event] : [],
    );
    expect(views.map((view) => view.locked)).toEqual([false, false, false]);
    expect(events).toContainEqual({ kind: "event", event: { event: "session", locked: true } });
    expect(events).toContainEqual({
      kind: "event",
      event: expect.objectContaining({
        event: "trace",
        entry: expect.objectContaining({
          action: null,
          result: { status: "blocked", reason: "session locked" },
        }),
      }),
    });
    expect(EVENT_CLASSES).toContain("session");
  });

  it("an on_fail run is a trace entry on its branch, after the press it belongs to", () => {
    const entries = fixture("events.ndjson")
      .map(parseServerMessage)
      .flatMap((message) =>
        message.kind === "event" && message.event.event === "trace" ? [message.event.entry] : [],
      );
    const at = entries.findIndex((entry) => entry.branch === "on_fail");
    const [primary, onFail] = [entries[at - 1], entries[at]];
    expect(onFail?.action).toEqual({ type: "exec", cmd: "notify-send 'build failed'" });
    expect(primary?.branch).toBeUndefined();
    expect(primary?.result).toMatchObject({ status: "exited", code: 101 });
    expect({ ...onFail, action: null, result: null, branch: undefined }).toEqual({
      ...primary,
      action: null,
      result: null,
    });
    expect(entries.filter((entry) => entry.branch === "on_fail")).toHaveLength(1);
  });

  it("the status reply carries the configuration directory and the device geometry", () => {
    const statuses = fixture("responses.ndjson")
      .map(parseServerMessage)
      .flatMap((message) =>
        message.kind === "response" && message.response.ok && "config_dir" in message.response.result
          ? [message.response.result as Status]
          : [],
      );
    expect(statuses.map((status) => status.config_dir)).toEqual([
      "/home/u/.config/keyvo",
      "/home/u/.config/keyvo",
    ]);
    const [connected, missing] = statuses;
    expect(connected?.device.geometry?.tile).toEqual({ width: 118, height: 118 });
    expect(connected?.device.geometry?.rects).toHaveLength(connected?.device.keys ?? 0);
    expect(connected?.device.geometry?.rects[1]).toEqual({ x: 182, y: 7, width: 118, height: 118 });
    expect(missing?.device.geometry).toBeUndefined();
  });

  it("the recipes request and its answers round-trip", () => {
    const [request, ...answers] = fixture("recipes.ndjson");
    expect(parseRequest(request ?? "")).toEqual({ id: 9, cmd: "recipes" });
    const replies = answers.map((line) => {
      const message = parseServerMessage(line);
      expect(json(serverLine(message)), line).toEqual(json(line));
      return message.kind === "response" && message.response.ok
        ? (message.response.result as Recipes)
        : undefined;
    });
    const [bundled, user] = replies;
    expect(bundled?.recipes.find((recipe) => recipe.id === "spectacle")?.variants[1]).toEqual({
      id: "region",
      name: "Region",
      action: { type: "exec", cmd: "spectacle --region" },
    });
    expect(bundled?.recipes.find((recipe) => recipe.id === "calculator")?.variants[1]?.action).toEqual({
      type: "launch",
      cmd: "flatpak run org.kde.kcalc",
    });
    expect(bundled?.problems).toBeUndefined();
    expect(user?.problems).toHaveLength(1);
  });

  it("an unknown field from a later daemon is kept, not refused", () => {
    const message = parseServerMessage('{"v":1,"event":"device","connected":true,"keys":9,"later":1}');
    expect(message).toEqual({
      kind: "event",
      event: { event: "device", connected: true, keys: 9, later: 1 },
    });
  });

  it.each([
    ["not json", "invalid_json"],
    ["[1]", "invalid_json"],
    ['{"ok":true,"result":{}}', "unsupported_version"],
    ['{"v":2,"ok":true,"result":{}}', "unsupported_version"],
    ['{"v":1,"ok":true}', "invalid_message"],
    ['{"v":1,"ok":false}', "invalid_message"],
    ['{"v":1,"event":7}', "invalid_message"],
  ])("%s is refused as %s", (line, code) => {
    expect(() => parseServerMessage(line)).toThrow(ProtocolError);
    try {
      parseServerMessage(line);
    } catch (error) {
      expect((error as ProtocolError).code).toBe(code);
    }
  });
});

/** The message as the daemon would write it. */
function serverLine(message: ServerMessage): string {
  return message.kind === "event"
    ? JSON.stringify({ v: 1, ...message.event })
    : JSON.stringify({ v: 1, ...message.response });
}
