// The actions catalogue column (pad screen brief §5.2, §6, §7): the three
// groups with the entries of a `recipes` reply and the raw tools, every
// entry draggable and assignable to the selected tile, the search across
// groups, names and variant names and its empty state, why presets are
// missing, and `/` to the search.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";

import Catalogue from "./Catalogue.svelte";
import { catalogueOf, DRAG_TYPE } from "./lib/catalogue";
import { nth, recipeReplies } from "./lib/fixtures";

const ENTRIES = catalogueOf(nth(recipeReplies(), 0).recipes);

function catalogue(
  props: { selected?: number; active?: boolean; problems?: string[]; unread?: string } = {},
) {
  const onassign = vi.fn<(entry: string) => void>();
  const result = render(Catalogue, { props: { entries: ENTRIES, onassign, ...props } });
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
  it("let a long entry name wrap inside the column instead of widening it", () => {
    const css = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "Catalogue.svelte"), "utf8");
    const rule = /\n  \.entry \{([^}]*)\}/.exec(css)?.[1] ?? "";
    expect(rule).toMatch(/min-width: 0;/);
    expect(rule).toMatch(/overflow-wrap: anywhere;/);
  });

  it("set the hint and the notes in the body size, as DESIGN.md Body says of notes", () => {
    const css = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "Catalogue.svelte"), "utf8");
    const rule = /\n  \.hint,\n  \.note \{([^}]*)\}/.exec(css)?.[1] ?? "";
    expect(rule).toMatch(/color: var\(--kv-fg-muted\);/);
    expect(rule).not.toMatch(/font-size/);
  });

  it("show General, System and Apps expanded, each with the reply's entries", () => {
    catalogue();
    expect(screen.getByRole("region", { name: "Actions" })).toBeInTheDocument();
    for (const name of ["General", "System", "Apps"]) {
      expect(group(name)).toHaveAttribute("aria-expanded", "true");
    }
    expect(entries("General")).toEqual(["New email", "Open folder", "Open website", "Chord", "Exec"]);
    expect(entries("System")).toEqual([
      "Brightness down",
      "Brightness up",
      "Lock the desktop",
      "Mute",
      "Open a settings page",
      "Volume down",
      "Volume up",
    ]);
    expect(entries("Apps")).toEqual(["Calculator", "Konsole", "Screenshot", "Spectacle", "VS Code"]);
  });

  it("collapse and expand on their heading button", async () => {
    catalogue();
    await fireEvent.click(group("System"));
    expect(group("System")).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("list", { name: "System" })).toBeNull();
    await fireEvent.click(group("System"));
    expect(group("System")).toHaveAttribute("aria-expanded", "true");
    expect(entries("System")).toHaveLength(7);
  });

  it("hold only the raw tools without recipes", () => {
    render(Catalogue, { props: { entries: catalogueOf([]), onassign: vi.fn() } });
    expect(entries("General")).toEqual(["Chord", "Exec"]);
    expect(screen.queryByRole("button", { name: "Apps" })).toBeNull();
  });
});

describe("the entries", () => {
  it("make a tool and a preset draggable with the entry's key in the drag data", async () => {
    catalogue({ selected: 7 });
    for (const [name, key] of [
      ["Exec", "tool:exec"],
      ["Spectacle", "recipe:spectacle"],
    ] as const) {
      const button = screen.getByRole("button", { name });
      expect(button).toHaveAttribute("draggable", "true");
      const setData = vi.fn();
      await fireEvent.dragStart(button, { dataTransfer: { setData, effectAllowed: "none" } });
      expect(setData).toHaveBeenCalledWith(DRAG_TYPE, key);
    }
  });

  it("a click on a tool or a preset assigns it to the selected tile", async () => {
    const { onassign } = catalogue({ selected: 7 });
    const chord = screen.getByRole("button", { name: "Chord" });
    expect(chord).toHaveAccessibleDescription("Assign to key 7, or drag it onto a key.");
    await fireEvent.click(chord);
    expect(onassign).toHaveBeenLastCalledWith("tool:chord");
    await fireEvent.click(screen.getByRole("button", { name: "Spectacle" }));
    expect(onassign).toHaveBeenLastCalledWith("recipe:spectacle");
  });

  it("a click assigns nothing while no tile is selected and says why", async () => {
    const { onassign } = catalogue();
    const exec = screen.getByRole("button", { name: "Exec" });
    // Dragging works without a selection, so the entry is not dimmed or
    // marked disabled; the hint says what a click needs.
    expect(exec).not.toHaveAttribute("aria-disabled");
    expect(exec).toHaveAccessibleDescription("Select a key to assign it, or drag it onto a key.");
    await fireEvent.click(exec);
    expect(onassign).not.toHaveBeenCalled();
  });
});

describe("missing presets", () => {
  it("names each user pack left out", () => {
    catalogue({ problems: nth(recipeReplies(), 1).problems ?? [] });
    expect(screen.getByRole("list", { name: "Recipe packs left out" })).toHaveTextContent(
      "/home/u/.config/keyvo/recipes/volume.toml:12:19: unknown variant `dbus`",
    );
  });

  it("says why the presets could not be read", () => {
    catalogue({ unread: "cannot reach the keyvo daemon" });
    expect(
      screen.getByText("The presets could not be read: cannot reach the keyvo daemon"),
    ).toBeInTheDocument();
  });
});

describe("the search", () => {
  it("narrows across groups and hides a group with no match", async () => {
    catalogue();
    const field = screen.getByRole("searchbox", { name: "Search actions" });
    expect(field).toHaveAttribute("placeholder", "Search actions");
    await fireEvent.input(field, { target: { value: "open" } });
    expect(entries("General")).toEqual(["Open folder", "Open website"]);
    expect(entries("System")).toEqual(["Open a settings page"]);
    await fireEvent.input(field, { target: { value: "konsole" } });
    expect(screen.queryByRole("button", { name: "General" })).toBeNull();
    expect(entries("Apps")).toEqual(["Konsole"]);
  });

  it("matches a variant's name", async () => {
    catalogue();
    await fireEvent.input(screen.getByRole("searchbox"), { target: { value: "Region" } });
    expect(entries("Apps")).toEqual(["Spectacle"]);
    expect(screen.queryByRole("button", { name: "System" })).toBeNull();
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
