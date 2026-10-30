// The app shell against a fake daemon and editor (docs/design/gui-ux.md
// §2; pad screen brief §5.1): header, rail, the screen the rail selects,
// the one drawer open at a time, the pin and the profile list. The Pad's own
// states are tested in Pad.test.ts and Inspector.test.ts, the Profiles
// screen's in Profiles.test.ts.
import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { tick } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";

import App from "./App.svelte";
import type { Daemon, Update } from "./lib/daemon";
import type { Editor } from "./lib/editor";
import type { PageEdit, ProfileFiles, ProfileSummary } from "./lib/profiles";
import { eventsOf, nth, responses, UNBOUND_CONTROLS } from "./lib/fixtures";
import type { Event, Status } from "./lib/protocol";

const STATUS = responses()[0] as Status;

const EDITOR: Editor = {
  key: () => Promise.resolve({ path: "/c/profiles/terminal.toml", fields: {} }),
  editKey: () => Promise.reject(new Error("not in this test")),
  ...UNBOUND_CONTROLS,
};

function summary(id: string, name: string, priority: number): ProfileSummary {
  return {
    id,
    name,
    priority,
    match: { class: id === "terminal" ? ["org.kde.konsole"] : [] },
    pages: ["context"],
    path: `/c/profiles/${id}.toml`,
    file: true,
  };
}

const PROFILES = [
  summary("global", "Global", 0),
  summary("terminal", "Terminal", 0),
  summary("vscode", "VS Code", 20),
];

/** The App's props with a daemon whose `status` is `status`, and `send`,
 * which hands events to the App's subscription. */
function props(
  status: () => Promise<Status>,
  list: () => Promise<ProfileSummary[]> = () => Promise.resolve(PROFILES),
) {
  let listener: ((update: Update) => void) | undefined;
  const files: ProfileFiles = {
    list: vi.fn(list),
    profile: () =>
      Promise.resolve({ path: "/c/profiles/terminal.toml", fields: { name: "Terminal" } }),
    editProfile: () => Promise.reject(new Error("not in this test")),
    editPage: vi.fn((_id: string, edit: PageEdit) =>
      Promise.resolve({ path: "/c/profiles/terminal.toml", edit, page: 2 }),
    ),
    restore: () => Promise.reject(new Error("not in this test")),
    open: () => Promise.reject(new Error("not in this test")),
  };
  const daemon: Daemon = {
    status,
    tiles: () => Promise.resolve([]),
    outline: () => Promise.resolve({ pages: 1 }),
    recipes: () => Promise.resolve({ recipes: [] }),
    press: () => Promise.reject(new Error("not in this test")),
    subscribe: (_replay, next) => {
      listener = next;
      return Promise.resolve(() => {
        listener = undefined;
      });
    },
  };
  return {
    props: { daemon, editor: EDITOR, files },
    files,
    async send(...events: Event[]) {
      await waitFor(() => expect(listener).toBeDefined());
      for (const event of events) listener?.({ kind: "event", event });
      await tick();
    },
  };
}

const WIDTH = window.innerWidth;

async function resize(width: number) {
  window.innerWidth = width;
  await fireEvent(window, new Event("resize"));
}

afterEach(() => {
  window.innerWidth = WIDTH;
});

function app(
  status: () => Promise<Status> = () => Promise.resolve(STATUS),
  list?: () => Promise<ProfileSummary[]>,
) {
  const fake = props(status, list);
  render(App, { props: fake.props });
  return fake;
}

function dropdown(): HTMLSelectElement {
  return screen.getByRole("combobox", { name: /^Profile/ });
}

/** The dropdown's option texts once the profile list has arrived. */
async function listed(): Promise<string[]> {
  await waitFor(() => expect(dropdown().options.length).toBeGreaterThan(0));
  return [...dropdown().options].map((option) => option.text);
}

describe("the drawers", () => {
  it("opens Preferences from the gear; Escape closes it and gives the focus back", async () => {
    app();
    const gear = screen.getByRole("button", { name: "Preferences" });
    gear.focus();
    await fireEvent.click(gear);
    const dialog = screen.getByRole("dialog", { name: "Preferences" });
    expect(within(dialog).getByRole("tab", { name: "General" })).toBeInTheDocument();
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(gear).toHaveFocus();
  });

  it("opens the keyboard shortcuts from the menu; Escape closes them and gives the focus back", async () => {
    app();
    const menu = screen.getByRole("button", { name: "More actions" });
    await fireEvent.click(menu);
    await fireEvent.click(screen.getByRole("menuitem", { name: "Keyboard shortcuts" }));
    expect(screen.getByRole("dialog", { name: "Keyboard shortcuts" })).toBeInTheDocument();
    expect(screen.queryByRole("menu")).toBeNull();
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(menu).toHaveFocus();
  });

  it("shows one drawer at a time", async () => {
    app();
    await fireEvent.click(screen.getByRole("button", { name: "Preferences" }));
    await fireEvent.click(screen.getByRole("button", { name: "More actions" }));
    await fireEvent.click(screen.getByRole("menuitem", { name: "Keyboard shortcuts" }));
    expect(screen.getAllByRole("dialog").map((dialog) => dialog.getAttribute("aria-modal"))).toEqual([
      "false",
    ]);
    expect(screen.getByRole("dialog", { name: "Keyboard shortcuts" })).toBeInTheDocument();
  });

  it("opens a trace entry of the Pad in the drawer, and Preferences in its place", async () => {
    const { send } = app();
    await send(nth(eventsOf("trace"), 0));
    await fireEvent.click(await screen.findByRole("button", { name: /open the trace entry$/ }));
    expect(screen.getByRole("dialog", { name: "Trace entry" })).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Preferences" }));
    expect(screen.getAllByRole("dialog")).toHaveLength(1);
    expect(screen.getByRole("dialog", { name: "Preferences" })).toBeInTheDocument();
  });

  it("closes a trace entry on Escape and gives the focus back to its line", async () => {
    const { send } = app();
    await send(nth(eventsOf("trace"), 3));
    const line = await screen.findByRole("button", { name: /failed: .*, open the trace entry$/ });
    line.focus();
    await fireEvent.click(line);
    const drawer = screen.getByRole("dialog", { name: "Trace entry" });
    expect(within(drawer).getByRole("region", { name: "Result" })).toHaveTextContent(
      "no such file or directory: gh",
    );
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(line).toHaveFocus();
  });

  it("opens a failed press with its on_fail run in the drawer", async () => {
    const { send } = app();
    await send(nth(eventsOf("trace"), 1));
    await send(nth(eventsOf("trace"), 2));
    await fireEvent.click(await screen.findByRole("button", { name: /; on_fail: .*, open the trace entry$/ }));
    const drawer = screen.getByRole("dialog", { name: "Trace entry" });
    expect(within(drawer).getByRole("region", { name: "Result" })).toHaveTextContent(
      "exec notify-send 'build failed' -> exited 0 after 41 ms",
    );
  });

  it("keeps the trace drawer to the Pad", async () => {
    const { send } = app();
    await send(nth(eventsOf("trace"), 0));
    await fireEvent.click(await screen.findByRole("button", { name: /open the trace entry$/ }));
    await fireEvent.keyDown(window, { key: "2", ctrlKey: true });
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});

describe("the pin", () => {
  it("lists the profiles in the header, the pad's one shown and marked", async () => {
    app();
    expect(await listed()).toEqual(["Global", "VS Code", "Terminal, on the pad"]);
    expect(dropdown().value).toBe("terminal");
  });

  it("keeps a pinned profile when the pad moves", async () => {
    const { send } = app();
    await listed();
    await fireEvent.change(dropdown(), { target: { value: "vscode" } });
    expect(dropdown().value).toBe("vscode");
    await send({ event: "profile", profile: "global", page: 1, mode: "auto", index: 1 });
    expect(dropdown().value).toBe("vscode");
    expect(await listed()).toEqual(["Global, on the pad", "VS Code", "Terminal"]);
  });

  it("presses the pin glyph after a header pick and follows the pad again when it is pressed", async () => {
    const { send } = app();
    await listed();
    const pin = screen.getByRole("button", { name: "Pin" });
    expect(pin).toHaveAttribute("aria-pressed", "false");
    await fireEvent.change(dropdown(), { target: { value: "vscode" } });
    expect(pin).toHaveAttribute("aria-pressed", "true");
    expect(dropdown()).toHaveAccessibleName("Profile, pinned");
    await send({ event: "profile", profile: "global", page: 1, mode: "auto", index: 1 });
    expect(await screen.findByText("Editing VS Code, page 1; the pad shows Global, page 1.")).toBeInTheDocument();
    await fireEvent.click(pin);
    expect(pin).toHaveAttribute("aria-pressed", "false");
    expect(dropdown()).toHaveAccessibleName("Profile, following the pad");
    expect(dropdown().value).toBe("global");
    expect(screen.queryByText(/^Editing /)).toBeNull();
  });

  it("keeps the dropdown disabled while the list cannot be read", async () => {
    const { files } = app(undefined, () => Promise.reject(new Error("no config dir")));
    await waitFor(() => expect(files.list).toHaveBeenCalled());
    await tick();
    expect(dropdown()).toBeDisabled();
  });

  it("keeps the newest list when two reads answer out of order", async () => {
    const answers: ((list: ProfileSummary[]) => void)[] = [];
    const { files, send } = app(
      undefined,
      () => new Promise<ProfileSummary[]>((resolve) => answers.push(resolve)),
    );
    await waitFor(() => expect(files.list).toHaveBeenCalledTimes(1));
    await send({ event: "config", reloaded: true });
    await waitFor(() => expect(files.list).toHaveBeenCalledTimes(2));
    answers[1]?.(PROFILES);
    await waitFor(() => expect(dropdown().options.length).toBe(3));
    answers[0]?.([summary("global", "Global", 0)]);
    await tick();
    await Promise.resolve();
    expect(dropdown().options.length).toBe(3);
  });

  it("reads the profile list again after a reload and unpins a profile that is gone", async () => {
    let answer = PROFILES;
    const { files, send } = app(undefined, () => Promise.resolve(answer));
    await listed();
    expect(files.list).toHaveBeenCalledTimes(1);
    await fireEvent.change(dropdown(), { target: { value: "vscode" } });
    answer = PROFILES.filter((profile) => profile.id !== "vscode");
    await send({ event: "config", reloaded: true });
    await waitFor(() => expect(files.list).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(dropdown().value).toBe("terminal"));
    expect(await listed()).toEqual(["Global", "Terminal, on the pad"]);
  });

  it("adds a page with the tabs' +, pins it and reads the list again", async () => {
    const { files } = app();
    await listed();
    const add = await screen.findByRole("button", { name: "Add a page" });
    await fireEvent.click(add);
    await waitFor(() => expect(files.editPage).toHaveBeenCalledWith("terminal", { op: "add" }));
    await waitFor(() => expect(files.list).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(screen.getByRole("button", { name: "Pin" })).toHaveAttribute("aria-pressed", "true"));
    expect(screen.getByText("Editing Terminal, page 2; the pad shows Terminal, page 1.")).toBeInTheDocument();
  });
});

describe("App", () => {
  it("names the app in its heading", () => {
    app(() => new Promise(() => {}));
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("keyvo");
    expect(screen.getByRole("list", { name: "Connecting to the daemon" })).toBeInTheDocument();
  });

  it("opens on the Pad with the view the daemon reports", async () => {
    app();
    expect(screen.getByRole("button", { name: "Pad" })).toHaveAttribute("aria-current", "page");
    expect(
      await screen.findByRole("grid", { name: "Keypad mirror: profile terminal, page 1" }),
    ).toBeInTheDocument();
    // Nothing is selected yet: the inspector asks for a key.
    expect(await screen.findByRole("heading", { name: "Inspector" })).toBeInTheDocument();
  });

  it("pins the inspector when a key is selected and keeps it when the pad moves", async () => {
    const { send } = app();
    await listed();
    await fireEvent.click(await screen.findByRole("button", { name: /^Key 4\b/ }));
    expect(await screen.findByRole("heading", { name: /^Key 4: / })).toBeInTheDocument();
    await send({ event: "profile", profile: "global", page: 1, mode: "auto", index: 1 });
    expect(dropdown().value).toBe("terminal");
    expect(await screen.findByText("Editing Terminal, key 4; the pad shows Global, page 1.")).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Follow" }));
    expect(dropdown().value).toBe("global");
    expect(screen.queryByText(/the pad shows/)).toBeNull();
  });

  it("says when the daemon cannot be reached", async () => {
    app(() => Promise.reject(new Error("cannot reach the keyvo daemon")));
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(
        "Daemon unreachable: cannot reach the keyvo daemon",
      ),
    );
    expect(screen.getByRole("combobox", { name: "Device" })).toHaveValue("simulator");
  });

  it("offers the Keypad the daemon reports in the header", async () => {
    app();
    const selector = screen.getByRole("combobox", { name: "Device" });
    await waitFor(() => expect(selector).toHaveValue("keypad"));
    await fireEvent.change(selector, { target: { value: "simulator" } });
    expect(selector).toHaveValue("simulator");
  });

  it("switches to Profiles with Ctrl+2 and back to the Pad with Ctrl+1", async () => {
    app();
    await screen.findByRole("grid");
    await fireEvent.keyDown(window, { key: "2", ctrlKey: true });
    expect(screen.getByRole("button", { name: "Profiles" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(screen.queryByRole("grid", { name: /^Keypad mirror/ })).toBeNull();
    expect(
      await screen.findByRole("button", { name: "Terminal (terminal), priority 0, on the pad" }),
    ).toHaveAttribute("aria-current", "true");
    await fireEvent.keyDown(window, { key: "1", ctrlKey: true });
    expect(screen.getByRole("grid", { name: /^Keypad mirror/ })).toBeInTheDocument();
  });

  it("keeps the Pad's undo to the Pad", async () => {
    app();
    await screen.findByRole("grid");
    await fireEvent.click(screen.getByRole("button", { name: "Profiles" }));
    expect(screen.queryByRole("button", { name: "Undo" })).toBeNull();
  });

  it("lays Profiles out by the window width", async () => {
    const screenOf = () => document.querySelector(".profiles-screen");
    await resize(1200);
    app();
    await fireEvent.click(screen.getByRole("button", { name: "Profiles" }));
    expect(screenOf()).not.toHaveClass("stacked");
    expect(screen.getByRole("button", { name: "Pad" })).toHaveTextContent("Pad");
    await resize(960);
    expect(screenOf()).toHaveClass("stacked");
    expect(screen.getByRole("button", { name: "Pad" })).toHaveTextContent("");
  });

  it("keeps the header and the Profiles editor on one pin while the pad moves", async () => {
    const { send } = app();
    await listed();
    await fireEvent.keyDown(window, { key: "2", ctrlKey: true });
    await fireEvent.change(dropdown(), { target: { value: "vscode" } });
    await send({ event: "profile", profile: "global", page: 1, mode: "auto", index: 1 });
    expect(await screen.findByRole("heading", { level: 2, name: "VS Code" })).toBeInTheDocument();
    expect(dropdown()).toHaveAccessibleName("Profile, pinned");
    // A pick in the list pins too, and the header names it.
    await fireEvent.click(screen.getByRole("button", { name: "Terminal (terminal), priority 0" }));
    expect(dropdown().value).toBe("terminal");
    // Follow in the header: both follow the pad again.
    await fireEvent.click(screen.getByRole("button", { name: "Pin" }));
    expect(dropdown()).toHaveAccessibleName("Profile, following the pad");
    expect(await screen.findByRole("heading", { level: 2, name: "Global" })).toBeInTheDocument();
  });

  it("opens the Pad pinned to a profile from Profiles", async () => {
    app();
    await listed();
    await fireEvent.keyDown(window, { key: "2", ctrlKey: true });
    await fireEvent.click(
      await screen.findByRole("button", { name: "VS Code (vscode), priority 20" }),
    );
    await fireEvent.click(await screen.findByRole("button", { name: "Edit keys on the Pad" }));
    expect(screen.getByRole("button", { name: "Pad" })).toHaveAttribute("aria-current", "page");
    expect(dropdown().value).toBe("vscode");
    expect(await screen.findByText("Editing VS Code, page 1; the pad shows Terminal, page 1.")).toBeInTheDocument();
  });

  it("gives every control an aria-label", async () => {
    app();
    await fireEvent.click(await screen.findByRole("button", { name: /^Key 1\b/ }));
    await screen.findByRole("heading", { name: /^Key 1: / });
    await screen.findByRole("tablist", { name: "Pages" });
    await fireEvent.click(screen.getByRole("button", { name: "More actions" }));
    const controls = document.querySelectorAll("button, select, input, textarea, [role=menuitem]");
    expect(controls.length).toBeGreaterThan(10);
    for (const control of controls) {
      if (control.closest("[hidden]") !== null) continue;
      expect(control.getAttribute("aria-label"), control.outerHTML).toBeTruthy();
    }
  });
});

describe("the scroll owner", () => {
  /** The body of one rule of the shell's style; jsdom applies none. */
  function rule(selector: string): string {
    const css = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "App.svelte"), "utf8");
    const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    return new RegExp(`\\n  ${escaped} \\{([^}]*)\\}`).exec(css)?.[1] ?? "";
  }

  it("leaves the scrolling to the screen's columns: main never scrolls (#166)", () => {
    // Two scroll bars side by side at 640 px (main and the editor) and the
    // Pad's drawing scrolling under its catalogue were the finding.
    expect(rule("main")).toMatch(/overflow: hidden;/);
    expect(rule("main")).not.toMatch(/overflow: auto/);
    expect(rule("main")).not.toMatch(/scrollbar-gutter/);
    // Each screen is exactly as tall as main, so its columns own the height.
    expect(rule(".screen")).toMatch(/height: 100%;/);
    expect(rule(".screen")).toMatch(/box-sizing: border-box;/);
  });
});
