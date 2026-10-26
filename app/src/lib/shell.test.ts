// The shell's rules (docs/design/gui-ux.md §2, §5.2, §6): the rail's
// screens and shortcuts, the column collapse order, the device selector.
import { describe, expect, it } from "vitest";

import { t } from "./i18n";
import { chosenDevice, deviceChoices, layout, SCREENS, shortcut } from "./shell";

function key(key: string, modifiers: Partial<KeyboardEventInit> = {}): KeyboardEvent {
  return new KeyboardEvent("keydown", { key, ctrlKey: true, ...modifiers });
}

describe("SCREENS", () => {
  it("lists the rail's screens in the design's order", () => {
    expect(SCREENS.map((screen) => t(screen.label))).toEqual([
      "Home",
      "Profiles",
      "Behaviors",
      "Projects",
      "Integrations",
      "Devices",
      "Settings",
    ]);
  });

  it("builds Home and Profiles; the others name their roadmap milestone", () => {
    expect(SCREENS.filter((screen) => screen.soon === undefined).map((s) => s.id)).toEqual([
      "home",
      "profiles",
    ]);
    const soon = SCREENS.flatMap((screen) => (screen.soon === undefined ? [] : [t(screen.soon)]));
    expect(soon).toHaveLength(5);
    for (const sentence of soon) expect(sentence).toMatch(/M4a-\d/);
  });
});

describe("shortcut", () => {
  it("maps Ctrl+1 to Ctrl+7 to the screens in order", () => {
    expect(["1", "2", "3", "4", "5", "6", "7"].map((digit) => shortcut(key(digit)))).toEqual([
      0, 1, 2, 3, 4, 5, 6,
    ]);
  });

  it("ignores other digits, plain digits and other modifiers", () => {
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
    expect(layout(1099)).toEqual({ railLabels: false, list: true, right: false });
  });

  it("collapses the list column next", () => {
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
