// The icon picker: a combobox over the tile icons keyvo-core bundles
// (lib/tileIcons.ts), each row with its glyph and name; the pick is
// written as `lucide:<name>`.
import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";

import IconPicker from "./IconPicker.svelte";
import { TILE_ICONS } from "./lib/tileIcons";

function picker(value = "") {
  const onpick = vi.fn<(value: string) => void>();
  const view = render(IconPicker, { props: { label: "Icon", value, onpick } });
  return { onpick, view, input: screen.getByRole("combobox", { name: "Icon" }) };
}

function options(): string[] {
  return within(screen.getByRole("listbox", { name: "Icon" }))
    .getAllByRole("option")
    .map((option) => option.textContent.trim());
}

describe("IconPicker", () => {
  it("lists the first 60 tile icons on an empty search, each with its glyph and name", async () => {
    const { input } = picker();
    expect(input).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    await fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(input).toHaveAttribute("aria-expanded", "true");
    expect(options()).toEqual(TILE_ICONS.slice(0, 60));
    expect(screen.getByRole("option", { name: TILE_ICONS[0] }).querySelector("svg")).not.toBeNull();
    expect(screen.getByText(`60 of ${TILE_ICONS.length}, keep typing`)).toBeInTheDocument();
  });

  it("shows at most 60 matches and names how many more match", async () => {
    const { input } = picker();
    const arrows = TILE_ICONS.filter((name) => name.includes("arrow"));
    expect(arrows.length).toBeGreaterThan(60);
    await fireEvent.input(input, { target: { value: "arrow" } });
    expect(options()).toEqual(arrows.slice(0, 60));
    expect(screen.getByText(`60 of ${arrows.length}, keep typing`)).toBeInTheDocument();
    await fireEvent.input(input, { target: { value: "volume-" } });
    expect(screen.queryByText(/keep typing/)).not.toBeInTheDocument();
  });

  it("moves over the shown rows only", async () => {
    const { input } = picker();
    const arrows = TILE_ICONS.filter((name) => name.includes("arrow"));
    await fireEvent.input(input, { target: { value: "arrow" } });
    await fireEvent.keyDown(input, { key: "ArrowUp" });
    expect(screen.getByRole("option", { name: arrows[59] })).toHaveAttribute("aria-selected", "true");
    await fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(screen.getByRole("option", { name: arrows[0] })).toHaveAttribute("aria-selected", "true");
  });

  it("filters on typing, case-insensitive", async () => {
    const { input } = picker();
    await fireEvent.input(input, { target: { value: "VOLUME-" } });
    expect(options()).toEqual(["volume-1", "volume-2", "volume-off", "volume-x"]);
    await fireEvent.input(input, { target: { value: "nothing-like-it" } });
    expect(screen.getByText("No icon matches")).toBeInTheDocument();
  });

  it("moves with the arrows and picks with Enter, written as lucide:<name>", async () => {
    const { input, onpick } = picker();
    await fireEvent.input(input, { target: { value: "volume-" } });
    await fireEvent.keyDown(input, { key: "ArrowDown" });
    await fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(input).toHaveAttribute("aria-activedescendant", screen.getByRole("option", { name: "volume-2" }).id);
    expect(screen.getByRole("option", { name: "volume-2" })).toHaveAttribute("aria-selected", "true");
    await fireEvent.keyDown(input, { key: "ArrowUp" });
    await fireEvent.keyDown(input, { key: "Enter" });
    expect(onpick).toHaveBeenCalledWith("lucide:volume-1");
    expect(input).toHaveValue("volume-1");
    expect(input).toHaveAttribute("aria-expanded", "false");
  });

  it("wraps at both ends", async () => {
    const { input } = picker();
    await fireEvent.input(input, { target: { value: "volume-" } });
    await fireEvent.keyDown(input, { key: "ArrowUp" });
    expect(screen.getByRole("option", { name: "volume-x" })).toHaveAttribute("aria-selected", "true");
    await fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(screen.getByRole("option", { name: "volume-1" })).toHaveAttribute("aria-selected", "true");
  });

  it("picks with a click", async () => {
    const { input, onpick } = picker();
    await fireEvent.input(input, { target: { value: "rock" } });
    await fireEvent.click(screen.getByRole("option", { name: "rocket" }));
    expect(onpick).toHaveBeenCalledWith("lucide:rocket");
  });

  it("closes on Escape and puts the chosen name back", async () => {
    const { input, onpick } = picker("lucide:rocket");
    expect(input).toHaveValue("rocket");
    await fireEvent.input(input, { target: { value: "volume-" } });
    await fireEvent.keyDown(input, { key: "Escape" });
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(input).toHaveValue("rocket");
    expect(onpick).not.toHaveBeenCalled();
  });

  it("shows the value as written under the field", () => {
    picker("lucide:rocket");
    // The id stays in the body font (DESIGN.md: paths and ids are not code).
    expect(screen.getByText("lucide:rocket").tagName).toBe("SPAN");
  });

  it("removes the icon when the field is emptied", async () => {
    const { input, onpick } = picker("lucide:rocket");
    await fireEvent.input(input, { target: { value: "" } });
    await fireEvent.change(input);
    expect(onpick).toHaveBeenCalledWith("");
  });

  it("keeps a value it does not bundle as written", async () => {
    const { input, onpick } = picker("emoji:rocket");
    expect(input).toHaveValue("emoji:rocket");
    await fireEvent.change(input);
    expect(onpick).not.toHaveBeenCalled();
  });

  it("follows a new value from outside", async () => {
    const { input, view } = picker("lucide:rocket");
    await view.rerender({ value: "lucide:terminal" });
    expect(input).toHaveValue("terminal");
  });
});
