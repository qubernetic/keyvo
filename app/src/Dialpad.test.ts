// The Dialpad panel under the mirror (#188): collapsed while no Dialpad is
// connected and no profile binds a control, with the line that says so;
// the drawing with the four buttons, the roller and the dial as selectable
// controls showing their labels, an app profile's control falling back to
// the global one; and the pulse of a control the device turned.
import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";

import Dialpad from "./Dialpad.svelte";
import { PULSE_MS } from "./lib/dialpad";
import type { ControlFields, ControlTarget, Editor } from "./lib/editor";
import { eventsOf, nth } from "./lib/fixtures";
import { t } from "./lib/i18n";
import type { Control, DialpadStatus } from "./lib/protocol";

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

/** An editor whose profiles bind `bindings`, by profile then control. */
function editor(bindings: Record<string, Partial<Record<Control, ControlFields>>> = { global: GLOBAL }) {
  return {
    key: () => Promise.reject(new Error("the panel reads no key")),
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
}

function panel({ status = CONNECTED, profile = "vscode", selected, bindings }: Props = {}) {
  const onselect = vi.fn<(control: Control) => void>();
  const keys = editor(bindings);
  const result = render(Dialpad, { props: { editor: keys, status, profile, selected, onselect } });
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
});
