// The Pad screen (pad screen brief §5.2, §6) against a fake daemon fed with
// the shared socket fixtures (tests/protocol-fixtures/socket/): the
// drawing with the pad's tiles, the pin a selected key sets and the
// "pad moved" line, the page list tabs (#151), the held caption,
// the dry-run presses, the notices and the event list. The inspector
// against a fake editor, for what the Pad hands it: the pinned page's tile,
// the global key under a fall-through key, Press; Inspector.test.ts tests
// the inspector itself, Mirror.test.ts and PageTabs.test.ts their
// components.
import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { tick } from "svelte";
import { describe, expect, it, onTestFinished, vi } from "vitest";

import Pad from "./Pad.svelte";
import { DRAG_TYPE } from "./lib/catalogue";
import type { Daemon, Tile, Update } from "./lib/daemon";
import type { Editor, KeyFields, Target } from "./lib/editor";
import { eventsOf, nth, recipeReplies, responses } from "./lib/fixtures";
import { t } from "./lib/i18n";
import { SNAPSHOTS } from "./lib/mock/data";
import { mockClients } from "./lib/mock";
import { PAD_TRACE, STRIP_LINE } from "./lib/pad";
import type { Pinned } from "./lib/pin";
import type { ProfileSummary } from "./lib/profiles";
import type { PressKind, Size, Status, TraceRecord } from "./lib/protocol";
import { layout, type Columns } from "./lib/shell";

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
  summary("global", ["context", "general", "apps"]),
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
    recipes: vi.fn(() => Promise.resolve(nth(recipeReplies(), 0))),
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
  profiles?: ProfileSummary[];
  ontrace?: (entry: TraceRecord) => void;
  /** The window's columns; the default window shows them all. */
  columns?: Columns;
}

/** The Pad with the pin held the way App holds it: `onpin` sets the prop. */
function pad({
  daemon = fake(),
  keys = editor(),
  pinned,
  profiles = PROFILES,
  ontrace,
  columns = layout(1200),
}: Options = {}) {
  const onpin = vi.fn<(next: Pinned) => void>();
  const result = render(Pad, {
    props: { daemon: daemon.daemon, editor: keys, profiles, pinned, onpin, ontrace, columns },
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

  it("shows the lock page and says so while the session is locked", async () => {
    const { daemon } = await mounted();
    expect(screen.queryByText(t("pad.locked"))).not.toBeInTheDocument();
    await daemon.send({ kind: "event", event: { event: "session", locked: true } });
    await waitFor(() =>
      expect(daemon.daemon.tiles).toHaveBeenLastCalledWith("terminal", 1, 9, TILE, true),
    );
    expect(screen.getByRole("status", { name: t("pad.context") })).toHaveTextContent(
      "Session locked; the pad shows the lock.",
    );
    await daemon.send({ kind: "event", event: { event: "session", locked: false } });
    await waitFor(() => expect(daemon.daemon.tiles).toHaveBeenLastCalledWith("terminal", 1, 9, TILE));
    expect(screen.queryByText(t("pad.locked"))).not.toBeInTheDocument();
  });

  it("shows the mock's lock tile and line under ?mock-locked", async () => {
    const { daemon } = mockClients("?mock-locked", document.createElement("div"));
    render(Pad, { props: { daemon, editor: editor(), profiles: PROFILES } });
    expect(await screen.findByText(t("pad.locked"))).toBeInTheDocument();
    await waitFor(() => expect(key(5).querySelector("img")).toHaveAttribute("src", SNAPSHOTS.lock));
    expect(key(5)).toHaveAccessibleName("Key 5: unbound");
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

  it("draws empty wells and No pad until a Keypad has reported its geometry", async () => {
    const daemon = fake({ status: vi.fn(() => Promise.resolve(responses()[1] as Status)) });
    pad({ daemon });
    const drawing = await screen.findByRole("figure", { name: "No pad" });
    expect(drawing.querySelectorAll(".well")).toHaveLength(9);
    expect(screen.queryByRole("grid")).not.toBeInTheDocument();
    expect(daemon.daemon.tiles).not.toHaveBeenCalled();
    await daemon.send({ kind: "event", event: nth(eventsOf("device"), 0) });
    await waitFor(() => expect(key(9)).toBeInTheDocument());
    expect(screen.queryByRole("figure", { name: "No pad" })).not.toBeInTheDocument();
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
    expect(screen.getByText(t("inspector.select"))).toBeInTheDocument();
    expect(keys.key).not.toHaveBeenCalled();
    expect(onpin).not.toHaveBeenCalled();
  });

  it("selecting a key pins the pad's profile and page; the inspector stays on it when the pad moves", async () => {
    const { daemon, keys, onpin } = await mounted();
    await fireEvent.click(key(5));
    expect(onpin).toHaveBeenCalledWith({ profile: "terminal", page: 1, fromKey: true });
    await waitFor(() => expect(keys.key).toHaveBeenLastCalledWith({ profile: "terminal", page: 1, key: 5 }));
    expect(screen.getByRole("heading", { name: "Key 5: No action" })).toBeInTheDocument();
    // A second selection keeps the pin.
    await fireEvent.click(key(6));
    expect(onpin).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(keys.key).toHaveBeenLastCalledWith({ profile: "terminal", page: 1, key: 6 }));
    // The drawing follows the pad; the inspector does not.
    await daemon.send({ kind: "event", event: nth(eventsOf("page"), 0) });
    await waitFor(() => expect(daemon.daemon.tiles).toHaveBeenCalledWith("global", 2, 9, TILE));
    // The pinned page's tiles for the inspector, now that the pad shows another.
    await waitFor(() => expect(daemon.daemon.tiles).toHaveBeenLastCalledWith("terminal", 1, 9, TILE));
    expect(keys.key).toHaveBeenLastCalledWith({ profile: "terminal", page: 1, key: 6 });
  });

  it("says where the pad moved and Follow unpins, keeping the key", async () => {
    const { daemon, keys, onpin } = await mounted();
    expect(screen.queryByText(/the pad shows/)).toBeNull();
    await fireEvent.click(key(7));
    await daemon.send({ kind: "event", event: nth(eventsOf("page"), 0) });
    const line = await screen.findByText("Editing terminal, key 7; the pad shows global, page 2.");
    // Follow leads the line, the sentence comes after it.
    const follow = within(line.closest("p") as HTMLElement).getByRole("button", { name: "Follow" });
    expect(follow.parentElement?.firstElementChild).toBe(follow);
    expect(follow.compareDocumentPosition(line)).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    await fireEvent.click(follow);
    expect(onpin).toHaveBeenLastCalledWith(undefined);
    await waitFor(() => expect(keys.key).toHaveBeenLastCalledWith({ profile: "global", page: 2, key: 7 }));
    expect(screen.queryByText(/the pad shows/)).toBeNull();
  });

  it("drops the outline while the pad is away, names the pinned key, and a click pins again", async () => {
    const { daemon, keys, onpin } = await mounted();
    const selection = () =>
      screen.getAllByRole("gridcell").filter((cell) => cell.getAttribute("aria-selected") === "true");
    await fireEvent.click(key(7));
    // Pinned and shown equal: the outline is on the key.
    expect(selection()).toEqual([key(7).closest('[role="gridcell"]')]);
    await daemon.send({ kind: "event", event: nth(eventsOf("page"), 0) });
    // Moved: no outline, the line names the pinned key, the inspector stays.
    await screen.findByText("Editing terminal, key 7; the pad shows global, page 2.");
    expect(selection()).toEqual([]);
    expect(keys.key).toHaveBeenLastCalledWith({ profile: "terminal", page: 1, key: 7 });
    // Re-pinned: a click pins the shown page and the outline is back.
    await fireEvent.click(key(2));
    expect(onpin).toHaveBeenLastCalledWith({ profile: "global", page: 2, fromKey: true });
    await waitFor(() => expect(keys.key).toHaveBeenLastCalledWith({ profile: "global", page: 2, key: 2 }));
    expect(screen.queryByText(/the pad shows/)).toBeNull();
    expect(selection()).toEqual([key(2).closest('[role="gridcell"]')]);
  });

  it("names the profile the pad shows, as the header does, not its id", async () => {
    const named = (id: string, name: string) => ({ ...summary(id, ["context"]), name });
    const profiles = [named("global", "Global"), named("terminal", "Konsole"), named("vscode", "VS Code")];
    const { daemon } = await mounted({ profiles });
    await fireEvent.click(key(3));
    await daemon.send({ kind: "event", event: { ...nth(eventsOf("profile"), 0), profile: "vscode" } });
    await screen.findByText("Editing Konsole, key 3; the pad shows VS Code, page 1.");
    expect(screen.getByRole("grid", { name: "Keypad mirror: profile vscode, page 1" })).toBeInTheDocument();
    await daemon.send({ kind: "event", event: { ...nth(eventsOf("profile"), 0), profile: "global" } });
    await screen.findByText("Editing Konsole, key 3; the pad shows Global, page 1.");
    // A profile the list does not know yet keeps its id.
    await daemon.send({ kind: "event", event: { ...nth(eventsOf("profile"), 0), profile: "new" } });
    await screen.findByText("Editing Konsole, key 3; the pad shows new, page 1.");
  });

  it("says nothing about a move while the pin and the pad agree", async () => {
    const { daemon } = await mounted({ pinned: { profile: "global", page: 2 } });
    expect(
      screen.getByText("Editing global, page 2; the pad shows terminal, page 1."),
    ).toBeInTheDocument();
    await daemon.send({ kind: "event", event: nth(eventsOf("page"), 0) });
    await waitFor(() => expect(screen.queryByText(/the pad shows/)).toBeNull());
  });
});

describe("the page tabs", () => {
  const names = () =>
    within(screen.getByRole("tablist", { name: "Pages" }))
      .getAllByRole("tab")
      .map((tab) => tab.textContent?.trim());

  it("show the pad's page list while following: the global pages muted, then the profile's", async () => {
    await mounted();
    const list = screen.getByRole("tablist", { name: "Pages" });
    expect(names()).toEqual(["apps", "general", "context", "git", "Page 3"]);
    expect(within(list).getAllByRole("tab").map((tab) => tab.classList.contains("global"))).toEqual([
      true,
      true,
      false,
      false,
      false,
    ]);
    expect(within(list).getByRole("tab", { name: "general, a page of global" })).toBeInTheDocument();
    expect(within(list).getByRole("tab", { selected: true })).toHaveTextContent("context");
    expect(screen.getByRole("img", { name: "Accent #2ecc71" })).toBeInTheDocument();
  });

  it("select the tab at the index the daemon reports, the list staying the pad's", async () => {
    const { daemon } = await mounted();
    // The fixture's page event: the global general page at index 0.
    await daemon.send({ kind: "event", event: nth(eventsOf("page"), 0) });
    await waitFor(() => expect(screen.getByRole("tab", { selected: true })).toHaveTextContent("general"));
    expect(names()).toEqual(["apps", "general", "context", "git", "Page 3"]);
    await daemon.send({
      kind: "event",
      event: { event: "page", profile: "terminal", page: 2, mode: "auto", index: 2 },
    });
    await waitFor(() => expect(screen.getByRole("tab", { selected: true })).toHaveTextContent("git"));
  });

  it("a global tab pins that page of the global profile and keeps the list", async () => {
    const { onpin } = await mounted();
    await fireEvent.click(screen.getByRole("tab", { name: "general, a page of global" }));
    expect(onpin).toHaveBeenLastCalledWith({ profile: "global", page: 2 });
    expect(names()).toEqual(["apps", "general", "context", "git", "Page 3"]);
    expect(screen.getByRole("tab", { selected: true })).toHaveTextContent("general");
    expect(
      screen.getByText("Editing global, page 2; the pad shows terminal, page 1."),
    ).toBeInTheDocument();
  });

  it("show the global profile's own list for its pinned context page", async () => {
    await mounted({ pinned: { profile: "global", page: 1 } });
    expect(names()).toEqual(["apps", "general", "context"]);
    expect(screen.getByRole("tab", { selected: true })).toHaveTextContent("context");
  });

  it("a tab pins its page, and the inspector edits it", async () => {
    const { keys, onpin } = await mounted();
    await fireEvent.click(key(2));
    await fireEvent.click(screen.getByRole("tab", { name: "git" }));
    expect(onpin).toHaveBeenLastCalledWith({ profile: "terminal", page: 2 });
    await waitFor(() => expect(keys.key).toHaveBeenLastCalledWith({ profile: "terminal", page: 2, key: 2 }));
    expect(screen.getByRole("tab", { selected: true })).toHaveTextContent("git");
    expect(
      screen.getByText("Editing terminal, key 2; the pad shows terminal, page 1."),
    ).toBeInTheDocument();
  });

  it("mark the pinned page's tab while the pad shows another page, and no tab once they agree", async () => {
    const { daemon } = await mounted();
    const marked = () =>
      within(screen.getByRole("tablist", { name: "Pages" }))
        .getAllByRole("tab")
        .filter((tab) => tab.querySelector(".icon") !== null);
    expect(marked()).toEqual([]);
    await fireEvent.click(key(4));
    expect(marked()).toEqual([]);
    await daemon.send({ kind: "event", event: nth(eventsOf("page"), 0) });
    await screen.findByText("Editing terminal, key 4; the pad shows global, page 2.");
    const pinned = screen.getByRole("tab", { name: "context, pinned" });
    expect(marked()).toEqual([pinned]);
    expect(pinned).toHaveAttribute("aria-selected", "true");
    // The #143 decision stands: no outline on the mirror while the pad is away.
    expect(screen.getAllByRole("gridcell").filter((cell) => cell.getAttribute("aria-selected") === "true")).toEqual([]);
    await daemon.send({
      kind: "event",
      event: { event: "page", profile: "terminal", page: 1, mode: "auto", index: 1 },
    });
    await waitFor(() => expect(marked()).toEqual([]));
  });

  it("a tab pins the pad's profile while following", async () => {
    const { onpin } = await mounted();
    await fireEvent.click(screen.getByRole("tab", { name: "Page 3" }));
    expect(onpin).toHaveBeenLastCalledWith({ profile: "terminal", page: 3 });
  });

  it("show the pinned profile's list, only the global pages for a profile without pages", async () => {
    await mounted({ pinned: { profile: "empty", page: 1 } });
    const list = screen.getByRole("tablist", { name: "Pages" });
    expect(names()).toEqual(["apps", "general"]);
    expect(within(list).queryByRole("tab", { selected: true })).toBeNull();
    const add = screen.getByRole("button", { name: "Add a page" });
    expect(add).toHaveAttribute("aria-disabled", "true");
    expect(within(list).getAllByRole("tab").at(-1)?.compareDocumentPosition(add)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
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
    // Nothing to select: no "Select a key" anywhere, the two columns say why.
    expect(screen.queryAllByText(/Select a key/)).toEqual([]);
    expect(screen.getByRole("status", { name: t("home.result") })).toHaveTextContent("");
    expect(screen.getByText("No pad connected.")).toBeInTheDocument();
    expect(screen.getByText("Connect the daemon to assign actions.")).toBeInTheDocument();
    await fireEvent.click(within(alert).getByRole("button", { name: "Retry" }));
    await waitFor(() => expect(key(5)).toHaveAccessibleName("Key 5: Test, exec cargo test"));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByText("Select a key to edit it.")).toBeInTheDocument();
    expect(screen.getByRole("status", { name: t("home.result") })).toHaveTextContent(
      t("pad.resultHint"),
    );
  });

  it("says nothing to select while it connects", () => {
    pad();
    expect(screen.queryAllByText(/Select a key/)).toEqual([]);
    expect(screen.getByRole("status", { name: t("home.result") })).toHaveTextContent("");
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

  it("fetches no tiles on the closed update and keeps the last frame and no tiles note", async () => {
    const fetch = vi.fn((profile: string, page: number, keys: number, _tile: Size) =>
      Promise.resolve(tiles(profile, page, keys)),
    );
    const { daemon } = await mounted({ daemon: fake({ tiles: fetch }) });
    const fetches = fetch.mock.calls.length;
    fetch.mockImplementation(() =>
      Promise.reject(new Error("cannot reach the keyvo daemon at /run/user/1000/keyvo/keyvo.sock")),
    );
    await daemon.send({ kind: "closed", reason: "the daemon closed the connection" });
    await tick();
    expect(fetch).toHaveBeenCalledTimes(fetches);
    expect(key(5).querySelector("img")).toHaveAttribute("src", "data:image/png;base64,terminal-1-5");
    expect(screen.queryByText(/Tiles not rendered/)).not.toBeInTheDocument();
  });

  it("disables Press on the selected key while the daemon cannot be reached", async () => {
    const { daemon } = await mounted();
    await fireEvent.click(key(5));
    expect(await screen.findByRole("button", { name: "Press" })).toBeEnabled();
    await daemon.send({ kind: "closed", reason: "the daemon closed the connection" });
    expect(screen.getByRole("button", { name: "Press" })).toBeDisabled();
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

  it("draws the device with empty wells and No pad under ?mock-error=unreachable", async () => {
    const { daemon } = mockClients("?mock-error=unreachable", document.createElement("div"));
    const { container } = render(Pad, { props: { daemon, editor: editor(), profiles: PROFILES } });
    await screen.findByRole("alert");
    const drawing = screen.getByRole("figure", { name: "No pad" });
    expect(within(drawing).getByRole("img", { name: t("device.keypad") })).toBeInTheDocument();
    expect(drawing.querySelectorAll(".well")).toHaveLength(9);
    expect(screen.queryByRole("grid")).not.toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "Connecting to the daemon" })).not.toBeInTheDocument();
    // The inspector and the tabs keep their empty states, the result line stays empty.
    expect(screen.getByText("No pad connected.")).toBeInTheDocument();
    expect(screen.queryByRole("tablist")).not.toBeInTheDocument();
    expect(screen.getByRole("status", { name: t("home.result") })).toHaveTextContent("");
    expect(container.querySelector(".drawing .note")).toBeNull();
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
    expect(screen.queryByRole("figure", { name: "No pad" })).not.toBeInTheDocument();
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

describe("the trace strip", () => {
  it("lists the trace entries newest first, one line each, and hands one to the drawer", async () => {
    const ontrace = vi.fn<(entry: TraceRecord) => void>();
    const { daemon } = await mounted({ ontrace });
    const entries = eventsOf("trace");
    await daemon.send(...entries.map((event): Update => ({ kind: "event", event })));
    const strip = screen.getByRole("region", { name: "Recent events" });
    expect(within(strip).getAllByRole("listitem")).toHaveLength(STRIP_LINE);
    await fireEvent.click(within(strip).getByRole("button", { name: t("trace.expand") }));
    const items = within(screen.getByRole("list", { name: "Recent events" })).getAllByRole("listitem");
    expect(items.map((item) => item.textContent?.trim())).toEqual([
      expect.stringContaining("Key 5: blocked, session locked"),
      expect.stringContaining("Key 2, global layer: launch flatpak run org.kde.kcalc, started, pid 4711"),
      expect.stringContaining("Key 1: unbound"),
      expect.stringContaining("Key 5 long, simulated, app layer: exec cargo test, dry run, nothing executed"),
      expect.stringContaining("Key 9, app layer: exec gh pr view --web, failed: no such file or directory: gh"),
      expect.stringContaining("Key 1, app layer: exec cargo build, exited 101 after 2318 ms"),
      expect.stringContaining("Key 4, app layer: chord ctrl+p, sent"),
    ]);
    await fireEvent.click(
      screen.getByRole("button", {
        name: /^\d\d:\d\d:\d\d Key 9, app layer: exec gh pr view --web, failed: no such file or directory: gh, open the trace entry$/,
      }),
    );
    expect(ontrace).toHaveBeenCalledWith(nth(entries, 2).entry);
  });

  it("says when nothing happened yet and how to fill the list", async () => {
    await mounted();
    expect(screen.getByText(t("home.noEvents"))).toBeInTheDocument();
  });
});

describe("the inspector", () => {
  it("shows the tile of the pad's page while the pin agrees, without fetching again", async () => {
    const { daemon } = await mounted();
    await fireEvent.click(key(2));
    expect(await screen.findByRole("img", { name: "Tile of key 2" })).toHaveAttribute(
      "src",
      "data:image/png;base64,terminal-1-2",
    );
    expect(daemon.daemon.tiles).toHaveBeenCalledTimes(1);
  });

  it("shows the tile of the pinned page when the pad shows another", async () => {
    const { daemon } = await mounted({ pinned: { profile: "terminal", page: 2 } });
    await fireEvent.click(key(2));
    await waitFor(() => expect(daemon.daemon.tiles).toHaveBeenCalledWith("terminal", 2, 9, TILE));
    expect(await screen.findByRole("img", { name: "Tile of key 2" })).toHaveAttribute(
      "src",
      "data:image/png;base64,terminal-2-2",
    );
    // A page the user chose stays pinned; the mirror's key 2 is another page's.
    expect(screen.getByText("Editing terminal, key 2; the pad shows terminal, page 1.")).toBeInTheDocument();
    expect(key(2)).not.toHaveClass("selected");
  });

  it("reads the global key under a key that falls through, and offers the override", async () => {
    const { keys } = await mounted();
    await fireEvent.click(key(3));
    await waitFor(() => expect(keys.key).toHaveBeenCalledWith({ profile: "global", page: 1, key: 3 }));
    expect(await screen.findByText("from global")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Override here" })).toBeInTheDocument();
    await fireEvent.click(key(4));
    await waitFor(() => expect(screen.queryByText("from global")).toBeNull());
  });

  it("has no global key under a key of the global profile", async () => {
    const { keys } = await mounted({ pinned: { profile: "global", page: 1 } });
    await fireEvent.click(key(4));
    await waitFor(() => expect(keys.key).toHaveBeenCalledWith({ profile: "global", page: 1, key: 4 }));
    expect(screen.queryByText("from global")).toBeNull();
  });

  it("presses the selected key as a dry run, and the result shows under the drawing", async () => {
    const { daemon } = await mounted();
    await fireEvent.click(key(5));
    await fireEvent.click(await screen.findByRole("button", { name: "Press" }));
    expect(daemon.daemon.press).toHaveBeenCalledWith(5, "short");
    expect(await screen.findByRole("status", { name: "Press result" })).toHaveTextContent(
      "Key 5, simulated, global layer: exec cargo test, dry run, nothing executed",
    );
  });

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

describe("the actions catalogue", () => {
  /** jsdom has no DataTransfer: a plain object with `getData` stands in. */
  function transfer(data: string) {
    return { getData: (type: string) => (type === DRAG_TYPE ? data : ""), dropEffect: "none" };
  }

  function cell(n: number): HTMLElement {
    return key(n).closest('[role="gridcell"]') as HTMLElement;
  }

  function action(): HTMLSelectElement {
    return screen.getByRole("combobox", { name: "Action" }) as HTMLSelectElement;
  }

  it("is the right column from 1200 px, and the Action select carries no entries", async () => {
    await mounted({ columns: layout(1200) });
    expect(screen.getByRole("region", { name: "Actions" })).toBeInTheDocument();
    await fireEvent.click(key(5));
    await waitFor(() => expect(action()).toBeInTheDocument());
    expect(within(action()).queryAllByRole("group")).toEqual([]);
  });

  it("collapses into the Action select at 1199 px", async () => {
    await mounted({ columns: layout(1199) });
    expect(screen.queryByRole("region", { name: "Actions" })).toBeNull();
    await fireEvent.click(key(5));
    await waitFor(() => expect(action()).toBeInTheDocument());
    const groups = within(action()).getAllByRole("group");
    expect(groups.map((group) => group.getAttribute("label"))).toEqual(["General", "System", "Apps"]);
    await waitFor(() => expect(within(action()).getByRole("option", { name: "Konsole" })).toBeEnabled());
  });

  it("lists the presets of the daemon's recipes reply", async () => {
    const { daemon } = await mounted({ columns: layout(1200) });
    expect(daemon.daemon.recipes).toHaveBeenCalledTimes(1);
    await fireEvent.click(await screen.findByRole("button", { name: "Apps" }));
    const apps = screen.getByRole("list", { name: "Apps" });
    expect(within(apps).getAllByRole("button").map((button) => button.textContent?.trim())).toEqual([
      "Calculator",
      "Konsole",
      "Screenshot",
      "Spectacle",
      "VS Code",
    ]);
  });

  it("reads the recipes again after a configuration reload", async () => {
    const { daemon } = await mounted({ columns: layout(1200) });
    await daemon.send({ kind: "event", event: nth(eventsOf("config"), 0) });
    await waitFor(() => expect(daemon.daemon.recipes).toHaveBeenCalledTimes(2));
  });

  it("shows the raw tools and why, when the recipes cannot be read", async () => {
    await mounted({
      columns: layout(1200),
      daemon: fake({ recipes: vi.fn(() => Promise.reject(new Error("connection refused"))) }),
    });
    expect(
      await screen.findByText("The presets could not be read: connection refused"),
    ).toBeInTheDocument();
    expect(within(screen.getByRole("list", { name: "General" })).getAllByRole("button")).toHaveLength(3);
  });

  it("names the user packs the daemon left out", async () => {
    await mounted({
      columns: layout(1200),
      daemon: fake({ recipes: vi.fn(() => Promise.resolve(nth(recipeReplies(), 1))) }),
    });
    expect(await screen.findByRole("list", { name: "Recipe packs left out" })).toHaveTextContent(
      "volume.toml:12:19",
    );
  });

  it("Spectacle dropped on key 7 writes its default and offers What it does", async () => {
    const { keys } = await mounted({ columns: layout(1200) });
    await fireEvent.click(await screen.findByRole("button", { name: "Apps" }));
    await screen.findByRole("button", { name: "Spectacle" });
    await fireEvent.drop(cell(7), { dataTransfer: transfer("recipe:spectacle") });
    const at = { profile: "terminal", page: 1, key: 7 };
    await waitFor(() =>
      expect(keys.editKey).toHaveBeenCalledWith(at, {
        icon: "lucide:camera",
        label: "Capture",
        action: { type: "exec", cmd: "spectacle --fullscreen" },
      }),
    );
    const what = await screen.findByRole("combobox", { name: "What it does" });
    await fireEvent.change(what, { target: { value: "region" } });
    await waitFor(() =>
      expect(keys.editKey).toHaveBeenLastCalledWith(at, {
        icon: "lucide:camera",
        label: "Capture",
        action: { type: "exec", cmd: "spectacle --region" },
      }),
    );
  });

  it("a click on Volume up assigns it to the selected key", async () => {
    const { keys } = await mounted({ columns: layout(1200) });
    await fireEvent.click(key(8));
    await waitFor(() => expect(action()).toHaveValue("none"));
    await fireEvent.click(await screen.findByRole("button", { name: "System" }));
    await fireEvent.click(screen.getByRole("button", { name: "Volume up" }));
    await waitFor(() =>
      expect(keys.editKey).toHaveBeenCalledWith(
        { profile: "terminal", page: 1, key: 8 },
        expect.objectContaining({ label: "Vol +", icon: "lucide:volume-2" }),
      ),
    );
  });

  it("Exec dropped on key 7 selects and pins the key and sets Exec without writing", async () => {
    const { keys, onpin } = await mounted({ columns: layout(1200) });
    await fireEvent.drop(cell(7), { dataTransfer: transfer("tool:exec") });
    expect(onpin).toHaveBeenCalledWith({ profile: "terminal", page: 1, fromKey: true });
    expect(key(7)).toHaveClass("selected");
    await waitFor(() => expect(action()).toHaveValue("exec"));
    expect(screen.getByRole("heading", { name: /^Key 7/ })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Command" })).toHaveValue("");
    expect(keys.editKey).not.toHaveBeenCalled();
    const command = screen.getByRole("textbox", { name: "Command" });
    await fireEvent.input(command, { target: { value: "kate" } });
    await fireEvent.change(command);
    await waitFor(() =>
      expect(keys.editKey).toHaveBeenCalledWith(
        { profile: "terminal", page: 1, key: 7 },
        { action: { type: "exec", cmd: "kate" } },
      ),
    );
  });

  it("an entry dropped on a tile while the pad is away pins the shown page and goes to its key", async () => {
    const { daemon, keys, onpin } = await mounted({ columns: layout(1200) });
    await fireEvent.click(key(8));
    await daemon.send({ kind: "event", event: nth(eventsOf("page"), 0) });
    await screen.findByText(/the pad shows global, page 2/);
    await fireEvent.drop(cell(7), { dataTransfer: transfer("tool:exec") });
    expect(onpin).toHaveBeenLastCalledWith({ profile: "global", page: 2, fromKey: true });
    await waitFor(() => expect(keys.key).toHaveBeenLastCalledWith({ profile: "global", page: 2, key: 7 }));
    await waitFor(() => expect(action()).toHaveValue("exec"));
    expect(key(7)).toHaveClass("selected");
  });

  it("a click on an entry while the pad is away assigns it to the pinned key", async () => {
    const { daemon, keys, onpin } = await mounted({ columns: layout(1200) });
    await fireEvent.click(key(8));
    await waitFor(() => expect(action()).toHaveValue("none"));
    await daemon.send({ kind: "event", event: nth(eventsOf("page"), 0) });
    await screen.findByText(/the pad shows global, page 2/);
    await fireEvent.click(await screen.findByRole("button", { name: "System" }));
    await fireEvent.click(screen.getByRole("button", { name: "Volume up" }));
    await waitFor(() =>
      expect(keys.editKey).toHaveBeenCalledWith(
        { profile: "terminal", page: 1, key: 8 },
        expect.objectContaining({ label: "Vol +" }),
      ),
    );
    expect(onpin).toHaveBeenCalledTimes(1);
  });

  it("a click on Chord assigns it to the selected key", async () => {
    await mounted({ columns: layout(1200) });
    await fireEvent.click(key(2));
    await waitFor(() => expect(action()).toHaveValue("none"));
    await fireEvent.click(screen.getByRole("button", { name: "Chord" }));
    await waitFor(() => expect(action()).toHaveValue("chord"));
    expect(screen.getByRole("textbox", { name: "Chord" })).toHaveValue("");
  });
});

describe("the style", () => {
  const SRC = dirname(fileURLToPath(import.meta.url));
  /** The rules of the Pad's own components, as written. */
  const css = [
    "Pad.svelte",
    "Mirror.svelte",
    "PageTabs.svelte",
    "Inspector.svelte",
    "IconPicker.svelte",
    "Catalogue.svelte",
    "TraceStrip.svelte",
  ]
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

describe("the columns", () => {
  /** The body of one rule of the Pad's style; jsdom applies none. */
  function rule(selector: string): string {
    const css = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "Pad.svelte"), "utf8");
    const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    return new RegExp(`\\n  ${escaped} \\{([^}]*)\\}`).exec(css)?.[1] ?? "";
  }

  it("each scroll on their own, the drawing, the inspector and the catalogue (#166)", async () => {
    await mounted({ columns: layout(1200) });
    const scrolling = [
      screen.getByRole("grid"),
      screen.getByRole("heading", { name: "Inspector" }),
      screen.getByRole("region", { name: "Actions" }),
    ].map((element) => element.closest(".scroll"));
    expect(scrolling).not.toContain(null);
    expect(new Set(scrolling).size).toBe(3);
    // DESIGN.md "Columns": a reserved gutter and the theme's thin bar.
    expect(rule(".scroll")).toMatch(/min-height: 0;/);
    expect(rule(".scroll")).toMatch(/overflow: auto;/);
    expect(rule(".scroll")).toMatch(/scrollbar-gutter: stable;/);
    expect(rule(".scroll")).toMatch(/scrollbar-width: thin;/);
  });

  it("fill the screen's height under the notices instead of growing it (#166)", () => {
    expect(rule(".pad")).toMatch(/height: 100%;/);
    expect(rule(".workspace")).toMatch(/flex: 1 1 0;/);
    expect(rule(".workspace")).toMatch(/min-height: 0;/);
    expect(rule(".workspace")).toMatch(/align-items: stretch;/);
    expect(rule(".workspace")).not.toMatch(/flex-wrap: wrap/);
  });
});
