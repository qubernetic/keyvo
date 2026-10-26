// The Pad screen (pad screen brief §5.2, §6) against a fake daemon fed with
// the shared socket fixtures (tests/protocol-fixtures/socket/): the
// drawing with the pad's tiles, the pin a selected key sets and the
// "pad moved" line, the page tabs of the pinned profile, the held caption,
// the dry-run presses, the notices and the event list. The key editor in
// the inspector's place against a fake editor; KeyEditor.test.ts tests the
// editor itself, Mirror.test.ts and PageTabs.test.ts their components.
import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { tick } from "svelte";
import { describe, expect, it, onTestFinished, vi } from "vitest";

import Pad from "./Pad.svelte";
import type { Daemon, Tile, Update } from "./lib/daemon";
import type { Editor, KeyFields, Target } from "./lib/editor";
import { eventsOf, nth, responses } from "./lib/fixtures";
import { t } from "./lib/i18n";
import { PAD_TRACE } from "./lib/pad";
import type { Pinned } from "./lib/pin";
import type { ProfileSummary } from "./lib/profiles";
import type { PressKind, Size, Status, TraceRecord } from "./lib/protocol";

const STATUS = responses()[0] as Status;
const TILE = { width: 118, height: 118 };
const DRY_RUN = (responses()[3] as { entry: TraceRecord }).entry;
const PROFILE = "/home/u/.config/keyvo/profiles/terminal.toml";

function summary(id: string, pages: (string | null)[], accent?: string): ProfileSummary {
  return {
    id,
    name: id,
    ...(accent === undefined ? {} : { accent }),
    priority: 0,
    match: { class: [] },
    pages,
    path: `/home/u/.config/keyvo/profiles/${id}.toml`,
    file: true,
  };
}

const PROFILES = [
  summary("global", ["context", "two", "three"]),
  summary("terminal", ["context", "git", null], "#2ecc71"),
  summary("empty", []),
];

/** Key 4 is the profile's own, key 3 a global key under it, key 5 has no
 * layer (as the tiles of a GUI before the layer was reported). */
function tiles(profile: string, page: number, keys = 9): Tile[] {
  return Array.from({ length: keys }, (_, index) => ({
    image: `data:image/png;base64,${profile}-${page}-${index + 1}`,
    ...(index === 4 ? { label: "Test", action: { type: "exec" as const, cmd: "cargo test" } } : {}),
    ...(index === 3
      ? {
          label: "Palette",
          action: { type: "chord" as const, keys: "ctrl+p" },
          layer: profile === "global" ? ("global" as const) : ("app" as const),
        }
      : {}),
    ...(index === 2
      ? { label: "Type", action: { type: "chord" as const, keys: "shift+k" }, layer: "global" as const }
      : {}),
  }));
}

/** A daemon whose subscription the test drives with `send`. */
function fake(overrides: Partial<Daemon> = {}) {
  let listener: ((update: Update) => void) | undefined;
  const daemon = {
    status: vi.fn(() => Promise.resolve(STATUS)),
    tiles: vi.fn((profile: string, page: number, keys: number, _tile: Size) =>
      Promise.resolve(tiles(profile, page, keys)),
    ),
    outline: vi.fn(() => Promise.reject(new Error("the Pad reads no outline"))),
    press: vi.fn((key: number, kind: PressKind) =>
      Promise.resolve({ ...DRY_RUN, key, trigger: { press: kind, simulated: true } }),
    ),
    subscribe: vi.fn((_replay: number, next: (update: Update) => void) => {
      listener = next;
      return Promise.resolve(() => {
        listener = undefined;
      });
    }),
    ...overrides,
  };
  return {
    daemon: daemon satisfies Daemon,
    async send(...updates: Update[]) {
      await waitFor(() => expect(listener).toBeDefined());
      for (const update of updates) {
        listener?.(update);
      }
      await tick();
    },
    get listening() {
      return listener !== undefined;
    },
  };
}

function key(n: number): HTMLElement {
  return screen.getByRole("button", { name: new RegExp(`^Key ${n}\\b`) });
}

/** A key editor on blank keys whose writes succeed. */
function editor(overrides: Partial<Editor> = {}) {
  return {
    key: vi.fn((_target: Target) => Promise.resolve({ path: PROFILE, fields: {} })),
    editKey: vi.fn((_target: Target, fields: KeyFields) =>
      Promise.resolve({ path: PROFILE, before: {}, fields }),
    ),
    ...overrides,
  } satisfies Editor;
}

interface Options {
  daemon?: ReturnType<typeof fake>;
  keys?: ReturnType<typeof editor>;
  pinned?: Pinned;
  ontrace?: (entry: TraceRecord) => void;
}

/** The Pad with the pin held the way App holds it: `onpin` sets the prop. */
function pad({ daemon = fake(), keys = editor(), pinned, ontrace }: Options = {}) {
  const onpin = vi.fn<(next: Pinned) => void>();
  const result = render(Pad, {
    props: { daemon: daemon.daemon, editor: keys, profiles: PROFILES, pinned, onpin, ontrace },
  });
  onpin.mockImplementation((next) => void result.rerender({ pinned: next }));
  return { daemon, keys, onpin };
}

async function mounted(options: Options = {}) {
  const shown = pad(options);
  await waitFor(() => expect(key(5)).toHaveAccessibleName("Key 5: Test, exec cargo test"));
  return shown;
}

describe("the drawing", () => {
  it("shows the pad's page from the status in the drawing", async () => {
    const { daemon } = await mounted();
    const grid = screen.getByRole("grid", { name: "Keypad mirror: profile terminal, page 1" });
    expect(within(grid).getByRole("img", { name: t("device.keypad") })).toBeInTheDocument();
    expect(within(grid).getAllByRole("gridcell")).toHaveLength(9);
    expect(daemon.daemon.tiles).toHaveBeenCalledWith("terminal", 1, 9, TILE);
    expect(key(1).querySelector("img")).toHaveAttribute("src", "data:image/png;base64,terminal-1-1");
    expect(daemon.daemon.outline).not.toHaveBeenCalled();
  });

  it("follows the view events and fetches the new page", async () => {
    const { daemon } = await mounted();
    await daemon.send({ kind: "event", event: nth(eventsOf("page"), 0) });
    await waitFor(() => expect(daemon.daemon.tiles).toHaveBeenLastCalledWith("global", 2, 9, TILE));
    expect(
      screen.getByRole("grid", { name: "Keypad mirror: profile global, page 2" }),
    ).toBeInTheDocument();
  });

  it("lays out the keys the daemon's geometry reports, on the plain panel when the drawing has no cutouts for them", async () => {
    const rect = (x: number, y: number) => ({ x, y, width: 90, height: 80 });
    const geometry = {
      panel: { width: 200, height: 200 },
      tile: { width: 90, height: 80 },
      rects: [rect(0, 0), rect(100, 0), rect(0, 100), rect(100, 100)],
    };
    const status: Status = { ...STATUS, device: { connected: true, keys: 4, geometry } };
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    onTestFinished(() => warn.mockRestore());
    const { daemon } = pad({ daemon: fake({ status: vi.fn(() => Promise.resolve(status)) }) });
    await waitFor(() => expect(key(4)).toHaveAccessibleName("Key 4: Palette, chord ctrl+p, app layer"));
    expect(daemon.daemon.tiles).toHaveBeenCalledWith("terminal", 1, 4, { width: 90, height: 80 });
    expect(screen.queryByRole("img", { name: t("device.keypad") })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Key 5\b/ })).not.toBeInTheDocument();
  });

  it("shows no mirror until a Keypad has reported its geometry", async () => {
    const daemon = fake({ status: vi.fn(() => Promise.resolve(responses()[1] as Status)) });
    pad({ daemon });
    expect(await screen.findByText(/once a Keypad has reported its geometry/)).toBeInTheDocument();
    expect(screen.queryByRole("grid")).not.toBeInTheDocument();
    await daemon.send({ kind: "event", event: nth(eventsOf("device"), 0) });
    await waitFor(() => expect(key(9)).toBeInTheDocument());
    expect(daemon.daemon.tiles).toHaveBeenCalledWith("global", 1, 9, TILE);
  });

  it("says why a page has no tiles", async () => {
    const daemon = fake({
      tiles: vi.fn(() => Promise.reject(new Error("no profile `terminal` in the configuration directory"))),
    });
    pad({ daemon });
    expect(await screen.findByText(/no profile `terminal`/)).toBeInTheDocument();
    expect(key(1)).toHaveAccessibleName("Key 1: unbound");
  });

  it("marks the keys held down on the device", async () => {
    const { daemon } = await mounted();
    await daemon.send({ kind: "event", event: nth(eventsOf("key"), 0) });
    expect(key(5)).toHaveClass("held");
    await daemon.send({ kind: "event", event: nth(eventsOf("key"), 1) });
    expect(key(5)).not.toHaveClass("held");
  });

  it("subscribes with the event list's replay and stops when unmounted", async () => {
    const daemon = fake();
    const { unmount } = render(Pad, { props: { daemon: daemon.daemon, editor: editor() } });
    await waitFor(() => expect(daemon.listening).toBe(true));
    expect(daemon.daemon.subscribe).toHaveBeenCalledWith(PAD_TRACE, expect.any(Function));
    unmount();
    await waitFor(() => expect(daemon.listening).toBe(false));
  });
});

describe("the pin", () => {
  it("selects nothing at first: the inspector asks for a key and nothing is pinned", async () => {
    const { keys, onpin } = await mounted();
    expect(screen.getByText(t("editor.select"))).toBeInTheDocument();
    expect(keys.key).not.toHaveBeenCalled();
    expect(onpin).not.toHaveBeenCalled();
  });

  it("selecting a key pins the pad's profile and page; the inspector stays on it when the pad moves", async () => {
    const { daemon, keys, onpin } = await mounted();
    await fireEvent.click(key(5));
    expect(onpin).toHaveBeenCalledWith({ profile: "terminal", page: 1 });
    await waitFor(() => expect(keys.key).toHaveBeenLastCalledWith({ profile: "terminal", page: 1, key: 5 }));
    expect(screen.getByRole("heading", { name: "Key 5" })).toBeInTheDocument();
    // The drawing follows the pad; the inspector does not.
    await daemon.send({ kind: "event", event: nth(eventsOf("page"), 0) });
    await waitFor(() => expect(daemon.daemon.tiles).toHaveBeenLastCalledWith("global", 2, 9, TILE));
    expect(keys.key).toHaveBeenLastCalledWith({ profile: "terminal", page: 1, key: 5 });
    // A second selection keeps the pin.
    await fireEvent.click(key(6));
    expect(onpin).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(keys.key).toHaveBeenLastCalledWith({ profile: "terminal", page: 1, key: 6 }));
  });

  it("says where the pad moved and Follow unpins, keeping the key", async () => {
    const { daemon, keys, onpin } = await mounted();
    expect(screen.queryByText(/The pad moved/)).toBeNull();
    await fireEvent.click(key(7));
    await daemon.send({ kind: "event", event: nth(eventsOf("page"), 0) });
    const line = await screen.findByText("The pad moved to global, page 2");
    await fireEvent.click(within(line.closest("p") as HTMLElement).getByRole("button", { name: "Follow" }));
    expect(onpin).toHaveBeenLastCalledWith(undefined);
    await waitFor(() => expect(keys.key).toHaveBeenLastCalledWith({ profile: "global", page: 2, key: 7 }));
    expect(screen.queryByText(/The pad moved/)).toBeNull();
  });

  it("says nothing about a move while the pin and the pad agree", async () => {
    const { daemon } = await mounted({ pinned: { profile: "global", page: 2 } });
    expect(screen.getByText("The pad moved to terminal, page 1")).toBeInTheDocument();
    await daemon.send({ kind: "event", event: nth(eventsOf("page"), 0) });
    await waitFor(() => expect(screen.queryByText(/The pad moved/)).toBeNull());
  });
});

describe("the page tabs", () => {
  it("show the pad's profile while following, its page selected, and the accent", async () => {
    await mounted();
    const list = screen.getByRole("tablist", { name: "Pages" });
    expect(within(list).getAllByRole("tab").map((tab) => tab.textContent?.trim())).toEqual([
      "context",
      "git",
      "Page 3",
    ]);
    expect(within(list).getByRole("tab", { selected: true })).toHaveTextContent("context");
    expect(screen.getByRole("img", { name: "Accent #2ecc71" })).toBeInTheDocument();
  });

  it("a tab pins its page, and the inspector edits it", async () => {
    const { keys, onpin } = await mounted();
    await fireEvent.click(key(2));
    await fireEvent.click(screen.getByRole("tab", { name: "git" }));
    expect(onpin).toHaveBeenLastCalledWith({ profile: "terminal", page: 2 });
    await waitFor(() => expect(keys.key).toHaveBeenLastCalledWith({ profile: "terminal", page: 2, key: 2 }));
    expect(screen.getByRole("tab", { selected: true })).toHaveTextContent("git");
    expect(screen.getByText("The pad moved to terminal, page 1")).toBeInTheDocument();
  });

  it("a tab pins the pad's profile while following", async () => {
    const { onpin } = await mounted();
    await fireEvent.click(screen.getByRole("tab", { name: "Page 3" }));
    expect(onpin).toHaveBeenLastCalledWith({ profile: "terminal", page: 3 });
  });

  it("show the pinned profile's pages, none for a profile without pages", async () => {
    await mounted({ pinned: { profile: "empty", page: 1 } });
    const list = screen.getByRole("tablist", { name: "Pages" });
    expect(within(list).queryAllByRole("tab")).toHaveLength(0);
    expect(screen.getByRole("button", { name: "Add a page" })).toHaveAttribute("aria-disabled", "true");
    // The drawing keeps the pad.
    expect(screen.getByRole("grid", { name: "Keypad mirror: profile terminal, page 1" })).toBeInTheDocument();
  });
});

describe("the held caption", () => {
  it("names the held window and the profile the pad keeps", async () => {
    const { daemon } = await mounted();
    expect(screen.queryByText(/focused, holding/)).toBeNull();
    await daemon.send({ kind: "event", event: nth(eventsOf("context"), 2) });
    expect(screen.getByRole("status", { name: "Context" })).toHaveTextContent(
      "keyvo-app focused, holding terminal",
    );
    await daemon.send({ kind: "event", event: nth(eventsOf("context"), 0) });
    expect(screen.queryByText(/focused, holding/)).toBeNull();
  });
});

describe("dry-run presses", () => {
  it("a click selects and does not press", async () => {
    const { daemon } = await mounted();
    await fireEvent.click(key(5));
    expect(daemon.daemon.press).not.toHaveBeenCalled();
  });

  it("Space presses the selected key short, Shift+Space long, and the result opens its entry", async () => {
    const ontrace = vi.fn<(entry: TraceRecord) => void>();
    const { daemon } = await mounted({ ontrace });
    const grid = screen.getByRole("grid");
    await fireEvent.keyDown(grid, { key: "5" });
    await fireEvent.keyDown(grid, { key: " " });
    expect(daemon.daemon.press).toHaveBeenLastCalledWith(5, "short");
    expect(await screen.findByRole("status", { name: "Press result" })).toHaveTextContent(
      "Key 5, simulated, global layer: exec cargo test, dry run, nothing executed",
    );
    await fireEvent.keyDown(grid, { key: " ", shiftKey: true });
    expect(daemon.daemon.press).toHaveBeenLastCalledWith(5, "long");
    await fireEvent.click(await screen.findByRole("button", { name: "Show the trace entry" }));
    expect(ontrace).toHaveBeenCalledWith({ ...DRY_RUN, key: 5, trigger: { press: "long", simulated: true } });
  });

  it("a refused press shows why", async () => {
    const daemon = fake({ press: vi.fn(() => Promise.reject(new Error("cannot reach the keyvo daemon"))) });
    await mounted({ daemon });
    await fireEvent.keyDown(screen.getByRole("grid"), { key: "1" });
    await fireEvent.keyDown(screen.getByRole("grid"), { key: " " });
    await waitFor(() =>
      expect(screen.getByRole("status", { name: "Press result" })).toHaveTextContent(
        "Press failed: cannot reach the keyvo daemon",
      ),
    );
  });

  it("says how to press before the first one", async () => {
    await mounted();
    expect(screen.getByRole("status", { name: "Press result" })).toHaveTextContent(
      t("pad.resultHint"),
    );
  });
});

describe("the notices", () => {
  it("says the daemon is unreachable, how to start it, and retries on request", async () => {
    const status = vi
      .fn<Daemon["status"]>()
      .mockRejectedValueOnce(new Error("cannot reach the keyvo daemon at /run/user/1000/keyvo/keyvo.sock"))
      .mockResolvedValue(STATUS);
    const daemon = fake({ status });
    pad({ daemon });
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(
      "Daemon unreachable: cannot reach the keyvo daemon at /run/user/1000/keyvo/keyvo.sock",
    );
    expect(alert).toHaveTextContent("Start it with keyvo-daemon.");
    expect(within(alert).getByText("keyvo-daemon").tagName).toBe("CODE");
    expect(daemon.daemon.subscribe).not.toHaveBeenCalled();
    await fireEvent.click(within(alert).getByRole("button", { name: "Retry" }));
    await waitFor(() => expect(key(5)).toHaveAccessibleName("Key 5: Test, exec cargo test"));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("keeps the last frame when the subscription ends and says why", async () => {
    const { daemon } = await mounted();
    await daemon.send({ kind: "closed", reason: "the daemon closed the connection" });
    expect(screen.getByRole("alert")).toHaveTextContent("Daemon unreachable: the daemon closed the connection");
    expect(key(5)).toHaveAccessibleName("Key 5: Test, exec cargo test");
    await fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => expect(daemon.daemon.subscribe).toHaveBeenCalledTimes(2));
    await daemon.send({ kind: "subscribed", view: STATUS.view });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("says the daemon is unreachable when subscribing fails", async () => {
    pad({ daemon: fake({ subscribe: vi.fn(() => Promise.reject(new Error("no shell"))) }) });
    expect(await screen.findByRole("alert")).toHaveTextContent("Daemon unreachable: no shell");
  });

  it("shows a schema error with file, line and column until a reload succeeds, and fetches the tiles again", async () => {
    const { daemon } = await mounted();
    await daemon.send({ kind: "event", event: nth(eventsOf("config"), 1) });
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Schema error in profiles/global.toml, line 8, column 9: invalid type: integer `7`, expected a string",
    );
    await waitFor(() => expect(daemon.daemon.tiles).toHaveBeenCalledTimes(2));
    await daemon.send({ kind: "event", event: nth(eventsOf("config"), 0) });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    await waitFor(() => expect(daemon.daemon.tiles).toHaveBeenCalledTimes(3));
  });

  it("shows a reload error that names no place as it is", async () => {
    const { daemon } = await mounted();
    await daemon.send({ kind: "event", event: { event: "config", reloaded: false, error: "no config dir" } });
    expect(screen.getByRole("alert")).toHaveTextContent("Configuration not loaded: no config dir");
  });

  it("notes a missing Keypad and when it was last seen; the mirror keeps the last frame", async () => {
    const seen = new Date(2026, 9, 26, 9, 30, 15).getTime();
    const now = vi.spyOn(Date, "now").mockReturnValue(seen);
    onTestFinished(() => now.mockRestore());
    const { daemon } = await mounted();
    expect(screen.queryByText(/is missing/)).not.toBeInTheDocument();
    now.mockReturnValue(seen + 60_000);
    await daemon.send({ kind: "event", event: nth(eventsOf("device"), 1) });
    const note = screen.getByText(/is missing/);
    expect(note).toHaveTextContent(`The Keypad is missing; last seen at ${new Date(seen).toLocaleTimeString()}.`);
    expect(note.closest(".notice")).toHaveClass("note");
    expect(key(5)).toHaveAccessibleName("Key 5: Test, exec cargo test");
  });

  it("notes a Keypad not seen since the app started", async () => {
    pad({ daemon: fake({ status: vi.fn(() => Promise.resolve(responses()[1] as Status)) }) });
    expect(
      await screen.findByText("The Keypad is missing; not seen since the app started."),
    ).toBeInTheDocument();
  });
});

describe("connecting", () => {
  it("shows skeleton tiles and no keys while connecting", () => {
    pad({ daemon: fake({ status: vi.fn(() => new Promise<Status>(() => {})) }) });
    expect(screen.queryByRole("grid")).not.toBeInTheDocument();
    expect(screen.queryByText(/reported its geometry/)).not.toBeInTheDocument();
    const loading = screen.getByRole("list", { name: "Connecting to the daemon" });
    expect(loading).toHaveAttribute("aria-busy", "true");
    expect(within(loading).getAllByRole("listitem")).toHaveLength(9);
    expect(screen.queryByText(/No presses yet/)).not.toBeInTheDocument();
  });

  it("keeps the mirror's last frame while it connects again", async () => {
    const status = vi
      .fn<Daemon["status"]>()
      .mockResolvedValueOnce(STATUS)
      .mockReturnValue(new Promise<Status>(() => {}));
    const { daemon } = await mounted({ daemon: fake({ status }) });
    await daemon.send({ kind: "closed", reason: "the daemon closed the connection" });
    await fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(screen.queryByRole("list", { name: "Connecting to the daemon" })).toBeNull();
    expect(key(5).querySelector("img")).toHaveAttribute("src", "data:image/png;base64,terminal-1-5");
  });
});

describe("the event list", () => {
  it("lists the trace entries newest first, one line each, and hands one to the drawer", async () => {
    const ontrace = vi.fn<(entry: TraceRecord) => void>();
    const { daemon } = await mounted({ ontrace });
    const entries = eventsOf("trace");
    await daemon.send(...entries.map((event): Update => ({ kind: "event", event })));
    const items = within(screen.getByRole("list", { name: "Recent events" })).getAllByRole("listitem");
    expect(items.map((item) => item.textContent?.trim())).toEqual([
      expect.stringContaining("Key 1: unbound"),
      expect.stringContaining("Key 5 long, simulated, app layer: exec cargo test, dry run, nothing executed"),
      expect.stringContaining("Key 9, app layer: exec gh pr view --web, failed: no such file or directory: gh"),
      expect.stringContaining("Key 1, app layer: exec cargo build, exited 101 after 2318 ms"),
      expect.stringContaining("Key 4, app layer: chord ctrl+p, sent"),
    ]);
    await fireEvent.click(
      screen.getByRole("button", {
        name: "Key 9, app layer: exec gh pr view --web, failed: no such file or directory: gh, open the trace entry",
      }),
    );
    expect(ontrace).toHaveBeenCalledWith(nth(entries, 2).entry);
  });

  it("says when nothing happened yet and how to fill the list", async () => {
    await mounted();
    expect(screen.getByText(t("home.noEvents"))).toBeInTheDocument();
  });
});

describe("the key editor", () => {
  it("fetches the tiles again after an edit", async () => {
    const { daemon, keys } = await mounted();
    await fireEvent.click(key(1));
    const label = await screen.findByRole("textbox", { name: "Label" });
    await fireEvent.input(label, { target: { value: "Shell" } });
    await fireEvent.change(label);
    await waitFor(() =>
      expect(keys.editKey).toHaveBeenCalledWith({ profile: "terminal", page: 1, key: 1 }, { label: "Shell" }),
    );
    await waitFor(() => expect(daemon.daemon.tiles).toHaveBeenCalledTimes(2));
  });
});

describe("the style", () => {
  const SRC = dirname(fileURLToPath(import.meta.url));
  /** The rules of the Pad's own components, as written. */
  const css = ["Pad.svelte", "Mirror.svelte", "PageTabs.svelte"]
    .map((file) => readFileSync(join(SRC, file), "utf8"))
    .flatMap((source) => [...source.matchAll(/<style>([\s\S]*?)<\/style>/g)].map((match) => match[1] ?? ""))
    .join("\n");

  it("draws no side stripe and writes no text in the accent", async () => {
    const { daemon } = await mounted({ pinned: { profile: "global", page: 1 } });
    await daemon.send({ kind: "event", event: nth(eventsOf("config"), 1) });
    for (const element of document.body.querySelectorAll<HTMLElement>("*")) {
      const style = getComputedStyle(element);
      expect(style.borderLeftWidth, element.outerHTML).not.toBe("4px");
      expect(style.color, element.outerHTML).not.toContain("--kv-accent");
    }
    // jsdom applies no component styles: the rules themselves.
    expect(css).not.toMatch(/border-left(-width)?:\s*4px/);
    expect(css).not.toMatch(/(^|[\s;{])color:\s*var\(--kv-accent\)/);
  });
});
