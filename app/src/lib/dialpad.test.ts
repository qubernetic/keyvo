// The Dialpad's controls as the panel shows them (#188): their order and
// names, and the binding of each, the shown profile's own over the global
// profile's, and the modifier key's over both (#189).
import { describe, expect, it } from "vitest";

import { binding, bound, CONTROLS, controlName, isRoller, modifierBinding } from "./dialpad";

const VOLUME = {
  cw: { type: "chord" as const, keys: "KEY_VOLUMEUP" },
  ccw: { type: "chord" as const, keys: "KEY_VOLUMEDOWN" },
  step: 5,
  label: "Volume",
};

describe("the controls", () => {
  it("are the four buttons, the roller and the dial, in the drawing's reading order", () => {
    expect(CONTROLS).toEqual(["button:1", "button:2", "roller", "dial", "button:3", "button:4"]);
    expect(CONTROLS.filter(isRoller)).toEqual(["roller", "dial"]);
  });

  it("have the names the panel and the trace show", () => {
    expect(CONTROLS.map(controlName)).toEqual([
      "Button 1",
      "Button 2",
      "Roller",
      "Dial",
      "Button 3",
      "Button 4",
    ]);
  });
});

describe("the binding", () => {
  it("is the profile's own when it binds the control", () => {
    expect(binding("vscode", { label: "Indent" }, VOLUME)).toEqual({
      fields: { label: "Indent" },
      layer: "app",
    });
    expect(binding("global", VOLUME, undefined)).toEqual({ fields: VOLUME, layer: "global" });
  });

  it("falls back to the global profile's control of the same name", () => {
    expect(binding("vscode", {}, VOLUME)).toEqual({ fields: VOLUME, layer: "global" });
    expect(binding("vscode", undefined, VOLUME)).toEqual({ fields: VOLUME, layer: "global" });
  });

  it("is empty, without a layer, when no profile binds it", () => {
    expect(binding("vscode", {}, {})).toEqual({ fields: {} });
    expect(binding("global", {}, undefined)).toEqual({ fields: {} });
  });

  it("counts any field as bound", () => {
    expect(bound({})).toBe(false);
    expect(bound({ step: 4 })).toBe(true);
    expect(bound({ label: "Tabs" })).toBe(true);
  });
});

describe("the binding of a modifier key", () => {
  const BRIGHTNESS = {
    label: "Brightness",
    dial: { cw: VOLUME.cw, step: 6 },
    roller: { cw: VOLUME.ccw, label: "Fine" },
  };

  it("is the key's rotary, labelled with the rotary's label, else the key's, and the mode", () => {
    expect(modifierBinding(BRIGHTNESS, "dial", 7, "held")).toEqual({
      fields: { cw: VOLUME.cw, step: 6, label: "Brightness (held)" },
      modifier: 7,
    });
    expect(modifierBinding(BRIGHTNESS, "roller", 7, "toggle")).toEqual({
      fields: { cw: VOLUME.ccw, label: "Fine (toggled)" },
      modifier: 7,
    });
  });

  it("names the key without a label, and leaves a rotary the key does not bind to the profile", () => {
    expect(modifierBinding({ dial: {} }, "dial", 8, "toggle")?.fields.label).toBe("Key 8 (toggled)");
    expect(modifierBinding({ dial: {} }, "roller", 8, "toggle")).toBeUndefined();
  });
});
