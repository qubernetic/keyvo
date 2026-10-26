// Preferences (pad screen brief §5.1; #108 open point 5): a non-modal
// drawer with the tabs General, Devices, Integrations, Projects; none is
// built yet, so every panel is the sentence of its roadmap item.
import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";

import Preferences from "./Preferences.svelte";

function preferences() {
  const onclose = vi.fn();
  render(Preferences, { props: { onclose } });
  return { dialog: screen.getByRole("dialog", { name: "Preferences" }), onclose };
}

function tab(name: string): HTMLElement {
  return screen.getByRole("tab", { name });
}

describe("Preferences", () => {
  it("is a non-modal drawer with the four tabs, General selected", () => {
    const { dialog } = preferences();
    expect(dialog).toHaveAttribute("aria-modal", "false");
    const tablist = within(dialog).getByRole("tablist", { name: "Preferences" });
    expect(within(tablist).getAllByRole("tab").map((tab) => tab.textContent?.trim())).toEqual([
      "General",
      "Devices",
      "Integrations",
      "Projects",
    ]);
    expect(tab("General")).toHaveAttribute("aria-selected", "true");
    expect(tab("Devices")).toHaveAttribute("aria-selected", "false");
  });

  it("shows the roadmap sentence of the selected tab", async () => {
    preferences();
    expect(screen.getByRole("tabpanel", { name: "General" })).toHaveTextContent(
      "Settings are not built yet; they arrive with M4a-8 on the roadmap.",
    );
    await fireEvent.click(tab("Projects"));
    expect(tab("Projects")).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tabpanel", { name: "Projects" })).toHaveTextContent(
      "Projects are not built yet; they arrive with M4a-6 on the roadmap.",
    );
  });

  it("moves between the tabs with the arrows, Home and End", async () => {
    preferences();
    tab("General").focus();
    await fireEvent.keyDown(tab("General"), { key: "ArrowRight" });
    expect(tab("Devices")).toHaveFocus();
    expect(tab("Devices")).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tabpanel", { name: "Devices" })).toHaveTextContent(/M4a-8/);
    await fireEvent.keyDown(tab("Devices"), { key: "ArrowLeft" });
    await fireEvent.keyDown(tab("General"), { key: "ArrowLeft" });
    expect(tab("Projects")).toHaveFocus();
    await fireEvent.keyDown(tab("Projects"), { key: "Home" });
    expect(tab("General")).toHaveFocus();
    await fireEvent.keyDown(tab("General"), { key: "End" });
    expect(tab("Projects")).toHaveFocus();
  });

  it("keeps only the selected tab in the tab order", () => {
    preferences();
    expect(tab("General")).toHaveAttribute("tabindex", "0");
    expect(tab("Devices")).toHaveAttribute("tabindex", "-1");
  });

  it("closes on Escape and on its close button", async () => {
    const { dialog, onclose } = preferences();
    const close = within(dialog).getByRole("button", { name: "Close Preferences" });
    expect(close).toHaveFocus();
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(onclose).toHaveBeenCalledTimes(1);
    await fireEvent.click(close);
    expect(onclose).toHaveBeenCalledTimes(2);
  });
});
