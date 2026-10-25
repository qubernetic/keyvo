// The TypeScript client against the shared socket fixtures in
// tests/protocol-fixtures/socket/: every line parses and writes back to the
// same JSON, as the Rust check in crates/keyvo-core/tests/socket.rs does.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  ProtocolError,
  parseRequest,
  parseServerMessage,
  requestLine,
  type ServerMessage,
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
      ["config", "context", "device", "key", "mode", "page", "profile", "trace"],
    );
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
