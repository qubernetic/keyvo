// The three columns of a screen (docs/design/gui-ux.md §2): list column,
// editor and right column; the window width collapses the right column
// first, then the list column, and a collapsed column can be shown again.
import { fireEvent, render, screen } from "@testing-library/svelte";
import { createRawSnippet } from "svelte";
import { describe, expect, it } from "vitest";

import Columns from "./Columns.svelte";
import { layout } from "./lib/shell";

function text(content: string) {
  return createRawSnippet(() => ({ render: () => `<p>${content}</p>` }));
}

function columns(width: number) {
  render(Columns, {
    props: {
      columns: layout(width),
      list: text("the list"),
      editor: text("the editor"),
      right: text("the preview"),
    },
  });
}

function shown(): string[] {
  return screen.getAllByRole("region").map((region) => region.getAttribute("aria-label") ?? "");
}

describe("Columns", () => {
  it("shows the list column, the editor and the right column at 1200 px", () => {
    columns(1200);
    expect(shown()).toEqual(["List", "Editor", "Preview"]);
    expect(screen.getByText("the preview")).toBeInTheDocument();
  });

  it("collapses the right column first", () => {
    columns(1100);
    expect(shown()).toEqual(["List", "Editor"]);
    expect(screen.getByRole("button", { name: "Show the right column" })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
  });

  it("collapses the list column next", () => {
    columns(960);
    expect(shown()).toEqual(["Editor"]);
    expect(screen.getByRole("button", { name: "Show the list column" })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
  });

  it("shows a collapsed column again, and hides it on request", async () => {
    columns(960);
    await fireEvent.click(screen.getByRole("button", { name: "Show the list column" }));
    expect(shown()).toEqual(["List", "Editor"]);
    const hide = screen.getByRole("button", { name: "Hide the list column" });
    expect(hide).toHaveAttribute("aria-expanded", "true");
    await fireEvent.click(hide);
    expect(shown()).toEqual(["Editor"]);
  });

  it("has no list or right column when the screen has none", () => {
    render(Columns, { props: { columns: layout(1200), editor: text("the editor") } });
    expect(shown()).toEqual(["Editor"]);
    expect(screen.queryByRole("button")).toBeNull();
  });
});
