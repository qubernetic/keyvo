// The actions catalogue column (pad screen brief §5.2, §6, §7): the three
// groups with their entries, the tools draggable and assignable to the
// selected tile, the presets "Coming soon" naming #119, the search across
// groups and its empty state, and `/` to the search.
import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";

import Catalogue from "./Catalogue.svelte";
import { DRAG_TYPE } from "./lib/catalogue";

function catalogue(props: { selected?: number; active?: boolean } = {}) {
  const onassign = vi.fn<(entry: string) => void>();
  const result = render(Catalogue, { props: { onassign, ...props } });
  return { onassign, ...result };
}

function group(name: string): HTMLElement {
  return screen.getByRole("button", { name });
}

function entries(name: string): string[] {
  const list = screen.getByRole("list", { name });
  return within(list)
    .getAllByRole("listitem")
    .map((item) => within(item).getByRole("button").getAttribute("aria-label") ?? "");
}

describe("the groups", () => {
  it("show General, System and Apps expanded, each with its entries", () => {
    catalogue();
    expect(screen.getByRole("region", { name: "Actions" })).toBeInTheDocument();
    for (const name of ["General", "System", "Apps"]) {
      expect(group(name)).toHaveAttribute("aria-expanded", "true");
    }
    expect(entries("General")).toEqual(["Open folder", "Open website", "New email", "Chord", "Exec"]);
    expect(entries("System")).toEqual(["Lock the desktop", "Volume", "Brightness", "Open a settings page"]);
    expect(entries("Apps")).toEqual(["Spectacle", "Calculator", "VS Code", "Konsole"]);
  });

  it("collapse and expand on their heading button", async () => {
    catalogue();
    await fireEvent.click(group("System"));
    expect(group("System")).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("list", { name: "System" })).toBeNull();
    await fireEvent.click(group("System"));
    expect(group("System")).toHaveAttribute("aria-expanded", "true");
    expect(entries("System")).toHaveLength(4);
  });
});

describe("the entries", () => {
  it("make Chord and Exec draggable with the entry in the drag data", async () => {
    catalogue({ selected: 7 });
    const exec = screen.getByRole("button", { name: "Exec" });
    expect(exec).toHaveAttribute("draggable", "true");
    const setData = vi.fn();
    await fireEvent.dragStart(exec, { dataTransfer: { setData, effectAllowed: "none" } });
    expect(setData).toHaveBeenCalledWith(DRAG_TYPE, "tool:exec");
  });

  it("a click on a tool assigns it to the selected tile", async () => {
    const { onassign } = catalogue({ selected: 7 });
    const chord = screen.getByRole("button", { name: "Chord" });
    expect(chord).toHaveAccessibleDescription("Assign to key 7, or drag it onto a key.");
    await fireEvent.click(chord);
    expect(onassign).toHaveBeenCalledWith("chord");
  });

  it("a click assigns nothing while no tile is selected and says why", async () => {
    const { onassign } = catalogue();
    const exec = screen.getByRole("button", { name: "Exec" });
    expect(exec).toHaveAttribute("aria-disabled", "true");
    expect(exec).toHaveAccessibleDescription("Select a key to assign it, or drag it onto a key.");
    await fireEvent.click(exec);
    expect(onassign).not.toHaveBeenCalled();
  });

  it("make the presets Coming soon controls naming #119", async () => {
    const { onassign } = catalogue({ selected: 7 });
    const spectacle = screen.getByRole("button", { name: "Spectacle" });
    expect(spectacle).toHaveAttribute("aria-disabled", "true");
    expect(spectacle).not.toHaveAttribute("draggable", "true");
    spectacle.focus();
    await fireEvent.focus(spectacle);
    expect(spectacle).toHaveAccessibleDescription(/Coming soon.*#119/);
    await fireEvent.click(spectacle);
    expect(onassign).not.toHaveBeenCalled();
  });
});

describe("the search", () => {
  it("narrows across groups and hides a group with no match", async () => {
    catalogue();
    const field = screen.getByRole("searchbox", { name: "Search actions" });
    expect(field).toHaveAttribute("placeholder", "Search actions");
    await fireEvent.input(field, { target: { value: "o" } });
    expect(entries("General")).toEqual(["Open folder", "Open website", "Chord"]);
    expect(entries("System")).toEqual(["Lock the desktop", "Volume", "Open a settings page"]);
    await fireEvent.input(field, { target: { value: "konsole" } });
    expect(screen.queryByRole("button", { name: "General" })).toBeNull();
    expect(entries("Apps")).toEqual(["Konsole"]);
  });

  it("shows the matches of a collapsed group while searching", async () => {
    catalogue();
    await fireEvent.click(group("Apps"));
    await fireEvent.input(screen.getByRole("searchbox"), { target: { value: "code" } });
    expect(entries("Apps")).toEqual(["VS Code"]);
  });

  it("says No action matches under the field", async () => {
    catalogue();
    await fireEvent.input(screen.getByRole("searchbox"), { target: { value: "zzz" } });
    expect(screen.getByText("No action matches")).toBeInTheDocument();
    expect(screen.queryAllByRole("list")).toEqual([]);
  });
});

describe("the keyboard", () => {
  it("/ focuses the search", async () => {
    catalogue();
    expect(await fireEvent.keyDown(window, { key: "/" })).toBe(false);
    expect(screen.getByRole("searchbox")).toHaveFocus();
    expect(screen.getByRole("searchbox")).toHaveAttribute("aria-keyshortcuts", "/");
  });

  it("/ is typed as itself in a field, and left alone while another screen shows", async () => {
    const { rerender } = catalogue();
    const other = document.createElement("input");
    document.body.append(other);
    other.focus();
    expect(await fireEvent.keyDown(other, { key: "/" })).toBe(true);
    expect(other).toHaveFocus();
    await rerender({ active: false });
    document.body.focus();
    expect(await fireEvent.keyDown(window, { key: "/" })).toBe(true);
    expect(screen.getByRole("searchbox")).not.toHaveFocus();
    other.remove();
  });
});
