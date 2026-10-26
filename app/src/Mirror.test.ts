// The mirror in the device drawing (pad screen brief §5.2): the tiles in
// the cutouts at the fixture geometry's percent rects, click selects and
// does not press, the keyboard of the grid, and the marks of a tile.
import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";

import Mirror from "./Mirror.svelte";
import type { Tile } from "./lib/daemon";
import { responses } from "./lib/fixtures";
import { t } from "./lib/i18n";
import type { Geometry, PressKind, Status } from "./lib/protocol";

const GEOMETRY = (responses()[0] as Status).device.geometry as Geometry;

/** Key 3 falls through from the global layer, key 4 is the profile's own. */
const TILES: Tile[] = Array.from({ length: 9 }, (_, index) => ({
  image: `data:image/png;base64,${index + 1}`,
  ...(index === 2
    ? { label: "Type", action: { type: "chord" as const, keys: "shift+k" }, layer: "global" as const }
    : {}),
  ...(index === 3
    ? { label: "Palette", action: { type: "chord" as const, keys: "ctrl+p" }, layer: "app" as const }
    : {}),
}));

interface Props {
  selected?: number;
  held?: number[];
  profile?: string;
  tiles?: Tile[];
}

function mirror(props: Props = {}) {
  const onselect = vi.fn<(key: number) => void>();
  const onpress = vi.fn<(key: number, kind: PressKind) => void>();
  const result = render(Mirror, {
    props: {
      geometry: GEOMETRY,
      tiles: TILES,
      profile: "terminal",
      held: [],
      name: "Keypad mirror: profile terminal, page 1",
      onselect,
      onpress,
      ...props,
    },
  });
  return { onselect, onpress, grid: screen.getByRole("grid"), ...result };
}

function key(n: number): HTMLElement {
  return screen.getByRole("button", { name: new RegExp(`^Key ${n}\\b`) });
}

function cell(n: number): HTMLElement {
  return key(n).closest('[role="gridcell"]') as HTMLElement;
}

function percent(element: HTMLElement, property: string): number {
  const style = element.getAttribute("style") ?? "";
  const value = new RegExp(`(?:^|;)\\s*${property}:\\s*([\\d.]+)%`).exec(style)?.[1];
  return value === undefined ? Number.NaN : Number(value);
}

describe("the mirror", () => {
  it("places the nine tiles in the drawing's cutouts by percent", () => {
    const { grid } = mirror();
    expect(grid).toHaveAccessibleName("Keypad mirror: profile terminal, page 1");
    const rows = within(grid).getAllByRole("row");
    expect(rows.map((row) => within(row).getAllByRole("gridcell").length)).toEqual([3, 3, 3]);
    expect(within(grid).getByRole("img", { name: t("device.keypad") })).toBeInTheDocument();
    expect(cell(1)).toHaveStyle({ position: "absolute" });
    expect(percent(cell(1), "left")).toBeCloseTo((57 / 512) * 100, 1);
    expect(percent(cell(9), "top")).toBeCloseTo((334 / 610) * 100, 1);
    expect(percent(cell(9), "width")).toBeCloseTo((117 / 512) * 100, 1);
    // Never above 1:1: the drawing is capped at the panel's pixel width.
    const device = grid.querySelector(".device") as HTMLElement;
    expect(device).toHaveStyle({ width: "100%", maxWidth: `${GEOMETRY.panel.width}px` });
    expect(key(1).querySelector("img")).toHaveAttribute("src", "data:image/png;base64,1");
    expect(key(1).querySelector("img")).toHaveAttribute("alt", "");
    expect(key(4)).toHaveAccessibleName("Key 4: Palette, chord ctrl+p, app layer");
  });

  it("shows the key number in a cutout without a tile", () => {
    mirror({ tiles: [] });
    expect(key(5)).toHaveTextContent("5");
    expect(key(5)).toHaveAccessibleName("Key 5: unbound");
  });

  it("a click selects the key and does not press it", async () => {
    const { onselect, onpress } = mirror();
    await fireEvent.click(key(5));
    expect(onselect).toHaveBeenCalledWith(5);
    expect(onpress).not.toHaveBeenCalled();
    // No right-click press any more: the browser's menu is left alone.
    expect(await fireEvent.contextMenu(key(2))).toBe(true);
    expect(onpress).not.toHaveBeenCalled();
  });

  it("outlines the selected key and gives it the tab stop", () => {
    mirror({ selected: 7 });
    expect(key(7)).toHaveClass("selected");
    expect(key(1)).not.toHaveClass("selected");
    expect(key(7)).toHaveAttribute("tabindex", "0");
    expect(key(1)).toHaveAttribute("tabindex", "-1");
    expect(screen.getByRole("gridcell", { selected: true })).toContainElement(key(7));
  });

  it("gives key 1 the tab stop while nothing is selected", () => {
    mirror();
    expect(key(1)).toHaveAttribute("tabindex", "0");
    expect(screen.queryByRole("gridcell", { selected: true })).toBeNull();
  });

  it("1 to 9 select and focus a key; Space presses it short, Shift+Space long", async () => {
    const { grid, onselect, onpress, rerender } = mirror();
    await fireEvent.keyDown(grid, { key: "7" });
    expect(onselect).toHaveBeenLastCalledWith(7);
    await rerender({ selected: 7 });
    expect(key(7)).toHaveFocus();
    expect(await fireEvent.keyDown(grid, { key: " " })).toBe(false);
    expect(onpress).toHaveBeenLastCalledWith(7, "short");
    await fireEvent.keyDown(grid, { key: " ", shiftKey: true });
    expect(onpress).toHaveBeenLastCalledWith(7, "long");
    await fireEvent.keyDown(grid, { key: "0" });
    await fireEvent.keyDown(grid, { key: "a" });
    expect(onselect).toHaveBeenCalledTimes(1);
    expect(onpress).toHaveBeenCalledTimes(2);
  });

  it("Space presses nothing while no key is selected", async () => {
    const { grid, onpress } = mirror();
    await fireEvent.keyDown(grid, { key: " " });
    expect(onpress).not.toHaveBeenCalled();
  });

  it("the arrows move the selection inside the grid", async () => {
    const { grid, onselect, rerender } = mirror({ selected: 5 });
    const step = async (arrow: string) => {
      await fireEvent.keyDown(grid, { key: arrow });
      const next = onselect.mock.lastCall?.[0];
      await rerender({ selected: next });
      return next;
    };
    expect(await step("ArrowRight")).toBe(6);
    expect(await step("ArrowRight")).toBe(6);
    expect(await step("ArrowDown")).toBe(9);
    expect(await step("ArrowLeft")).toBe(8);
    await step("ArrowUp");
    await step("ArrowUp");
    expect(await step("ArrowUp")).toBe(2);
    expect(key(2)).toHaveFocus();
  });

  it("an arrow with nothing selected starts at key 1", async () => {
    const { grid, onselect } = mirror();
    await fireEvent.keyDown(grid, { key: "ArrowRight" });
    expect(onselect).toHaveBeenLastCalledWith(2);
  });

  it("dims a key that falls through from the global layer and marks its corner", () => {
    mirror();
    expect(key(3)).toHaveAccessibleName("Key 3: Type, chord shift+k, global layer");
    expect(key(3)).toHaveClass("fallen");
    expect(within(key(3)).getByText("global")).toBeInTheDocument();
    expect(key(4)).not.toHaveClass("fallen");
    expect(within(key(4)).queryByText("global")).toBeNull();
  });

  it("does not mark global keys on the global profile", () => {
    mirror({ profile: "global" });
    expect(key(3)).not.toHaveClass("fallen");
    expect(within(key(3)).queryByText("global")).toBeNull();
  });

  it("marks the keys held down on the device", () => {
    mirror({ held: [5] });
    expect(key(5)).toHaveClass("held");
    expect(key(4)).not.toHaveClass("held");
  });
});
