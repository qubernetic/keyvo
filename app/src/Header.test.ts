// The header (docs/design/gui-ux.md §2, §5.2; pad screen brief §5.1): the
// app name, the device selector with every device the daemon reports plus
// the Simulator, the profile dropdown that pins the inspector, the sandbox
// switch not built yet, the gear that opens Preferences, and the overflow
// menu whose entries are disabled until they exist.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";

import Header from "./Header.svelte";
import type { Pinned } from "./lib/pin";
import type { ProfileSummary } from "./lib/profiles";
import type { DeviceChoice } from "./lib/shell";
import type { DeviceStatus, View } from "./lib/protocol";

const MENU = [
  "Export",
  "Import",
  "Open config folder",
  "Re-run setup step",
  "Keyboard shortcuts",
  "About",
];

function summary(id: string, name: string, priority: number): ProfileSummary {
  return { id, name, priority, match: { class: [] }, pages: [null], path: `/c/${id}.toml`, file: true };
}

/** In id order, as `files.list()` answers. */
const PROFILES = [
  summary("global", "Global", 0),
  summary("terminal", "Terminal", 10),
  summary("vscode", "VS Code", 20),
];

const PAD: View = { profile: "terminal", page: 1, mode: "auto", index: 1 };

function header(
  device?: DeviceStatus,
  chosen?: DeviceChoice,
  {
    profiles = PROFILES,
    pad = PAD,
    pinned,
  }: { profiles?: ProfileSummary[]; pad?: View; pinned?: Pinned } = {},
) {
  const handlers = {
    onchoose: vi.fn<(choice: DeviceChoice) => void>(),
    onshortcuts: vi.fn<() => void>(),
    onpreferences: vi.fn<() => void>(),
    onpin: vi.fn<(pinned: Pinned) => void>(),
  };
  render(Header, { props: { device, chosen, profiles, pad, pinned, ...handlers } });
  return handlers;
}

function dropdown(): HTMLSelectElement {
  return screen.getByRole("combobox", { name: /^Profile/ });
}

function pin(): HTMLButtonElement {
  return screen.getByRole("button", { name: "Pin" });
}

/** The body of one rule of the component's style; jsdom applies none. */
function rule(selector: string): string {
  const css = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "Header.svelte"), "utf8");
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`\\n  ${escaped} \\{([^}]*)\\}`).exec(css)?.[1] ?? "";
}

describe("the profile dropdown", () => {
  it("lists the profiles by priority, the global one first, the one on the pad marked", () => {
    header();
    expect([...dropdown().options].map((option) => option.text)).toEqual([
      "Global",
      "VS Code",
      "Terminal, on the pad",
    ]);
  });

  it("shows the pad's profile while following", () => {
    header();
    expect(dropdown().value).toBe("terminal");
  });

  it("shows the pinned profile when it differs from the pad's", () => {
    header(undefined, undefined, { pinned: { profile: "vscode", page: 1 } });
    expect(dropdown().value).toBe("vscode");
  });

  it("pins the profile the user chooses", async () => {
    const { onpin } = header();
    await fireEvent.change(dropdown(), { target: { value: "vscode" } });
    expect(onpin).toHaveBeenCalledWith({ profile: "vscode", page: 1 });
  });

  it("is disabled while the profiles load", () => {
    header(undefined, undefined, { profiles: [] });
    expect(dropdown()).toBeDisabled();
  });

  it("says in its name that it follows the pad while nothing is pinned", () => {
    header();
    expect(dropdown()).toHaveAccessibleName("Profile, following the pad");
  });

  it("says in its name that it is pinned, on the pad's profile too", () => {
    header(undefined, undefined, { pinned: { profile: "terminal", page: 2 } });
    expect(dropdown()).toHaveAccessibleName("Profile, pinned");
  });

  it("is capped at 24ch, cutting a long name, which its title carries whole", () => {
    const long = "A profile with a name far longer than the header has room for";
    header(undefined, undefined, { profiles: [...PROFILES, summary("long", long, 30)], pinned: { profile: "long", page: 1 } });
    expect(dropdown()).toHaveAttribute("title", long);
    expect(dropdown()).toHaveClass("profile");
    expect(rule(".profile")).toMatch(/max-width: 24ch;/);
    expect(rule(".profile")).toMatch(/text-overflow: ellipsis;/);
    expect([...dropdown().options].map((option) => option.text)).toContain(long);
  });
});

describe("the pin", () => {
  it("is a toggle beside the dropdown, not pressed while following", () => {
    header();
    expect(pin()).toHaveAttribute("aria-pressed", "false");
    expect(dropdown().nextElementSibling).toBe(pin());
    expect(pin()).toHaveAccessibleDescription("Following the pad. Press to pin Terminal, page 1.");
  });

  it("is pressed while the inspector is pinned", () => {
    header(undefined, undefined, { pinned: { profile: "vscode", page: 1 } });
    expect(pin()).toHaveAttribute("aria-pressed", "true");
    expect(pin()).toHaveAccessibleDescription("Pinned to VS Code, page 1. Press to follow the pad.");
  });

  it("follows the pad again when pressed while pinned", async () => {
    const { onpin } = header(undefined, undefined, { pinned: { profile: "vscode", page: 1 } });
    await fireEvent.click(pin());
    expect(onpin).toHaveBeenCalledWith(undefined);
  });

  it("pins the pad's profile and page when pressed while following", async () => {
    const { onpin } = header(undefined, undefined, { pad: { ...PAD, page: 2 } });
    await fireEvent.click(pin());
    expect(onpin).toHaveBeenCalledWith({ profile: "terminal", page: 2 });
  });

  it("is disabled while nothing is pinned and the pad reports no view", () => {
    render(Header, { props: { onchoose: vi.fn(), profiles: PROFILES } });
    expect(pin()).toBeDisabled();
  });
});

describe("the sandbox switch", () => {
  it("is a switch not built yet, off, with its roadmap sentence", async () => {
    header();
    const sandbox = screen.getByRole("switch", { name: "Sandbox" });
    expect(sandbox).toHaveAttribute("aria-checked", "false");
    expect(sandbox).toHaveAttribute("aria-disabled", "true");
    sandbox.focus();
    await Promise.resolve();
    expect(sandbox).toHaveAccessibleDescription(/M4a-4/);
    expect(screen.queryByRole("button", { name: "Apply" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Discard" })).toBeNull();
  });
});

describe("the gear", () => {
  it("opens Preferences", async () => {
    const { onpreferences } = header();
    const gear = screen.getByRole("button", { name: "Preferences" });
    expect(gear).toHaveAttribute("aria-haspopup", "dialog");
    await fireEvent.click(gear);
    expect(onpreferences).toHaveBeenCalledTimes(1);
  });
});

function selector(): HTMLSelectElement {
  return screen.getByRole("combobox", { name: "Device" });
}

function options(): string[] {
  return [...selector().options].map((option) => option.text);
}

describe("Header", () => {
  it("names the app", () => {
    header();
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("keyvo");
  });

  it("offers the connected Keypad and the Simulator, the Keypad selected", () => {
    header({ connected: true, keys: 9 });
    expect(options()).toEqual(["Keypad", "Simulator"]);
    expect(selector().value).toBe("keypad");
  });

  it("shows the Simulator while no Keypad is connected", () => {
    header({ connected: false, error: "not found" }, "keypad");
    expect(options()).toEqual(["Simulator"]);
    expect(selector().value).toBe("simulator");
  });

  it("hands the user's choice on", async () => {
    const { onchoose } = header({ connected: true });
    await fireEvent.change(selector(), { target: { value: "simulator" } });
    expect(onchoose).toHaveBeenCalledWith("simulator");
  });

  it("opens the overflow menu with every entry but the shortcuts disabled", async () => {
    header();
    const button = screen.getByRole("button", { name: "More actions" });
    expect(button).toHaveAttribute("aria-haspopup", "menu");
    expect(button).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("menu")).toBeNull();

    await fireEvent.click(button);
    expect(button).toHaveAttribute("aria-expanded", "true");
    const items = screen.getAllByRole("menuitem");
    expect(items.map((item) => item.getAttribute("aria-label"))).toEqual(MENU);
    const soon = items.filter((item) => item.getAttribute("aria-label") !== "Keyboard shortcuts");
    for (const item of soon) {
      expect(item).toHaveAttribute("aria-disabled", "true");
      expect(item).toHaveAccessibleDescription("Coming soon");
    }
    const shortcuts = screen.getByRole("menuitem", { name: "Keyboard shortcuts" });
    expect(shortcuts).not.toHaveAttribute("aria-disabled");
    expect(shortcuts).not.toHaveAccessibleDescription();
    expect(items[0]).toHaveFocus();
  });

  it("opens the keyboard shortcuts from the menu and closes the menu", async () => {
    const { onshortcuts } = header();
    await fireEvent.click(screen.getByRole("button", { name: "More actions" }));
    await fireEvent.click(screen.getByRole("menuitem", { name: "Keyboard shortcuts" }));
    expect(onshortcuts).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("moves through the menu with the arrows and closes it on Escape", async () => {
    header();
    const button = screen.getByRole("button", { name: "More actions" });
    await fireEvent.click(button);
    const menu = screen.getByRole("menu");
    await fireEvent.keyDown(menu, { key: "ArrowDown" });
    expect(screen.getByRole("menuitem", { name: "Import" })).toHaveFocus();
    await fireEvent.keyDown(menu, { key: "ArrowUp" });
    await fireEvent.keyDown(menu, { key: "ArrowUp" });
    expect(screen.getByRole("menuitem", { name: "About" })).toHaveFocus();
    await fireEvent.keyDown(menu, { key: "Escape" });
    expect(screen.queryByRole("menu")).toBeNull();
    expect(button).toHaveFocus();
  });

  it("does nothing on a disabled entry", async () => {
    header();
    await fireEvent.click(screen.getByRole("button", { name: "More actions" }));
    await fireEvent.click(screen.getByRole("menuitem", { name: "About" }));
    expect(screen.getByRole("menu")).toBeInTheDocument();
  });

  it("closes the menu on a click elsewhere", async () => {
    header();
    await fireEvent.click(screen.getByRole("button", { name: "More actions" }));
    await fireEvent.click(document.body);
    expect(screen.queryByRole("menu")).toBeNull();
  });
});
