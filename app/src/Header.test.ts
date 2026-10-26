// The header (docs/design/gui-ux.md §2, §5.2): the app name, the device
// selector with every device the daemon reports plus the Simulator, and
// the overflow menu whose entries are disabled until they exist.
import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";

import Header from "./Header.svelte";
import type { DeviceChoice } from "./lib/shell";
import type { DeviceStatus } from "./lib/protocol";

const MENU = [
  "Export",
  "Import",
  "Open config folder",
  "Re-run setup step",
  "Keyboard shortcuts",
  "About",
];

function header(device?: DeviceStatus, chosen?: DeviceChoice) {
  const handlers = {
    onchoose: vi.fn<(choice: DeviceChoice) => void>(),
    onshortcuts: vi.fn<() => void>(),
  };
  render(Header, { props: { device, chosen, ...handlers } });
  return handlers;
}

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
