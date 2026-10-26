// Home against a fake daemon fed with the shared socket fixtures
// (tests/protocol-fixtures/socket/): the mirror, the status strip, the
// event list, the virtual pad's dry-run presses and the unreachable state.
// The key editor beside the mirror against a fake editor; KeyEditor.test.ts
// tests the editor itself.
import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { tick } from "svelte";
import { describe, expect, it, onTestFinished, vi } from "vitest";

import Home from "./Home.svelte";
import type { Daemon, Tile, Update } from "./lib/daemon";
import type { Editor, KeyFields, Target } from "./lib/editor";
import { eventsOf, nth, responses } from "./lib/fixtures";
import { HOME_TRACE } from "./lib/home";
import { t } from "./lib/i18n";
import type { PressKind, Size, Status, TraceRecord } from "./lib/protocol";

const STATUS = responses()[0] as Status;
const TILE = { width: 118, height: 118 };
const DRY_RUN = (responses()[3] as { entry: TraceRecord }).entry;
const PROFILE = "/home/u/.config/keyvo/profiles/terminal.toml";

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
    outline: vi.fn((profile: string) =>
      Promise.resolve(profile === "terminal" ? { pages: 3, accent: "#2ecc71" } : { pages: 2 }),
    ),
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

/** The grid cell of key `n`, which the device drawing places. */
function cell(n: number): HTMLElement {
  return key(n).closest('[role="gridcell"]') as HTMLElement;
}

/** The percent value of `property` in the inline style of `element`. */
function percent(element: HTMLElement, property: string): number {
  const style = element.getAttribute("style") ?? "";
  const value = new RegExp(`(?:^|;)\\s*${property}:\\s*([\\d.]+)%`).exec(style)?.[1];
  return value === undefined ? Number.NaN : Number(value);
}

/** A key editor on blank keys whose writes succeed; KeyEditor.test.ts
 * tests the editor itself. */
function editor(overrides: Partial<Editor> = {}) {
  return {
    key: vi.fn((_target: Target) => Promise.resolve({ path: PROFILE, fields: {} })),
    editKey: vi.fn((_target: Target, fields: KeyFields) =>
      Promise.resolve({ path: PROFILE, before: {}, fields }),
    ),
    ...overrides,
  } satisfies Editor;
}

async function mounted(daemon = fake(), keys = editor()) {
  render(Home, { props: { daemon: daemon.daemon, editor: keys } });
  await waitFor(() => expect(key(5)).toHaveAccessibleName("Key 5: Test, exec cargo test"));
  return daemon;
}

describe("Home mirror", () => {
  it("shows the nine tiles of the active page in a labelled 3x3 grid", async () => {
    const daemon = await mounted();
    const grid = screen.getByRole("grid", { name: "Keypad mirror: profile terminal, page 1" });
    const rows = within(grid).getAllByRole("row");
    expect(rows).toHaveLength(3);
    for (const row of rows) {
      expect(within(row).getAllByRole("gridcell")).toHaveLength(3);
    }
    // Key count and tile size are the geometry of the status reply.
    expect(daemon.daemon.tiles).toHaveBeenCalledWith("terminal", 1, 9, TILE);
    // The tiles sit in the cutouts of the Keypad drawing, in percent of it.
    expect(within(grid).getByRole("img", { name: t("device.keypad") })).toBeInTheDocument();
    expect(cell(1)).toHaveStyle({ position: "absolute" });
    expect(percent(cell(1), "left")).toBeCloseTo((57 / 512) * 100, 1);
    expect(percent(cell(9), "top")).toBeCloseTo((334 / 610) * 100, 1);
    expect(percent(cell(9), "width")).toBeCloseTo((117 / 512) * 100, 1);
    // The image is decorative (alt=""): the key's name says what it shows.
    expect(key(1).querySelector("img")).toHaveAttribute(
      "src",
      "data:image/png;base64,terminal-1-1",
    );
    expect(key(1).querySelector("img")).toHaveAttribute("alt", "");
    expect(key(1)).toHaveAccessibleName("Key 1: unbound");
    expect(key(4)).toHaveAccessibleName("Key 4: Palette, chord ctrl+p, app layer");
  });

  it("names the layer of each key and marks the global keys under an app profile", async () => {
    const daemon = await mounted();
    expect(key(3)).toHaveAccessibleName("Key 3: Type, chord shift+k, global layer");
    expect(within(key(3)).getByText("global")).toBeInTheDocument();
    expect(within(key(4)).queryByText("global")).not.toBeInTheDocument();
    // On the global profile every key is global: no mark.
    await daemon.send({ kind: "event", event: nth(eventsOf("page"), 0) });
    await waitFor(() =>
      expect(key(4)).toHaveAccessibleName("Key 4: Palette, chord ctrl+p, global layer"),
    );
    expect(within(key(3)).queryByText("global")).not.toBeInTheDocument();
  });

  it("follows the view events and fetches the new page", async () => {
    const daemon = await mounted();
    await daemon.send({ kind: "event", event: nth(eventsOf("page"), 0) });
    await waitFor(() =>
      expect(daemon.daemon.tiles).toHaveBeenLastCalledWith("global", 2, 9, TILE),
    );
    expect(
      screen.getByRole("grid", { name: "Keypad mirror: profile global, page 2" }),
    ).toBeInTheDocument();
  });

  it("offers the profile's pages and shows its accent bar", async () => {
    const daemon = await mounted();
    const pages = screen.getByRole("combobox", { name: "Page shown" });
    expect(within(pages).getAllByRole("option").map((option) => option.textContent)).toEqual([
      "Page 1",
      "Page 2",
      "Page 3",
    ]);
    expect(pages).toHaveValue("1");
    expect(daemon.daemon.outline).toHaveBeenCalledWith("terminal");
    expect(screen.getByRole("img", { name: "Accent of profile terminal" })).toHaveStyle({
      backgroundColor: "#2ecc71",
    });
    // The global profile has no accent: no bar.
    await daemon.send({ kind: "event", event: nth(eventsOf("page"), 0) });
    await waitFor(() => expect(within(pages).getAllByRole("option")).toHaveLength(2));
    expect(screen.queryByRole("img", { name: /^Accent/ })).not.toBeInTheDocument();
  });

  it("shows the page the selector picks until the pad changes its view", async () => {
    const keys = editor();
    const daemon = await mounted(fake(), keys);
    await fireEvent.change(screen.getByRole("combobox", { name: "Page shown" }), {
      target: { value: "3" },
    });
    await waitFor(() => expect(daemon.daemon.tiles).toHaveBeenLastCalledWith("terminal", 3, 9, TILE));
    expect(
      screen.getByRole("grid", { name: "Keypad mirror: profile terminal, page 3" }),
    ).toBeInTheDocument();
    // The strip keeps saying what the pad shows; the editor edits the shown page.
    const strip = screen.getByRole("region", { name: "Status" });
    expect(within(strip).getByLabelText("Page")).toHaveTextContent("1");
    await waitFor(() => expect(keys.key).toHaveBeenLastCalledWith({ profile: "terminal", page: 3, key: 1 }));
    // A press simulates the pad's page, so a key of another page only selects.
    await fireEvent.click(key(2));
    expect(daemon.daemon.press).not.toHaveBeenCalled();
    expect(screen.getByRole("status", { name: "Press result" })).toHaveTextContent(
      "The pad shows page 1; presses simulate that page.",
    );
    expect(key(2)).toHaveAttribute("tabindex", "0");
    // The pad's next view brings the selector back to it.
    await daemon.send({ kind: "event", event: nth(eventsOf("profile"), 0) });
    await waitFor(() =>
      expect(screen.getByRole("combobox", { name: "Page shown" })).toHaveValue("1"),
    );
    await waitFor(() => expect(daemon.daemon.tiles).toHaveBeenLastCalledWith("terminal", 1, 9, TILE));
  });

  it("fetches the tiles again after a reload and shows a failed one", async () => {
    const daemon = await mounted();
    const good = nth(eventsOf("config"), 0);
    const bad = nth(eventsOf("config"), 1);
    await daemon.send({ kind: "event", event: bad });
    await waitFor(() => expect(daemon.daemon.tiles).toHaveBeenCalledTimes(2));
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Schema error in profiles/global.toml, line 8, column 9",
    );
    await daemon.send({ kind: "event", event: good });
    await waitFor(() => expect(daemon.daemon.tiles).toHaveBeenCalledTimes(3));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("lays out the keys and sizes the tiles the daemon's geometry reports", async () => {
    const rect = (x: number, y: number) => ({ x, y, width: 90, height: 80 });
    const geometry = {
      panel: { width: 200, height: 200 },
      tile: { width: 90, height: 80 },
      rects: [rect(0, 0), rect(100, 0), rect(0, 100), rect(100, 100)],
    };
    const status: Status = { ...STATUS, device: { connected: true, keys: 4, geometry } };
    // Four keys do not fit the Keypad's nine cutouts: the plain panel.
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    onTestFinished(() => warn.mockRestore());
    const daemon = fake({ status: vi.fn(() => Promise.resolve(status)) });
    render(Home, { props: { daemon: daemon.daemon, editor: editor() } });
    await waitFor(() => expect(key(4)).toHaveAccessibleName("Key 4: Palette, chord ctrl+p, app layer"));
    expect(daemon.daemon.tiles).toHaveBeenCalledWith("terminal", 1, 4, { width: 90, height: 80 });
    const rows = within(screen.getByRole("grid")).getAllByRole("row");
    expect(rows.map((row) => within(row).getAllByRole("gridcell").length)).toEqual([2, 2]);
    expect(screen.queryByRole("img", { name: t("device.keypad") })).not.toBeInTheDocument();
    expect(percent(cell(2), "left")).toBeCloseTo(50, 1);
    expect(percent(cell(2), "width")).toBeCloseTo(45, 1);
    expect(percent(cell(2), "height")).toBeCloseTo(40, 1);
    expect(screen.queryByRole("button", { name: /^Key 5\b/ })).not.toBeInTheDocument();
    // Arrows move inside the reported 2x2 grid; digits past the last key do nothing.
    await fireEvent.keyDown(screen.getByRole("grid"), { key: "ArrowRight" });
    await fireEvent.keyDown(screen.getByRole("grid"), { key: "ArrowRight" });
    expect(key(2)).toHaveFocus();
    await fireEvent.keyDown(screen.getByRole("grid"), { key: "ArrowDown" });
    expect(key(4)).toHaveFocus();
    await fireEvent.keyDown(screen.getByRole("grid"), { key: "7" });
    expect(key(4)).toHaveFocus();
  });

  it("shows no mirror until a Keypad has reported its geometry", async () => {
    const daemon = fake({ status: vi.fn(() => Promise.resolve(responses()[1] as Status)) });
    render(Home, { props: { daemon: daemon.daemon, editor: editor() } });
    expect(await screen.findByText(/once a Keypad has reported its geometry/)).toBeInTheDocument();
    expect(screen.queryByRole("grid")).not.toBeInTheDocument();
    expect(daemon.daemon.tiles).not.toHaveBeenCalled();
    // A connected Keypad brings it.
    await daemon.send({ kind: "event", event: nth(eventsOf("device"), 0) });
    await waitFor(() => expect(key(9)).toBeInTheDocument());
    expect(daemon.daemon.tiles).toHaveBeenCalledWith("global", 1, 9, TILE);
  });

  it("says why a page has no tiles", async () => {
    const daemon = fake({
      tiles: vi.fn(() => Promise.reject(new Error("no profile `terminal` in the configuration directory"))),
    });
    render(Home, { props: { daemon: daemon.daemon, editor: editor() } });
    expect(await screen.findByText(/no profile `terminal`/)).toBeInTheDocument();
    expect(key(1)).toHaveAccessibleName("Key 1: unbound");
  });

  it("marks the keys held down on the device", async () => {
    const daemon = await mounted();
    const press = nth(eventsOf("key"), 0);
    const release = nth(eventsOf("key"), 1);
    await daemon.send({ kind: "event", event: press });
    expect(key(5)).toHaveClass("held");
    await daemon.send({ kind: "event", event: release });
    expect(key(5)).not.toHaveClass("held");
  });

  it("subscribes with the event list's replay and stops when unmounted", async () => {
    const daemon = fake();
    const { unmount } = render(Home, { props: { daemon: daemon.daemon, editor: editor() } });
    await waitFor(() => expect(daemon.listening).toBe(true));
    expect(daemon.daemon.subscribe).toHaveBeenCalledWith(HOME_TRACE, expect.any(Function));
    unmount();
    await waitFor(() => expect(daemon.listening).toBe(false));
  });
});

describe("Home status strip", () => {
  it("shows device, profile, page, mode, top overlay and context", async () => {
    await mounted();
    const strip = screen.getByRole("region", { name: "Status" });
    expect(within(strip).getByLabelText("Device")).toHaveTextContent("Keypad connected, 9 keys");
    expect(within(strip).getByLabelText("Profile")).toHaveTextContent("terminal");
    expect(within(strip).getByLabelText("Page")).toHaveTextContent("1");
    expect(within(strip).getByLabelText("Mode")).toHaveTextContent("auto");
    // The daemon has no overlays and sends no project yet (§3.1, d7).
    expect(within(strip).getByLabelText("Overlay")).toHaveTextContent("none");
    expect(within(strip).getByLabelText("Context")).toHaveTextContent(
      "unknown project, org.kde.konsole: ~ : bash",
    );
  });

  it("links each fact to its screen; facts of screens not built yet are inert", async () => {
    const onnavigate = vi.fn();
    render(Home, { props: { daemon: fake().daemon, editor: editor(), onnavigate } });
    const strip = await screen.findByRole("region", { name: "Status" });
    await waitFor(() => expect(within(strip).getByLabelText("Profile")).toHaveTextContent("terminal"));
    const links = within(strip).getAllByRole("button");
    expect(links.map((link) => link.getAttribute("aria-label"))).toEqual([
      "Profile terminal, open Profiles",
      "Page 1, open Profiles",
      "Mode auto, open Profiles",
    ]);
    await fireEvent.click(within(strip).getByRole("button", { name: "Page 1, open Profiles" }));
    expect(onnavigate).toHaveBeenCalledWith("profiles");
    for (const fact of ["Device", "Overlay", "Context"]) {
      expect(within(within(strip).getByLabelText(fact)).queryByRole("button")).toBeNull();
    }
  });

  it("shows a schema error with file and line until a reload succeeds", async () => {
    const onnavigate = vi.fn();
    const daemon = fake();
    render(Home, { props: { daemon: daemon.daemon, editor: editor(), onnavigate } });
    await daemon.send({ kind: "event", event: nth(eventsOf("config"), 1) });
    const strip = screen.getByRole("region", { name: "Status" });
    const error = within(strip).getByRole("alert");
    expect(error).toHaveTextContent(
      "Schema error in profiles/global.toml, line 8, column 9: invalid type: integer `7`, expected a string",
    );
    await fireEvent.click(within(error).getByRole("button", { name: "Open Profiles" }));
    expect(onnavigate).toHaveBeenCalledWith("profiles");
    await daemon.send({ kind: "event", event: nth(eventsOf("config"), 0) });
    expect(within(strip).queryByRole("alert")).not.toBeInTheDocument();
  });

  it("shows a reload error that names no place as it is", async () => {
    const daemon = await mounted();
    await daemon.send({ kind: "event", event: { event: "config", reloaded: false, error: "no config dir" } });
    expect(screen.getByRole("alert")).toHaveTextContent("Configuration not loaded: no config dir");
  });

  it("follows device, mode and context events", async () => {
    const daemon = await mounted();
    await daemon.send(
      ...[...eventsOf("device"), ...eventsOf("mode"), ...eventsOf("context")].map(
        (event): Update => ({ kind: "event", event }),
      ),
    );
    const strip = screen.getByRole("region", { name: "Status" });
    expect(within(strip).getByLabelText("Device")).toHaveTextContent("No Keypad: device disconnected");
    expect(within(strip).getByLabelText("Mode")).toHaveTextContent("manual");
    // The last context is the keyvo app's, holding the window before it.
    expect(within(strip).getByLabelText("Context")).toHaveTextContent(
      "unknown project, keyvo-app focused, holding org.gnome.Ptyxis",
    );
  });

  it("names the window a held context holds", async () => {
    const app = nth(eventsOf("context"), 2);
    const daemon = await mounted();
    await daemon.send({ kind: "event", event: app });
    const strip = screen.getByRole("region", { name: "Status" });
    expect(within(strip).getByLabelText("Context")).toHaveTextContent(
      "unknown project, keyvo-app focused, holding org.kde.konsole",
    );
  });

  it("says the context is held when the window before is not known", async () => {
    const { event: _, ...held } = nth(eventsOf("context"), 2);
    const status: Status = { ...STATUS, context: held };
    await mounted(fake({ status: vi.fn(() => Promise.resolve(status)) }));
    const strip = screen.getByRole("region", { name: "Status" });
    expect(within(strip).getByLabelText("Context")).toHaveTextContent(
      "unknown project, keyvo-app focused, holding the context",
    );
  });

  it("says which device is missing and when it was last seen", async () => {
    const seen = new Date(2026, 9, 26, 9, 30, 15).getTime();
    const now = vi.spyOn(Date, "now").mockReturnValue(seen);
    try {
      const daemon = await mounted();
      expect(screen.queryByText(/is missing/)).not.toBeInTheDocument();
      now.mockReturnValue(seen + 60_000);
      await daemon.send({ kind: "event", event: nth(eventsOf("device"), 1) });
      expect(screen.getByText(/is missing/)).toHaveTextContent(
        `The Keypad is missing; last seen at ${new Date(seen).toLocaleTimeString()}.`,
      );
      // The mirror keeps the last frame.
      expect(key(5)).toHaveAccessibleName("Key 5: Test, exec cargo test");
    } finally {
      now.mockRestore();
    }
  });

  it("says when no window was reported yet", async () => {
    const daemon = fake({ status: vi.fn(() => Promise.resolve(responses()[1] as Status)) });
    render(Home, { props: { daemon: daemon.daemon, editor: editor() } });
    const strip = await screen.findByRole("region", { name: "Status" });
    await waitFor(() =>
      expect(within(strip).getByLabelText("Context")).toHaveTextContent(
        "unknown project, no window reported",
      ),
    );
    expect(within(strip).getByLabelText("Device")).toHaveTextContent(
      "No Keypad: no hidraw node for 046d:c354",
    );
    expect(
      screen.getByText("The Keypad is missing; not seen since the app started."),
    ).toBeInTheDocument();
  });
});

describe("Home event list", () => {
  it("lists the trace entries newest first, one line each", async () => {
    const daemon = await mounted();
    await daemon.send(...eventsOf("trace").map((event): Update => ({ kind: "event", event })));
    const list = screen.getByRole("list", { name: "Recent events" });
    const items = within(list).getAllByRole("listitem");
    expect(items.map((item) => item.textContent?.trim())).toEqual([
      expect.stringContaining("Key 1: unbound"),
      expect.stringContaining(
        "Key 5 long, simulated, app layer: exec cargo test, dry run, nothing executed",
      ),
      expect.stringContaining(
        "Key 9, app layer: exec gh pr view --web, failed: no such file or directory: gh",
      ),
      expect.stringContaining("Key 1, app layer: exec cargo build, exited 101 after 2318 ms"),
      expect.stringContaining("Key 4, app layer: chord ctrl+p, sent"),
    ]);
  });

  it("hands an entry to the shell's trace drawer", async () => {
    const daemon = fake();
    const ontrace = vi.fn<(entry: TraceRecord) => void>();
    render(Home, { props: { daemon: daemon.daemon, editor: editor(), ontrace } });
    const entries = eventsOf("trace");
    await daemon.send(...entries.map((event): Update => ({ kind: "event", event })));
    await fireEvent.click(
      screen.getByRole("button", {
        name: "Key 9, app layer: exec gh pr view --web, failed: no such file or directory: gh, open the trace entry",
      }),
    );
    expect(ontrace).toHaveBeenCalledWith(nth(entries, 2).entry);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("leaves the debug log switch to the trace drawer", async () => {
    await mounted();
    expect(screen.queryByRole("switch", { name: "Debug log" })).toBeNull();
  });

  it("says when nothing happened yet and how to fill the list", async () => {
    await mounted();
    expect(
      screen.getByText(
        "No presses yet. Press a key on the pad, or click one in the mirror for a dry run.",
      ),
    ).toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "Connecting to the daemon" })).not.toBeInTheDocument();
  });
});

describe("Home virtual pad", () => {
  it("a click sends a short dry-run press and shows its one-line result", async () => {
    const daemon = await mounted();
    await fireEvent.click(key(5));
    expect(daemon.daemon.press).toHaveBeenCalledWith(5, "short");
    expect(await screen.findByRole("status", { name: "Press result" })).toHaveTextContent(
      "Key 5, simulated, global layer: exec cargo test, dry run, nothing executed",
    );
  });

  it("the result line opens the dry run's trace entry in the shell's drawer", async () => {
    const ontrace = vi.fn<(entry: TraceRecord) => void>();
    render(Home, { props: { daemon: fake().daemon, editor: editor(), ontrace } });
    await waitFor(() => expect(key(5)).toHaveAccessibleName("Key 5: Test, exec cargo test"));
    await fireEvent.click(key(5));
    await fireEvent.click(
      await screen.findByRole("button", { name: "Show the trace entry" }),
    );
    expect(ontrace).toHaveBeenCalledWith({
      ...DRY_RUN,
      key: 5,
      trigger: { press: "short", simulated: true },
    });
  });

  it("a right click sends a long press", async () => {
    const daemon = await mounted();
    const shown = await fireEvent.contextMenu(key(2));
    expect(shown).toBe(false);
    expect(daemon.daemon.press).toHaveBeenCalledWith(2, "long");
    expect(await screen.findByRole("status", { name: "Press result" })).toHaveTextContent(
      "Key 2 long, simulated",
    );
  });

  it("a refused press shows why", async () => {
    const daemon = fake({ press: vi.fn(() => Promise.reject(new Error("cannot reach the keyvo daemon"))) });
    await mounted(daemon);
    await fireEvent.click(key(1));
    await waitFor(() =>
      expect(screen.getByRole("status", { name: "Press result" })).toHaveTextContent(
        "Press failed: cannot reach the keyvo daemon",
      ),
    );
  });

  it("1 to 9 select a key, Space presses it short, Shift+Space long", async () => {
    const daemon = await mounted();
    const grid = screen.getByRole("grid");
    await fireEvent.keyDown(grid, { key: "7" });
    expect(key(7)).toHaveFocus();
    expect(key(7)).toHaveAttribute("tabindex", "0");
    expect(key(1)).toHaveAttribute("tabindex", "-1");
    expect(screen.getByRole("gridcell", { selected: true })).toContainElement(key(7));

    const space = await fireEvent.keyDown(grid, { key: " " });
    expect(space).toBe(false);
    expect(daemon.daemon.press).toHaveBeenLastCalledWith(7, "short");
    await fireEvent.keyDown(grid, { key: " ", shiftKey: true });
    expect(daemon.daemon.press).toHaveBeenLastCalledWith(7, "long");

    await fireEvent.keyDown(grid, { key: "0" });
    await fireEvent.keyDown(grid, { key: "a" });
    expect(key(7)).toHaveFocus();
    expect(daemon.daemon.press).toHaveBeenCalledTimes(2);
  });

  it("the arrows move the selection inside the grid", async () => {
    await mounted();
    const grid = screen.getByRole("grid");
    await fireEvent.keyDown(grid, { key: "5" });
    await fireEvent.keyDown(grid, { key: "ArrowRight" });
    expect(key(6)).toHaveFocus();
    await fireEvent.keyDown(grid, { key: "ArrowRight" });
    expect(key(6)).toHaveFocus();
    await fireEvent.keyDown(grid, { key: "ArrowDown" });
    expect(key(9)).toHaveFocus();
    await fireEvent.keyDown(grid, { key: "ArrowLeft" });
    expect(key(8)).toHaveFocus();
    await fireEvent.keyDown(grid, { key: "ArrowUp" });
    await fireEvent.keyDown(grid, { key: "ArrowUp" });
    await fireEvent.keyDown(grid, { key: "ArrowUp" });
    expect(key(2)).toHaveFocus();
  });

  it("clicking a key selects it", async () => {
    await mounted();
    await fireEvent.click(key(3));
    expect(key(3)).toHaveAttribute("tabindex", "0");
  });
});

describe("Home without a daemon", () => {
  it("says the daemon is unreachable and retries on request", async () => {
    const status = vi
      .fn<Daemon["status"]>()
      .mockRejectedValueOnce(new Error("cannot reach the keyvo daemon at /run/user/1000/keyvo/keyvo.sock"))
      .mockResolvedValue(STATUS);
    const daemon = fake({ status });
    render(Home, { props: { daemon: daemon.daemon, editor: editor() } });

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Daemon unreachable: cannot reach the keyvo daemon at /run/user/1000/keyvo/keyvo.sock",
    );
    expect(daemon.daemon.subscribe).not.toHaveBeenCalled();

    await fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => expect(key(5)).toHaveAccessibleName("Key 5: Test, exec cargo test"));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("keeps the last frame when the subscription ends and says why", async () => {
    const daemon = await mounted();
    await daemon.send({ kind: "closed", reason: "the daemon closed the connection" });
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Daemon unreachable: the daemon closed the connection",
    );
    expect(key(5)).toHaveAccessibleName("Key 5: Test, exec cargo test");

    await fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => expect(daemon.daemon.subscribe).toHaveBeenCalledTimes(2));
    await daemon.send({ kind: "subscribed", view: STATUS.view });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("says the daemon is unreachable when subscribing fails", async () => {
    const daemon = fake({ subscribe: vi.fn(() => Promise.reject(new Error("no shell"))) });
    render(Home, { props: { daemon: daemon.daemon, editor: editor() } });
    expect(await screen.findByRole("alert")).toHaveTextContent("Daemon unreachable: no shell");
  });

  it("shows skeleton rows and no keys while connecting, as the geometry comes with the status", () => {
    const daemon = fake({ status: vi.fn(() => new Promise<Status>(() => {})) });
    render(Home, { props: { daemon: daemon.daemon, editor: editor() } });
    expect(screen.queryByRole("grid")).not.toBeInTheDocument();
    expect(screen.queryByText(/reported its geometry/)).not.toBeInTheDocument();
    const loading = screen.getByRole("list", { name: "Connecting to the daemon" });
    expect(loading).toHaveAttribute("aria-busy", "true");
    expect(within(loading).getAllByRole("listitem")).toHaveLength(3);
    expect(screen.queryByText(/No presses yet/)).not.toBeInTheDocument();
  });

  it("keeps the mirror's last frame while it connects again", async () => {
    const status = vi
      .fn<Daemon["status"]>()
      .mockResolvedValueOnce(STATUS)
      .mockReturnValue(new Promise<Status>(() => {}));
    const daemon = fake({ status });
    await mounted(daemon);
    await daemon.send({ kind: "closed", reason: "the daemon closed the connection" });
    await fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(screen.getByRole("list", { name: "Connecting to the daemon" })).toBeInTheDocument();
    expect(key(5)).toHaveAccessibleName("Key 5: Test, exec cargo test");
    expect(key(5).querySelector("img")).toHaveAttribute("src", "data:image/png;base64,terminal-1-5");
  });
});

describe("Home key editor", () => {
  it("edits the key selected on the mirror", async () => {
    const keys = editor();
    await mounted(fake(), keys);
    await waitFor(() => expect(keys.key).toHaveBeenCalledWith({ profile: "terminal", page: 1, key: 1 }));
    await fireEvent.keyDown(screen.getByRole("grid"), { key: "5" });
    await waitFor(() => expect(keys.key).toHaveBeenLastCalledWith({ profile: "terminal", page: 1, key: 5 }));
    expect(screen.getByRole("heading", { name: "Key 5" })).toBeInTheDocument();
  });

  it("fetches the tiles again after an edit", async () => {
    const daemon = fake();
    const keys = editor();
    await mounted(daemon, keys);
    const label = await screen.findByRole("textbox", { name: "Label" });
    await fireEvent.input(label, { target: { value: "Shell" } });
    await fireEvent.change(label);
    await waitFor(() =>
      expect(keys.editKey).toHaveBeenCalledWith({ profile: "terminal", page: 1, key: 1 }, { label: "Shell" }),
    );
    await waitFor(() => expect(daemon.daemon.tiles).toHaveBeenCalledTimes(2));
  });
});
