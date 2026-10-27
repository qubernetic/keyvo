// The navigation rail (docs/design/gui-ux.md §2, §6; pad screen brief
// §5.1): three screens in order on Ctrl+1 to Ctrl+3; Behaviors, not built
// yet, stays in the tab order and says so, but never navigates.
import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";

import Rail from "./Rail.svelte";
import type { ScreenId } from "./lib/shell";

const ORDER = ["Pad", "Profiles", "Behaviors"];

function rail(labels = true, active: ScreenId = "home") {
  const onnavigate = vi.fn<(id: ScreenId) => void>();
  render(Rail, { props: { active, labels, onnavigate } });
  return onnavigate;
}

function item(name: string): HTMLElement {
  return screen.getByRole("button", { name });
}

describe("Rail", () => {
  it("lists the three screens in order with their shortcuts", () => {
    rail();
    const nav = screen.getByRole("navigation", { name: "Screens" });
    const buttons = [...nav.querySelectorAll("button")];
    expect(buttons.map((button) => button.getAttribute("aria-label"))).toEqual(ORDER);
    expect(buttons.map((button) => button.getAttribute("aria-keyshortcuts"))).toEqual([
      "Control+1",
      "Control+2",
      "Control+3",
    ]);
  });

  it("marks the active screen", () => {
    rail(true, "profiles");
    expect(item("Profiles")).toHaveAttribute("aria-current", "page");
    expect(item("Pad")).not.toHaveAttribute("aria-current");
  });

  it("navigates to a built screen on a click", async () => {
    const onnavigate = rail();
    await fireEvent.click(item("Profiles"));
    expect(onnavigate).toHaveBeenCalledWith("profiles");
  });

  it("keeps a screen not built yet focusable, announced and inert", async () => {
    const onnavigate = rail();
    const behaviors = item("Behaviors");
    expect(behaviors).toHaveAttribute("aria-disabled", "true");
    expect(behaviors).not.toHaveAttribute("disabled");
    expect(behaviors).toHaveAccessibleDescription(/Coming soon/);
    await fireEvent.click(behaviors);
    expect(onnavigate).not.toHaveBeenCalled();
  });

  it("shows Coming soon on hover", async () => {
    rail();
    expect(screen.queryByRole("tooltip")).toBeNull();
    await fireEvent.mouseEnter(item("Behaviors"));
    expect(screen.getByRole("tooltip")).toHaveTextContent("Coming soon");
    await fireEvent.mouseLeave(item("Behaviors"));
    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  it("names the roadmap milestone when focused", async () => {
    rail();
    item("Behaviors").focus();
    await Promise.resolve();
    expect(screen.getByRole("tooltip")).toHaveTextContent(
      "Behaviors are not built yet; they arrive with M4a-5 on the roadmap.",
    );
    expect(item("Behaviors")).toHaveAccessibleDescription(/M4a-5/);
  });

  it("navigates with Ctrl+1 and Ctrl+2 to the built screens", async () => {
    const onnavigate = rail();
    await fireEvent.keyDown(window, { key: "2", ctrlKey: true });
    await fireEvent.keyDown(window, { key: "1", ctrlKey: true });
    expect(onnavigate.mock.calls).toEqual([["profiles"], ["home"]]);
  });

  it("focuses and announces Behaviors on Ctrl+3", async () => {
    const onnavigate = rail();
    await fireEvent.keyDown(window, { key: "3", ctrlKey: true });
    expect(onnavigate).not.toHaveBeenCalled();
    expect(item("Behaviors")).toHaveFocus();
    expect(screen.getByRole("tooltip")).toHaveTextContent(/M4a-5/);
  });

  it("ignores Ctrl+4", async () => {
    const onnavigate = rail();
    await fireEvent.keyDown(window, { key: "4", ctrlKey: true });
    expect(onnavigate).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(document.body);
  });

  it("shows the labels beside the icons", () => {
    rail(true);
    expect(item("Pad")).toHaveTextContent("Pad");
    expect(item("Pad").querySelector("svg")).not.toBeNull();
  });

  it("shows only the icons in a narrow window, named all the same", () => {
    rail(false);
    expect(item("Pad")).toHaveTextContent("");
    expect(item("Pad").querySelector("svg")).not.toBeNull();
  });
});
