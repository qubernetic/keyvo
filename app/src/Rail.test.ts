// The navigation rail (docs/design/gui-ux.md §2, §6): seven screens in
// order on Ctrl+1 to Ctrl+7; the screens not built yet stay in the tab
// order and say so, but never navigate.
import { fireEvent, render, screen } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";

import Rail from "./Rail.svelte";
import type { ScreenId } from "./lib/shell";

const ORDER = ["Home", "Profiles", "Behaviors", "Projects", "Integrations", "Devices", "Settings"];

function rail(labels = true, active: ScreenId = "home") {
  const onnavigate = vi.fn<(id: ScreenId) => void>();
  render(Rail, { props: { active, labels, onnavigate } });
  return onnavigate;
}

function item(name: string): HTMLElement {
  return screen.getByRole("button", { name });
}

describe("Rail", () => {
  it("lists the screens in order with their shortcuts", () => {
    rail();
    const nav = screen.getByRole("navigation", { name: "Screens" });
    const buttons = [...nav.querySelectorAll("button")];
    expect(buttons.map((button) => button.getAttribute("aria-label"))).toEqual(ORDER);
    expect(buttons.map((button) => button.getAttribute("aria-keyshortcuts"))).toEqual(
      ORDER.map((_, index) => `Control+${index + 1}`),
    );
  });

  it("marks the active screen", () => {
    rail(true, "profiles");
    expect(item("Profiles")).toHaveAttribute("aria-current", "page");
    expect(item("Home")).not.toHaveAttribute("aria-current");
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
    await fireEvent.mouseEnter(item("Projects"));
    expect(screen.getByRole("tooltip")).toHaveTextContent("Coming soon");
    await fireEvent.mouseLeave(item("Projects"));
    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  it("names the roadmap milestone when focused", async () => {
    rail();
    item("Settings").focus();
    await Promise.resolve();
    expect(screen.getByRole("tooltip")).toHaveTextContent(
      "Settings are not built yet; they arrive with M4a-8 on the roadmap.",
    );
    expect(item("Settings")).toHaveAccessibleDescription(/M4a-8/);
  });

  it("navigates with Ctrl+1 to Ctrl+7 to the built screens", async () => {
    const onnavigate = rail();
    await fireEvent.keyDown(window, { key: "2", ctrlKey: true });
    await fireEvent.keyDown(window, { key: "1", ctrlKey: true });
    expect(onnavigate.mock.calls).toEqual([["profiles"], ["home"]]);
  });

  it("focuses and announces a screen not built yet on its shortcut", async () => {
    const onnavigate = rail();
    await fireEvent.keyDown(window, { key: "6", ctrlKey: true });
    expect(onnavigate).not.toHaveBeenCalled();
    expect(item("Devices")).toHaveFocus();
    expect(screen.getByRole("tooltip")).toHaveTextContent(/M4a-8/);
  });

  it("shows the labels beside the icons", () => {
    rail(true);
    expect(item("Home")).toHaveTextContent("Home");
    expect(item("Home").querySelector("svg")).not.toBeNull();
  });

  it("shows only the icons in a narrow window, named all the same", () => {
    rail(false);
    expect(item("Home")).toHaveTextContent("");
    expect(item("Home").querySelector("svg")).not.toBeNull();
  });
});
