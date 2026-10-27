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
  it("lists every tile icon, each with its glyph and name", async () => {
    const { input } = picker();
    expect(input).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    await fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(input).toHaveAttribute("aria-expanded", "true");
    expect(options()).toEqual(TILE_ICONS);
    const terminal = screen.getByRole("option", { name: "terminal" });
    expect(terminal.querySelector("svg")).not.toBeNull();
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
    expect(screen.getByText("lucide:rocket")).toBeInTheDocument();
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
