// Home's state against the shared socket fixtures in
// tests/protocol-fixtures/socket/: the status reply, then the updates of a
// subscription, folded the way crates/keyvo-client/src/mirror.rs folds them.
import { describe, expect, it } from "vitest";

import { apply, fromStatus, HOME_TRACE, initial, summarize } from "./home";
import type { Status, TraceRecord } from "./protocol";
import { events, eventsOf, nth, responses } from "./fixtures";

const STATUS = responses()[0] as Status;
const EVENTS = events();

function traces(): TraceRecord[] {
  return EVENTS.flatMap((event) => (event.event === "trace" ? [event.entry] : []));
}

describe("fromStatus", () => {
  it("takes the view, the device and the context", () => {
    const state = fromStatus(STATUS);
    expect(state.view).toEqual({ profile: "terminal", page: 1, mode: "auto" });
    expect(state.device).toEqual({ connected: true, keys: 9 });
    expect(state.context?.class).toBe("org.kde.konsole");
    expect(state.trace).toEqual([]);
    expect(state.closed).toBeUndefined();
  });

  it("leaves the context out before the first report", () => {
    expect(fromStatus(responses()[1] as Status).context).toBeUndefined();
  });
});

describe("apply", () => {
  it("the subscribed view replaces the status view", () => {
    const state = apply(fromStatus(STATUS), {
      kind: "subscribed",
      view: { profile: "global", page: 2, mode: "manual" },
    });
    expect(state.view).toEqual({ profile: "global", page: 2, mode: "manual" });
  });

  it("profile, page and mode events carry the whole view", () => {
    let state = initial();
    for (const event of [...eventsOf("profile"), ...eventsOf("page"), ...eventsOf("mode")]) {
      state = apply(state, { kind: "event", event });
      expect(state.view).toEqual({ profile: event.profile, page: event.page, mode: event.mode });
    }
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
    expect(state.context).toEqual({ class: "org.gnome.Ptyxis", title: "vim", source: "notify_context" });
  });

  it("key events hold and release a key", () => {
    const press = nth(eventsOf("key"), 0);
    const release = nth(eventsOf("key"), 1);
    const held = apply(initial(), { kind: "event", event: press });
    expect(held.held).toEqual([5]);
    expect(apply(held, { kind: "event", event: press }).held).toEqual([5]);
    expect(apply(held, { kind: "event", event: release }).held).toEqual([]);
  });

  it("trace entries come newest first, at most HOME_TRACE", () => {
    let state = initial();
    for (let round = 0; round < HOME_TRACE; round++) {
      for (const event of eventsOf("trace")) {
        state = apply(state, { kind: "event", event });
      }
    }
    expect(state.trace).toHaveLength(HOME_TRACE);
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

describe("summarize", () => {
  it("says what every fixture entry did on one line", () => {
    expect(traces().map(summarize)).toEqual([
      "Key 4: chord ctrl+p, sent",
      "Key 1: exec cargo build, exited 101 after 2318 ms",
      "Key 9: exec gh pr view --web, failed: no such file or directory: gh",
      "Key 5 long, simulated: exec cargo test, dry run, nothing executed",
      "Key 1: unbound",
    ]);
  });

  it("says when an exec is still running", () => {
    const pending = (responses()[4] as { entry: TraceRecord }).entry;
    expect(summarize(pending)).toBe("Key 1 long: exec cargo build, running");
  });

  it("names a status it does not know", () => {
    const entry = nth(traces(), 0);
    expect(summarize({ ...entry, result: { status: "teleported" } })).toBe(
      "Key 4: chord ctrl+p, teleported",
    );
  });
});
