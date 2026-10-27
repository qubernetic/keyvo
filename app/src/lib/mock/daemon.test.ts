// The mock daemon: its replies are lines of the socket protocol
// (parseServerMessage reads each), its tiles and presses come from the
// contrib profiles over the global layer, and its subscription moves: a
// fixture trace entry every two seconds, a focus change every ten.
import { readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { Update } from "../daemon";
import { parseServerMessage, VERSION, type Event, type Reply } from "../protocol";
import { fixtureTrace, SNAPSHOTS, contribProfiles, fixtureStatus } from "./data";
import { FOCUS_MS, mockDaemon, TRACE_MS } from "./daemon";
import { mockProfiles } from "./profiles";
import { Store } from "./store";

/** `result` as the daemon's response line, read back by the protocol module. */
function asResponse(result: Reply): Reply {
  const message = parseServerMessage(JSON.stringify({ v: VERSION, id: 1, ok: true, result }));
  if (message.kind !== "response" || !message.response.ok) throw new Error("not a reply");
  return message.response.result;
}

function asEvent(event: Event): Event {
  const message = parseServerMessage(JSON.stringify({ v: VERSION, ...event }));
  if (message.kind !== "event") throw new Error("not an event");
  return message.event;
}

const TILE = { width: 118, height: 118 };

const RECIPES = join(dirname(fileURLToPath(import.meta.url)), "../../../../contrib/recipes");

describe("the mock daemon", () => {
  beforeEach(() => {
    vi.useFakeTimers({ now: 1_800_000_000_000 });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("reports the fixture's connected Keypad, no match focused, the global profile", async () => {
    const status = await mockDaemon(new Store(contribProfiles())).status();
    const fixture = fixtureStatus();
    expect(asResponse(status)).toEqual(status);
    expect(status.device).toEqual(fixture.device);
    expect(status.device.connected).toBe(true);
    expect(status.device.geometry?.rects).toHaveLength(9);
    expect(status.config_dir).toBe(fixture.config_dir);
    expect(status.view).toEqual({ profile: "global", page: 1, mode: "auto", index: 1 });
    expect(status.context?.class).toBe("org.kde.dolphin");
  });

  it("rejects status when told the daemon is unreachable", async () => {
    const daemon = mockDaemon(new Store(contribProfiles()), { unreachable: true });
    await expect(daemon.status()).rejects.toMatch(/^cannot reach the keyvo daemon at /);
  });

  it("lists the recipe packs of contrib/recipes/ and the fixture's user pack", async () => {
    const reply = await mockDaemon(new Store(contribProfiles())).recipes();
    expect(asResponse(reply)).toEqual(reply);
    const bundled = reply.recipes.filter((recipe) => recipe.source === "bundled");
    const files = readdirSync(RECIPES)
      .filter((file) => file.endsWith(".toml"))
      .map((file) => file.slice(0, -".toml".length));
    expect(bundled.map((recipe) => recipe.id).sort()).toEqual(files.sort());
    expect(reply.recipes.filter((recipe) => recipe.source === "user").map((recipe) => recipe.id)).toEqual([
      "screenshot",
    ]);
    expect(reply.problems).toBeUndefined();
  });

  it("outlines a profile from its file", async () => {
    const daemon = mockDaemon(new Store(contribProfiles()));
    expect(await daemon.outline("global")).toEqual({ pages: 3, accent: "#4e8bd6" });
    await expect(daemon.outline("nope")).rejects.toBe("no profile `nope` in the configuration directory");
  });

  it("follows an edit of the profile header", async () => {
    const store = new Store(contribProfiles());
    await mockProfiles(store).editProfile("global", { name: "Global", accent: "#ff0000" });
    expect(await mockDaemon(store).outline("global")).toEqual({ pages: 3, accent: "#ff0000" });
  });

  it("tiles an app page over the global context page", async () => {
    const tiles = await mockDaemon(new Store(contribProfiles())).tiles("konsole", 1, 9, TILE);
    expect(tiles).toHaveLength(9);
    expect(tiles[0]).toEqual({
      image: SNAPSHOTS.iconLabel,
      label: "New tab",
      action: { type: "chord", keys: "ctrl+shift+t" },
      layer: "app",
    });
    // Konsole has no key 3: the global profile's context page supplies it.
    expect(tiles[2]).toEqual({
      image: SNAPSHOTS.iconLabel,
      label: "Type",
      action: { type: "chord", keys: "shift+k, e, y" },
      layer: "global",
    });
    // No layer defines key 7.
    expect(tiles[6]).toEqual({ image: SNAPSHOTS.background });
  });

  it("picks the snapshot by what the key shows", async () => {
    const store = new Store(contribProfiles());
    const daemon = mockDaemon(store);
    const third = await daemon.tiles("global", 3, 9, TILE);
    expect(third[0]).toEqual({ image: SNAPSHOTS.label, label: "Third", layer: "global" });
    expect(third[1]).toEqual({ image: SNAPSHOTS.background });
    const page = store.page("global", 3);
    page.keys.set(2, { icon: "lucide:x" });
    expect((await daemon.tiles("global", 3, 9, TILE))[1]).toEqual({
      image: SNAPSHOTS.icon,
      layer: "global",
    });
  });

  it("rejects tiles of an unknown profile or page", async () => {
    const daemon = mockDaemon(new Store(contribProfiles()));
    await expect(daemon.tiles("nope", 1, 9, TILE)).rejects.toBe(
      "no profile `nope` in the configuration directory",
    );
    await expect(daemon.tiles("konsole", 2, 9, TILE)).rejects.toBe("profile `konsole` has no page 2");
  });

  it("answers a press with a dry-run trace record of the key's action", async () => {
    const daemon = mockDaemon(new Store(contribProfiles()));
    const entry = await daemon.press(5, "long");
    expect(asResponse({ entry })).toEqual({ entry });
    expect(entry).toEqual({
      timestamp_ms: 1_800_000_000_000,
      trigger: { press: "long", simulated: true },
      profile: "global",
      page: 1,
      key: 5,
      layer: "global",
      action: { type: "exec", cmd: "true" },
      result: { status: "dry_run" },
    });
    expect(await daemon.press(9, "short")).toMatchObject({
      key: 9,
      action: null,
      result: { status: "unbound" },
    });
    expect((await daemon.press(9, "short")).layer).toBeUndefined();
  });

  it("replays the fixture trace every two seconds and cycles the focus every ten", async () => {
    const daemon = mockDaemon(new Store(contribProfiles()));
    const updates: Update[] = [];
    const stop = await daemon.subscribe(20, (update) => updates.push(update));
    expect(updates).toEqual([
      { kind: "subscribed", view: { profile: "global", page: 1, mode: "auto", index: 1 } },
    ]);

    vi.advanceTimersByTime(TRACE_MS);
    const trace = fixtureTrace();
    expect(updates[1]).toEqual({
      kind: "event",
      event: { event: "trace", entry: { ...trace[0], timestamp_ms: 1_800_000_000_000 + TRACE_MS } },
    });

    vi.advanceTimersByTime(FOCUS_MS - TRACE_MS);
    const views = () =>
      updates.flatMap((update) =>
        update.kind === "event" && update.event.event === "profile" ? [update.event.profile] : [],
      );
    const contexts = () =>
      updates.flatMap((update) =>
        update.kind === "event" && update.event.event === "context" ? [update.event.class] : [],
      );
    expect(views()).toEqual(["konsole"]);
    expect(contexts()).toEqual(["org.kde.konsole"]);
    expect((await daemon.status()).view).toEqual({ profile: "konsole", page: 1, mode: "auto", index: 1 });
    expect((await daemon.press(1, "short")).profile).toBe("konsole");

    vi.advanceTimersByTime(3 * FOCUS_MS);
    expect(views()).toEqual(["konsole", "vscode", "global"]);
    expect(contexts()).toEqual(["org.kde.konsole", "keyvo-app", "code", "org.kde.dolphin"]);

    const traces = updates.filter(
      (update) => update.kind === "event" && update.event.event === "trace",
    );
    expect(traces).toHaveLength((4 * FOCUS_MS) / TRACE_MS);
    for (const update of updates) {
      if (update.kind === "event") expect(asEvent(update.event)).toEqual(update.event);
    }

    stop();
    const seen = updates.length;
    vi.advanceTimersByTime(3 * FOCUS_MS);
    expect(updates).toHaveLength(seen);
  });

  it("holds the context while the keyvo app has focus, as [focus].hold_classes does", async () => {
    const daemon = mockDaemon(new Store(contribProfiles()));
    const updates: Update[] = [];
    const stop = await daemon.subscribe(0, (update) => updates.push(update));
    vi.advanceTimersByTime(FOCUS_MS);
    const before = updates.length;
    vi.advanceTimersByTime(FOCUS_MS);
    const step = updates
      .slice(before)
      .filter((update) => update.kind === "event" && update.event.event !== "trace");
    // One held context event and no profile event: the view stays Konsole's.
    expect(step).toEqual([
      {
      kind: "event",
        event: { event: "context", class: "keyvo-app", title: "keyvo", pid: 4545, source: "watcher", held: true },
      },
    ]);
    const status = await daemon.status();
    expect(status.view).toEqual({ profile: "konsole", page: 1, mode: "auto", index: 1 });
    expect(status.context?.held).toBe(true);
    expect((await daemon.press(1, "short")).profile).toBe("konsole");
    stop();
  });

  it("replays the newest trace entries to a new subscriber", async () => {
    const daemon = mockDaemon(new Store(contribProfiles()));
    const stop = await daemon.subscribe(0, () => undefined);
    vi.advanceTimersByTime(3 * TRACE_MS);
    stop();
    const updates: Update[] = [];
    (await daemon.subscribe(2, (update) => updates.push(update)))();
    expect(updates.map((update) => update.kind)).toEqual(["subscribed", "event", "event"]);
    expect((await daemon.status()).trace).toBe(3);
  });
});
