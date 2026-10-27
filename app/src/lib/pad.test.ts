// The Pad's state against the shared socket fixtures in
// tests/protocol-fixtures/socket/: the status reply, then the updates of a
// subscription, folded the way crates/keyvo-client/src/mirror.rs folds them.
import { describe, expect, it } from "vitest";

import { apply, caption, clock, fromStatus, grid, PAD_TRACE, initial, schemaError, summarize, type PadState } from "./pad";
import type { Event, Status, TraceRecord } from "./protocol";
import { events, eventsOf, nth, responses } from "./fixtures";

const STATUS = responses()[0] as Status;
const EVENTS = events();

function traces(): TraceRecord[] {
  return EVENTS.flatMap((event) => (event.event === "trace" ? [event.entry] : []));
}

describe("fromStatus", () => {
  it("takes the view, the device and its geometry, and the context", () => {
    const state = fromStatus(STATUS);
    expect(state.view).toEqual({ profile: "terminal", page: 1, mode: "auto", index: 1 });
    expect(state.device).toMatchObject({ connected: true, keys: 9 });
    expect(state.geometry?.tile).toEqual({ width: 118, height: 118 });
    expect(state.geometry?.rects).toHaveLength(9);
    expect(state.context?.class).toBe("org.kde.konsole");
    expect(state.trace).toEqual([]);
    expect(state.closed).toBeUndefined();
  });

  it("leaves the context and the geometry out before the first report", () => {
    const state = fromStatus(responses()[1] as Status);
    expect(state.context).toBeUndefined();
    expect(state.geometry).toBeUndefined();
  });
});

describe("grid", () => {
  it("lays the reported keys out in rows, top to bottom and left to right", () => {
    const geometry = fromStatus(STATUS).geometry;
    expect(geometry && grid(geometry)).toEqual([
      [1, 2, 3],
      [4, 5, 6],
      [7, 8, 9],
    ]);
  });

  it("follows the rectangles, not a fixed 3x3", () => {
    const rect = (x: number, y: number) => ({ x, y, width: 50, height: 40 });
    const geometry = {
      panel: { width: 200, height: 100 },
      tile: { width: 50, height: 40 },
      // Out of table order, and key 4 a few pixels lower than key 3.
      rects: [rect(0, 50), rect(60, 0), rect(0, 0), rect(120, 3), rect(60, 52)],
    };
    expect(grid(geometry)).toEqual([
      [3, 2, 4],
      [1, 5],
    ]);
    expect(grid({ ...geometry, rects: [] })).toEqual([]);
  });
});

describe("apply", () => {
  it("the subscribed view replaces the status view", () => {
    const state = apply(fromStatus(STATUS), {
      kind: "subscribed",
      view: { profile: "global", page: 2, mode: "manual", index: 0 },
    });
    expect(state.view).toEqual({ profile: "global", page: 2, mode: "manual", index: 0 });
  });

  it("the list owner is the view's profile from index 1 on and stays below", () => {
    let state = fromStatus(STATUS);
    expect(state.owner).toBe("terminal");
    const view = (profile: string, page: number, index: number) =>
      apply(state, { kind: "event", event: { event: "page", profile, page, mode: "auto", index } });
    state = view("global", 2, 0);
    expect(state.owner).toBe("terminal");
    state = view("vscode", 2, 2);
    expect(state.owner).toBe("vscode");
    state = apply(initial(), {
      kind: "subscribed",
      view: { profile: "global", page: 3, mode: "auto", index: -1 },
    });
    expect(state.owner).toBe("global");
  });

  it("profile, page and mode events carry the whole view", () => {
    let state = initial();
    for (const event of [...eventsOf("profile"), ...eventsOf("page"), ...eventsOf("mode")]) {
      state = apply(state, { kind: "event", event });
      expect(state.view).toStrictEqual({
        profile: event.profile,
        page: event.page,
        mode: event.mode,
        index: event.index,
      });
      expect(typeof state.view?.index).toBe("number");
    }
  });

  it("a connected device event brings its geometry, a lost one keeps the last", () => {
    const [connected, lost] = [nth(eventsOf("device"), 0), nth(eventsOf("device"), 1)];
    const state = apply(fromStatus(responses()[1] as Status), { kind: "event", event: connected });
    expect(state.geometry?.rects).toHaveLength(9);
    const unplugged = apply(state, { kind: "event", event: lost });
    expect(unplugged.device?.connected).toBe(false);
    expect(unplugged.geometry).toEqual(state.geometry);
  });

  it("remembers when the Keypad was last seen connected", () => {
    const [connected, lost] = [nth(eventsOf("device"), 0), nth(eventsOf("device"), 1)];
    expect(fromStatus(STATUS, 1000).seen).toBe(1000);
    const missing = fromStatus(responses()[1] as Status, 1000);
    expect(missing.seen).toBeUndefined();
    const plugged = apply(missing, { kind: "event", event: connected }, 2000);
    expect(plugged.seen).toBe(2000);
    // A lost Keypad keeps the time it was last seen.
    expect(apply(plugged, { kind: "event", event: lost }, 3000).seen).toBe(2000);
    // Other updates while connected do not move it.
    expect(apply(plugged, { kind: "event", event: nth(eventsOf("mode"), 0) }, 4000).seen).toBe(2000);
  });

  it("device and context events replace the last report", () => {
    let state = fromStatus(STATUS);
    for (const event of eventsOf("device")) {
      state = apply(state, { kind: "event", event });
    }
    expect(state.device).toEqual({ connected: false, error: "device disconnected" });
    for (const event of eventsOf("context")) {
      state = apply(state, { kind: "event", event });
    }
    expect(state.context).toEqual({
      class: "keyvo-app",
      title: "keyvo",
      pid: 5150,
      source: "watcher",
      held: true,
    });
    expect(state.holding?.class).toBe("org.gnome.Ptyxis");
  });

  it("a held context holds the window reported before it", () => {
    const contexts = eventsOf("context");
    const [konsole, ptyxis, app] = [nth(contexts, 0), nth(contexts, 1), nth(contexts, 2)];
    const fold = (state: PadState, event: Event) => apply(state, { kind: "event", event });
    // Nothing reported before the keyvo app: it holds no known window.
    let state = fold(initial(), app);
    expect(state.holding).toBeUndefined();
    state = fold(fold(state, konsole), app);
    expect(state.holding?.class).toBe("org.kde.konsole");
    // A second held report keeps the window held over.
    state = fold(state, app);
    expect(state.holding?.class).toBe("org.kde.konsole");
    state = fold(state, ptyxis);
    expect(state.holding).toBeUndefined();
    expect(fromStatus(STATUS).holding).toBeUndefined();
  });

  it("key events hold and release a key", () => {
    const press = nth(eventsOf("key"), 0);
    const release = nth(eventsOf("key"), 1);
    const held = apply(initial(), { kind: "event", event: press });
    expect(held.held).toEqual([5]);
    expect(apply(held, { kind: "event", event: press }).held).toEqual([5]);
    expect(apply(held, { kind: "event", event: release }).held).toEqual([]);
  });

  it("trace entries come newest first, at most PAD_TRACE", () => {
    let state = initial();
    for (let round = 0; round < PAD_TRACE; round++) {
      for (const event of eventsOf("trace")) {
        state = apply(state, { kind: "event", event });
      }
    }
    expect(state.trace).toHaveLength(PAD_TRACE);
    expect(state.trace[0]).toEqual(traces().at(-1));
    expect(state.trace[1]).toEqual(traces().at(-2));
  });

  it("a failed reload is kept until one succeeds, and every reload counts", () => {
    const good = nth(eventsOf("config"), 0);
    const bad = nth(eventsOf("config"), 1);
    const failed = apply(initial(), { kind: "event", event: bad });
    expect(failed.configError).toContain("profiles/global.toml:8:9");
    expect(failed.reloads).toBe(1);
    const fixed = apply(failed, { kind: "event", event: good });
    expect(fixed.configError).toBeUndefined();
    expect(fixed.reloads).toBe(2);
  });

  it("closed keeps the last frame and says why", () => {
    const state = apply(fromStatus(STATUS), { kind: "closed", reason: "the daemon closed the connection" });
    expect(state.closed).toBe("the daemon closed the connection");
    expect(state.view).toEqual(STATUS.view);
  });

  it("does not change the state it is given", () => {
    const state = fromStatus(STATUS);
    const copy = structuredClone(state);
    for (const event of EVENTS) {
      apply(state, { kind: "event", event });
    }
    expect(state).toEqual(copy);
  });
});

describe("caption", () => {
  it("names the held window and the profile the pad holds after a held context", () => {
    const app = nth(eventsOf("context"), 2);
    const state = apply(fromStatus(STATUS), { kind: "event", event: app });
    expect(caption(state)).toBe("keyvo-app focused, holding terminal");
  });

  it("says nothing while the focused window is not held", () => {
    expect(caption(fromStatus(STATUS))).toBeUndefined();
    expect(caption(initial())).toBeUndefined();
  });

  it("says nothing before the pad reports a view", () => {
    const app = nth(eventsOf("context"), 2);
    expect(caption(apply(initial(), { kind: "event", event: app }))).toBeUndefined();
  });
});

describe("summarize", () => {
  it("says what every fixture entry did on one line", () => {
    expect(traces().map(summarize)).toEqual([
      "Key 4, app layer: chord ctrl+p, sent",
      "Key 1, app layer: exec cargo build, exited 101 after 2318 ms",
      "Key 9, app layer: exec gh pr view --web, failed: no such file or directory: gh",
      "Key 5 long, simulated, app layer: exec cargo test, dry run, nothing executed",
      "Key 1: unbound",
      "Key 2, global layer: launch flatpak run org.kde.kcalc, started, pid 4711",
    ]);
  });

  it("says when an exec is still running", () => {
    const pending = (responses()[4] as { entry: TraceRecord }).entry;
    expect(summarize(pending)).toBe("Key 1 long, global layer: exec cargo build, running");
  });

  it("names the global layer of a key the app profile lacks", () => {
    const entry = nth(traces(), 0);
    expect(summarize({ ...entry, profile: "terminal", layer: "global" })).toBe(
      "Key 4, global layer: chord ctrl+p, sent",
    );
  });

  it("names a status it does not know", () => {
    const entry = nth(traces(), 0);
    expect(summarize({ ...entry, result: { status: "teleported" } })).toBe(
      "Key 4, app layer: chord ctrl+p, teleported",
    );
  });
});

describe("schemaError", () => {
  it("takes file, line and column from the daemon's reload error", () => {
    const failed = nth(eventsOf("config"), 1);
    expect(schemaError(failed.error ?? "")).toEqual({
      file: "profiles/global.toml",
      line: 8,
      column: 9,
      message: "invalid type: integer `7`, expected a string",
    });
  });

  it("takes a line without a column", () => {
    expect(schemaError("config.toml:3: unknown key `x`")).toEqual({
      file: "config.toml",
      line: 3,
      message: "unknown key `x`",
    });
  });

  it("has nothing for an error without a place", () => {
    expect(schemaError("cannot read the configuration directory")).toBeUndefined();
  });
});

describe("clock", () => {
  it("writes a time of day in 24 h with seconds", () => {
    expect(clock(new Date(2026, 9, 26, 15, 4, 5).getTime())).toBe("15:04:05");
    expect(clock(new Date(2026, 9, 26, 0, 0, 9).getTime())).toBe("00:00:09");
  });
});
