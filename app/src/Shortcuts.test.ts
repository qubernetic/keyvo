// The keyboard map (pad screen brief §7; critique heuristic 6): the
// overflow menu's "Keyboard shortcuts" drawer lists the shortcuts that
// work, from lib/shortcuts.ts, and the ones still to come with their
// roadmap sentence.
import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";

import Shortcuts from "./Shortcuts.svelte";
import { t } from "./lib/i18n";
import { SHORTCUTS } from "./lib/shortcuts";

function shortcuts() {
  const onclose = vi.fn();
  render(Shortcuts, { props: { onclose } });
  return { dialog: screen.getByRole("dialog", { name: "Keyboard shortcuts" }), onclose };
}

/** The table's body rows as [keys, action] text. */
function rows(dialog: HTMLElement): string[][] {
  const table = within(dialog).getByRole("table", { name: "Keyboard shortcuts" });
  return within(table)
    .getAllByRole("row")
    .slice(1)
    .map((row) => within(row).getAllByRole("cell").map((cell) => cell.textContent?.trim() ?? ""));
}

describe("SHORTCUTS", () => {
  it("lists the working keys in the order of the brief", () => {
    expect(SHORTCUTS.filter((row) => row.soon === undefined).map((row) => t(row.keys))).toEqual([
      "Ctrl+1 to Ctrl+3",
      "1 to 9",
      "Arrow keys",
      "Space",
      "Shift+Space",
      "Ctrl+Z",
      "Ctrl+Shift+Z",
      "Escape",
    ]);
  });

  it("names the roadmap item of Ctrl+X and Ctrl+V", () => {
    const soon = SHORTCUTS.filter((row) => row.soon !== undefined);
    expect(soon.map((row) => t(row.keys))).toEqual(["Ctrl+X", "Ctrl+V"]);
    for (const row of soon) expect(t(row.soon ?? "soon")).toMatch(/M4a-4/);
  });
});

describe("Shortcuts", () => {
  it("is a non-modal drawer with one row per shortcut", () => {
    const { dialog } = shortcuts();
    expect(dialog).toHaveAttribute("aria-modal", "false");
    expect(rows(dialog)).toEqual(
      SHORTCUTS.map((row) => [
        t(row.keys),
        row.soon === undefined ? t(row.action) : `${t(row.action)} ${t(row.soon)}`,
      ]),
    );
  });

  it("names the columns", () => {
    const { dialog } = shortcuts();
    expect(within(dialog).getAllByRole("columnheader").map((cell) => cell.textContent)).toEqual([
      "Keys",
      "Action",
    ]);
  });

  it("shows the keys as keys", () => {
    const { dialog } = shortcuts();
    expect(within(dialog).getByText("Ctrl+1 to Ctrl+3").tagName).toBe("KBD");
  });

  it("closes on Escape and on its close button", async () => {
    const { dialog, onclose } = shortcuts();
    const close = within(dialog).getByRole("button", { name: "Close the keyboard shortcuts" });
    expect(close).toHaveFocus();
    await fireEvent.keyDown(window, { key: "Escape" });
    expect(onclose).toHaveBeenCalledTimes(1);
    await fireEvent.click(close);
    expect(onclose).toHaveBeenCalledTimes(2);
  });
});
