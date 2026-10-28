// The trace drawer (docs/design/gui-ux.md §8) against the trace entries of
// the shared socket fixtures (tests/protocol-fixtures/socket/): one section
// per part of the entry that the daemon records today, in resolution order.
import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";

import TraceDrawer from "./TraceDrawer.svelte";
import { eventsOf, nth, responses } from "./lib/fixtures";
import type { TraceRecord } from "./lib/protocol";

/** The fixture entries: chord sent, exec exited 101, its on_fail run, exec
 * failed, dry run, unbound, launch started, blocked under the session lock. */
const ENTRIES = eventsOf("trace").map((event) => event.entry);

function drawer(entry: TraceRecord, onclose = vi.fn()) {
  render(TraceDrawer, { props: { entry, onclose } });
  return { dialog: screen.getByRole("dialog", { name: "Trace entry" }), onclose };
}

/** The section headings of the drawer, in order. */
function sections(dialog: HTMLElement): string[] {
  return within(dialog)
    .getAllByRole("heading", { level: 3 })
    .map((heading) => heading.textContent ?? "");
}

function section(dialog: HTMLElement, name: string): HTMLElement {
  return within(dialog).getByRole("region", { name });
}

/** The value next to the term `term` in `region`. */
function value(region: HTMLElement, term: string): string {
  return within(region).getByText(term, { selector: "dt" }).nextElementSibling?.textContent ?? "";
}

describe("TraceDrawer", () => {
  it("shows a sent chord: trigger, context, resolved action and the key events", () => {
    const { dialog } = drawer(nth(ENTRIES, 0));
    expect(sections(dialog)).toEqual(["Trigger", "Context snapshot", "Resolved action", "Result"]);
    const trigger = section(dialog, "Trigger");
    // The date and a 24 h time, as the strip's times are 24 h.
    expect(value(trigger, "Time")).toBe(
      new Date(1792929600250).toLocaleString(undefined, { hourCycle: "h23" }),
    );
    expect(value(trigger, "Key")).toBe("4");
    expect(value(trigger, "Press")).toBe("short");
    expect(value(trigger, "Source")).toBe("the device");
    const context = section(dialog, "Context snapshot");
    expect(value(context, "Profile")).toBe("demo");
    expect(value(context, "Page")).toBe("1");
    const action = section(dialog, "Resolved action");
    expect(value(action, "Layer")).toBe("app layer");
    expect(value(action, "Type")).toBe("chord");
    expect(value(action, "Keys")).toBe("ctrl+p");
    const result = section(dialog, "Result");
    expect(value(result, "Status")).toBe("sent");
    expect(value(result, "Key events")).toBe("29 down, 25 down, 25 up, 29 up");
  });

  it("shows an exec's exit code and duration", () => {
    const { dialog } = drawer(nth(ENTRIES, 1));
    expect(value(section(dialog, "Resolved action"), "Command")).toBe("cargo build");
    const result = section(dialog, "Result");
    expect(value(result, "Status")).toBe("exited");
    expect(value(result, "Exit code")).toBe("101");
    expect(value(result, "Duration")).toBe("2318 ms");
  });

  it("shows the on_fail run of a failed press under its result", () => {
    render(TraceDrawer, { props: { entry: nth(ENTRIES, 1), onFail: nth(ENTRIES, 2), onclose: vi.fn() } });
    const dialog = screen.getByRole("dialog", { name: "Trace entry" });
    expect(sections(dialog)).toEqual(["Trigger", "Context snapshot", "Resolved action", "Result"]);
    const result = section(dialog, "Result");
    expect(value(result, "Exit code")).toBe("101");
    expect(value(result, "on_fail")).toBe("exec notify-send 'build failed' -> exited 0 after 41 ms");
  });

  it("names the branch of an on_fail entry opened alone", () => {
    const { dialog } = drawer(nth(ENTRIES, 2));
    const action = section(dialog, "Resolved action");
    expect(value(action, "Branch")).toBe("on_fail");
    expect(value(action, "Command")).toBe("notify-send 'build failed'");
    expect(within(section(dialog, "Result")).queryByText("on_fail", { selector: "dt" })).toBeNull();
  });

  it("shows a launch that was let go as started with its pid", () => {
    const { dialog } = drawer(nth(ENTRIES, 6));
    const action = section(dialog, "Resolved action");
    expect(value(action, "Type")).toBe("launch");
    expect(value(action, "Command")).toBe("flatpak run org.kde.kcalc");
    expect(value(section(dialog, "Result"), "Status")).toBe("started, pid 4711");
  });

  it("shows an exec ended by a signal", () => {
    const entry = nth(ENTRIES, 1);
    const { dialog } = drawer({ ...entry, result: { status: "exited", code: null, duration_ms: 40 } });
    expect(value(section(dialog, "Result"), "Exit code")).toBe("none, ended by a signal");
  });

  it("shows why an action failed", () => {
    const { dialog } = drawer(nth(ENTRIES, 3));
    const result = section(dialog, "Result");
    expect(value(result, "Status")).toBe("failed");
    expect(value(result, "Error")).toBe("no such file or directory: gh");
  });

  it("gives a dry run's block reason instead of a result", () => {
    const { dialog } = drawer(nth(ENTRIES, 4));
    expect(sections(dialog)).toEqual([
      "Trigger",
      "Context snapshot",
      "Resolved action",
      "Block reason",
    ]);
    expect(value(section(dialog, "Trigger"), "Press")).toBe("long");
    expect(value(section(dialog, "Trigger"), "Source")).toBe("simulated");
    expect(section(dialog, "Block reason")).toHaveTextContent(
      "Dry run: resolved, nothing executed.",
    );
  });

  it("says no layer binds an unbound key", () => {
    const { dialog } = drawer(nth(ENTRIES, 5));
    const action = section(dialog, "Resolved action");
    expect(value(action, "Layer")).toBe("none");
    expect(value(action, "Type")).toBe("none");
    expect(section(dialog, "Block reason")).toHaveTextContent(
      "Unbound: no layer binds this key on this page.",
    );
  });

  it("gives a blocked press's reason instead of a result", () => {
    const { dialog } = drawer(nth(ENTRIES, 7));
    expect(sections(dialog)).toEqual([
      "Trigger",
      "Context snapshot",
      "Resolved action",
      "Block reason",
    ]);
    expect(value(section(dialog, "Resolved action"), "Type")).toBe("none");
    expect(section(dialog, "Block reason")).toHaveTextContent(
      "Blocked: session locked. Nothing was resolved or run.",
    );
  });

  it("shows a running exec as running", () => {
    const pending = (responses()[4] as { entry: TraceRecord }).entry;
    const { dialog } = drawer(pending);
    expect(value(section(dialog, "Result"), "Status")).toBe("running");
  });

  it("names a result status it does not know", () => {
    const { dialog } = drawer({ ...nth(ENTRIES, 0), result: { status: "teleported" } });
    expect(value(section(dialog, "Result"), "Status")).toBe("teleported");
  });

  it("is a non-modal dialog", () => {
    const { dialog } = drawer(nth(ENTRIES, 0));
    expect(dialog).toHaveAttribute("aria-modal", "false");
  });

  it("carries the Debug switch in its header, disabled until the daemon sends HID events", () => {
    const { dialog } = drawer(nth(ENTRIES, 0));
    const debug = within(dialog).getByRole("switch", { name: "Debug log" });
    expect(debug.closest("header")).not.toBeNull();
    expect(debug).toHaveAttribute("aria-checked", "false");
    expect(debug).toHaveAttribute("aria-disabled", "true");
    expect(debug).toHaveAccessibleDescription(/Coming soon/);
  });

  it("gives the focus back to the control that opened it", async () => {
    const opener = document.createElement("button");
    document.body.append(opener);
    opener.focus();
    const { unmount } = render(TraceDrawer, { props: { entry: nth(ENTRIES, 0), onclose: vi.fn() } });
    expect(opener).not.toHaveFocus();
    unmount();
    expect(opener).toHaveFocus();
    opener.remove();
  });

  it("leaves an Escape another control handled alone", async () => {
    const { onclose } = drawer(nth(ENTRIES, 0));
    const event = new KeyboardEvent("keydown", { key: "Escape", cancelable: true });
    event.preventDefault();
    window.dispatchEvent(event);
    expect(onclose).not.toHaveBeenCalled();
  });

  it("takes the focus and closes on Esc and on its close button", async () => {
    const { dialog, onclose } = drawer(nth(ENTRIES, 0));
    const close = within(dialog).getByRole("button", { name: "Close the trace entry" });
    expect(close).toHaveFocus();
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(onclose).toHaveBeenCalledTimes(1);
    await fireEvent.click(close);
    expect(onclose).toHaveBeenCalledTimes(2);
    await fireEvent.keyDown(window, { key: "Enter" });
    expect(onclose).toHaveBeenCalledTimes(2);
  });
});
