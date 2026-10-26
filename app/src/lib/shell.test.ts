// The shell's rules (docs/design/gui-ux.md §2, §5.2, §6): the rail's
// screens and shortcuts, the column collapse order, the device selector.
import { describe, expect, it } from "vitest";

import { t } from "./i18n";
import {
  chosenDevice,
  deviceChoices,
  layout,
  MENU,
  PREFERENCES,
  SCREENS,
  shortcut,
} from "./shell";

function key(key: string, modifiers: Partial<KeyboardEventInit> = {}): KeyboardEvent {
  return new KeyboardEvent("keydown", { key, ctrlKey: true, ...modifiers });
}

describe("SCREENS", () => {
  it("lists the rail's three screens in the design's order", () => {
    expect(SCREENS.map((screen) => t(screen.label))).toEqual(["Pad", "Profiles", "Behaviors"]);
  });

  it("keeps the screen id home for the Pad", () => {
    expect(SCREENS.map((screen) => screen.id)).toEqual(["home", "profiles", "behaviors"]);
  });

  it("builds Pad and Profiles; Behaviors names its roadmap milestone", () => {
    expect(SCREENS.filter((screen) => screen.soon === undefined).map((s) => s.id)).toEqual([
      "home",
      "profiles",
    ]);
    const soon = SCREENS.flatMap((screen) => (screen.soon === undefined ? [] : [t(screen.soon)]));
    expect(soon).toEqual(["Behaviors are not built yet; they arrive with M4a-5 on the roadmap."]);
  });
});

describe("shortcut", () => {
  it("maps Ctrl+1 to Ctrl+3 to the screens in order", () => {
    expect(["1", "2", "3"].map((digit) => shortcut(key(digit)))).toEqual([0, 1, 2]);
  });

  it("ignores other digits, plain digits and other modifiers", () => {
    expect(shortcut(key("4"))).toBeUndefined();
    expect(shortcut(key("8"))).toBeUndefined();
    expect(shortcut(key("0"))).toBeUndefined();
    expect(shortcut(key("1", { ctrlKey: false }))).toBeUndefined();
    expect(shortcut(key("1", { shiftKey: true }))).toBeUndefined();
    expect(shortcut(key("1", { altKey: true }))).toBeUndefined();
  });
});

describe("layout", () => {
  it("shows every column with labels on the rail at the default 1200 px", () => {
    expect(layout(1200)).toEqual({ railLabels: true, list: true, right: true });
  });

  it("collapses the right column first", () => {
    expect(layout(1199)).toEqual({ railLabels: true, list: true, right: false });
  });

  it("drops the rail labels below 1100 px", () => {
    expect(layout(1100)).toEqual({ railLabels: true, list: true, right: false });
    expect(layout(1099)).toEqual({ railLabels: false, list: true, right: false });
  });

  it("collapses the list column next", () => {
    expect(layout(1000)).toEqual({ railLabels: false, list: true, right: false });
    expect(layout(999)).toEqual({ railLabels: false, list: false, right: false });
    expect(layout(960)).toEqual({ railLabels: false, list: false, right: false });
  });

  it("never shows the right column without the list column", () => {
    for (let width = 900; width <= 1400; width += 10) {
      const columns = layout(width);
      expect(!columns.right || columns.list, String(width)).toBe(true);
    }
  });
});

describe("MENU", () => {
  it("enables Keyboard shortcuts as its one entry that exists", () => {
    expect(MENU.filter((entry) => entry.enabled).map((entry) => t(entry.label))).toEqual([
      "Keyboard shortcuts",
    ]);
    expect(MENU.map((entry) => t(entry.label))).toContain("About");
  });
});

describe("PREFERENCES", () => {
  it("lists the tabs General, Devices, Integrations, Projects", () => {
    expect(PREFERENCES.map((tab) => t(tab.label))).toEqual([
      "General",
      "Devices",
      "Integrations",
      "Projects",
    ]);
  });

  it("gives every tab the sentence of its roadmap item", () => {
    expect(PREFERENCES.map((tab) => t(tab.soon))).toEqual([
      "Settings are not built yet; they arrive with M4a-8 on the roadmap.",
      "Devices are not built yet; they arrive with M4a-8 on the roadmap.",
      "Integrations are not built yet; they arrive with M4a-8 on the roadmap.",
      "Projects are not built yet; they arrive with M4a-6 on the roadmap.",
    ]);
  });
});

describe("the device selector", () => {
  it("offers the connected Keypad and the Simulator", () => {
    expect(deviceChoices({ connected: true, keys: 9 })).toEqual(["keypad", "simulator"]);
  });

  it("offers only the Simulator without a Keypad or a daemon", () => {
    expect(deviceChoices({ connected: false })).toEqual(["simulator"]);
    expect(deviceChoices(undefined)).toEqual(["simulator"]);
  });

  it("selects the Keypad unless the user chose the Simulator", () => {
    expect(chosenDevice({ connected: true }, undefined)).toBe("keypad");
    expect(chosenDevice({ connected: true }, "simulator")).toBe("simulator");
  });

  it("shows the Simulator while the Keypad is missing, whatever was chosen", () => {
    expect(chosenDevice({ connected: false }, "keypad")).toBe("simulator");
    expect(chosenDevice(undefined, undefined)).toBe("simulator");
  });
});
