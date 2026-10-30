// The Dialpad panel under the mirror (#188): collapsed while no Dialpad is
// connected and no profile binds a control, with the line that says so;
// the drawing with the four buttons, the roller and the dial as selectable
// controls showing their labels, an app profile's control falling back to
// the global one; and the pulse of a control the device turned.
import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";

import Dialpad from "./Dialpad.svelte";
import { PULSE_MS } from "./lib/dialpad";
import type { ControlFields, ControlTarget, Editor, KeyFields, Target } from "./lib/editor";
import { eventsOf, nth } from "./lib/fixtures";
import { t } from "./lib/i18n";
import type { Control, DialpadStatus, ModifierMode } from "./lib/protocol";

const CONNECTED: DialpadStatus = (() => {
  const { event: _, ...status } = nth(eventsOf("dialpad"), 0);
  return status;
})();
const MISSING: DialpadStatus = { connected: false };

/** The mock's bindings, in the global profile only. */
const GLOBAL: Partial<Record<Control, ControlFields>> = {
  dial: {
    cw: { type: "chord", keys: "KEY_VOLUMEUP" },
    ccw: { type: "chord", keys: "KEY_VOLUMEDOWN" },
    step: 5,
    label: "Volume",
  },
  roller: { cw: { type: "chord", keys: "ctrl+tab" }, ccw: { type: "chord", keys: "ctrl+shift+tab" }, step: 4, label: "Tabs" },
  "button:1": { label: "Mute", action: { type: "chord", keys: "KEY_MUTE" } },
};

/** The contrib profiles' modifier keys on the global context page (#189). */
const KEYS: Record<number, KeyFields> = {
  7: {
    label: "Brightness",
    dial: { cw: { type: "exec", cmd: "brightnessctl set +5%" }, step: 6 },
    roller: { cw: { type: "chord", keys: "KEY_BRIGHTNESSUP" }, label: "Fine" },
  },
  8: { label: "Zoom", modifier: "toggle", dial: { cw: { type: "chord", keys: "ctrl+equal" } } },
};

/** An editor whose profiles bind `bindings`, by profile then control, and
 * whose global context page holds KEYS. */
function editor(bindings: Record<string, Partial<Record<Control, ControlFields>>> = { global: GLOBAL }) {
  return {
    key: vi.fn(({ profile, key }: Target) =>
      profile === "global" && KEYS[key] !== undefined
        ? Promise.resolve({ path: "/c/profiles/global.toml", fields: KEYS[key] })
        : Promise.reject(new Error(`no key ${key} in ${profile}`)),
    ),
    editKey: () => Promise.reject(new Error("the panel writes no key")),
    control: vi.fn(({ profile, control }: ControlTarget) =>
      Promise.resolve({ path: `/c/profiles/${profile}.toml`, fields: bindings[profile]?.[control] ?? {} }),
    ),
    editControl: () => Promise.reject(new Error("the panel writes no control")),
  } satisfies Editor;
}

interface Props {
  status?: DialpadStatus;
  profile?: string;
  selected?: Control;
  bindings?: Record<string, Partial<Record<Control, ControlFields>>>;
  modifier?: { key: Target; mode: ModifierMode };
}

function panel({ status = CONNECTED, profile = "vscode", selected, bindings, modifier }: Props = {}) {
  const onselect = vi.fn<(control: Control) => void>();
  const keys = editor(bindings);
  const result = render(Dialpad, {
    props: { editor: keys, status, profile, selected, modifier, onselect },
  });
  return { onselect, editor: keys, ...result };
}

function toggle(): HTMLElement {
  return screen.getByRole("button", { name: t("dialpad.title") });
}

function control(name: string): HTMLElement {
  return screen.getByRole("button", { name: new RegExp(`^${name}:`) });
}

describe("the Dialpad panel", () => {
  it("is collapsed with its line while no Dialpad is connected and nothing is bound", async () => {
    const { editor } = panel({ status: MISSING, bindings: {} });
    await waitFor(() => expect(editor.control).toHaveBeenCalledTimes(12));
    expect(toggle()).toHaveAttribute("aria-expanded", "false");
    expect(screen.getByText(t("dialpad.none"))).toBeInTheDocument();
    expect(screen.queryByRole("group", { name: t("dialpad.controls") })).toBeNull();
  });

  it("is open on a connected Dialpad: the drawing, the six controls, their labels", async () => {
    panel();
    expect(toggle()).toHaveAttribute("aria-expanded", "true");
    const group = screen.getByRole("group", { name: t("dialpad.controls") });
    expect(within(group).getAllByRole("button")).toHaveLength(6);
    expect(screen.getByRole("img", { name: t("device.dialpad") })).toBeInTheDocument();
    expect(screen.queryByText(t("dialpad.none"))).toBeNull();
    expect(screen.getByText(t("dialpad.slot", { slot: 3 }))).toBeInTheDocument();
    await waitFor(() => expect(within(control("Dial")).getByText("Volume")).toBeInTheDocument());
    expect(within(control("Roller")).getByText("Tabs")).toBeInTheDocument();
    expect(within(control("Button 1")).getByText("Mute")).toBeInTheDocument();
    expect(control("Button 2")).toHaveAccessibleName("Button 2: unbound");
  });

  it("names what each control does and the layer it falls back to", async () => {
    panel();
    await waitFor(() =>
      expect(control("Dial")).toHaveAccessibleName(
        "Dial: Volume, clockwise chord KEY_VOLUMEUP, counter-clockwise chord KEY_VOLUMEDOWN, global layer",
      ),
    );
    expect(control("Button 1")).toHaveAccessibleName("Button 1: Mute, chord KEY_MUTE, global layer");
    expect(control("Dial")).toHaveClass("fallen");
  });

  it("shows the profile's own control over the global one", async () => {
    panel({ bindings: { global: GLOBAL, vscode: { dial: { label: "Indent" } } } });
    await waitFor(() => expect(within(control("Dial")).getByText("Indent")).toBeInTheDocument());
    expect(control("Dial")).toHaveAccessibleName("Dial: Indent, unbound, app layer");
    expect(control("Dial")).not.toHaveClass("fallen");
  });

  it("opens without a Dialpad when a profile binds a control, the line kept", async () => {
    panel({ status: MISSING });
    await waitFor(() => expect(toggle()).toHaveAttribute("aria-expanded", "true"));
    expect(screen.getByText(t("dialpad.none"))).toBeInTheDocument();
    expect(screen.getByRole("group", { name: t("dialpad.controls") })).toBeInTheDocument();
  });

  it("collapses and opens on its heading", async () => {
    panel();
    await fireEvent.click(toggle());
    expect(toggle()).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("group", { name: t("dialpad.controls") })).toBeNull();
    await fireEvent.click(toggle());
    expect(toggle()).toHaveAttribute("aria-expanded", "true");
  });

  it("selects a control on a click and marks the selected one", async () => {
    const { onselect, rerender } = panel();
    await fireEvent.click(control("Roller"));
    expect(onselect).toHaveBeenCalledWith("roller");
    await rerender({ selected: "roller" });
    expect(control("Roller")).toHaveAttribute("aria-pressed", "true");
    expect(control("Dial")).toHaveAttribute("aria-pressed", "false");
  });

  it("reads the bindings again for another profile or revision", async () => {
    const { editor, rerender } = panel({ profile: "global" });
    await waitFor(() => expect(editor.control).toHaveBeenCalledTimes(6));
    await rerender({ profile: "vscode" });
    await waitFor(() => expect(editor.control).toHaveBeenCalledTimes(18));
    expect(editor.control).toHaveBeenCalledWith({ profile: "vscode", control: "button:4" });
    await rerender({ revision: 1 });
    await waitFor(() => expect(editor.control).toHaveBeenCalledTimes(30));
  });

  it("pulses the control the device turned, for a moment, and again on the next turn", async () => {
    const { rerender } = panel();
    await rerender({ pulse: { control: "dial", seq: 1 } });
    expect(control("Dial")).toHaveClass("pulse");
    expect(control("Roller")).not.toHaveClass("pulse");
    await waitFor(() => expect(control("Dial")).not.toHaveClass("pulse"), { timeout: 4 * PULSE_MS });
    await rerender({ pulse: { control: "dial", seq: 2 } });
    expect(control("Dial")).toHaveClass("pulse");
  });

  it("shows a held modifier key's dial and roller, each labelled with (held)", async () => {
    panel({ modifier: { key: { profile: "global", page: 1, key: 7 }, mode: "held" } });
    await waitFor(() => expect(within(control("Dial")).getByText("Brightness (held)")).toBeInTheDocument());
    // The roller's own label goes before the key's.
    expect(within(control("Roller")).getByText("Fine (held)")).toBeInTheDocument();
    expect(control("Dial")).toHaveAccessibleName(
      "Dial: Brightness (held), clockwise exec brightnessctl set +5%, counter-clockwise unbound, from key 7",
    );
    expect(control("Dial")).toHaveClass("modifier");
    expect(control("Dial")).not.toHaveClass("fallen");
    // A button stays the profile's.
    expect(within(control("Button 1")).getByText("Mute")).toBeInTheDocument();
  });

  it("keeps the profile's roller when a toggled key binds only the dial, and its labels once it ends", async () => {
    const { rerender } = panel({ modifier: { key: { profile: "global", page: 1, key: 8 }, mode: "toggle" } });
    await waitFor(() => expect(within(control("Dial")).getByText("Zoom (toggled)")).toBeInTheDocument());
    expect(within(control("Roller")).getByText("Tabs")).toBeInTheDocument();
    expect(control("Roller")).not.toHaveClass("modifier");
    await rerender({ modifier: undefined });
    await waitFor(() => expect(within(control("Dial")).getByText("Volume")).toBeInTheDocument());
    expect(control("Dial")).not.toHaveClass("modifier");
  });

  it("keeps the profile's labels when the modifier key cannot be read", async () => {
    const { editor } = panel({ modifier: { key: { profile: "vscode", page: 1, key: 7 }, mode: "held" } });
    await waitFor(() => expect(editor.key).toHaveBeenCalledWith({ profile: "vscode", page: 1, key: 7 }));
    await waitFor(() => expect(within(control("Dial")).getByText("Volume")).toBeInTheDocument());
  });
});
