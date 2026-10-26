// The page tabs under the drawing (pad screen brief §5.2, decision Q5): the
// pinned profile's pages by name, the "+" not built yet, the accent at the
// right.
import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";

import PageTabs from "./PageTabs.svelte";
import type { ProfileSummary } from "./lib/profiles";

const VSCODE: ProfileSummary = {
  id: "vscode",
  name: "VS Code",
  accent: "#e07a1f",
  priority: 20,
  match: { class: ["code"] },
  pages: ["context", null, "git"],
  path: "/home/u/.config/keyvo/profiles/vscode.toml",
  file: true,
};

function tabs(profile: ProfileSummary = VSCODE, page = 2) {
  const onpage = vi.fn<(page: number) => void>();
  render(PageTabs, { props: { profile, page, onpage } });
  return { onpage, list: screen.getByRole("tablist", { name: "Pages" }) };
}

describe("the page tabs", () => {
  it("names each page, numbers an unnamed one, and selects the pinned page", () => {
    const { list } = tabs();
    const all = within(list).getAllByRole("tab");
    expect(all.map((tab) => tab.textContent?.trim())).toEqual(["context", "Page 2", "git"]);
    expect(within(list).getByRole("tab", { selected: true })).toHaveTextContent("Page 2");
    expect(all.map((tab) => tab.getAttribute("tabindex"))).toEqual(["-1", "0", "-1"]);
  });

  it("a click pins the page", async () => {
    const { list, onpage } = tabs();
    await fireEvent.click(within(list).getByRole("tab", { name: "git" }));
    expect(onpage).toHaveBeenCalledWith(3);
  });

  it("the arrows wrap, Home and End go to the ends", async () => {
    const { list, onpage } = tabs();
    await fireEvent.keyDown(list, { key: "ArrowRight" });
    expect(onpage).toHaveBeenLastCalledWith(3);
    await fireEvent.keyDown(list, { key: "ArrowLeft" });
    expect(onpage).toHaveBeenLastCalledWith(1);
    await fireEvent.keyDown(list, { key: "Home" });
    expect(onpage).toHaveBeenLastCalledWith(1);
    await fireEvent.keyDown(list, { key: "End" });
    expect(onpage).toHaveBeenLastCalledWith(3);
    await fireEvent.keyDown(list, { key: "a" });
    expect(onpage).toHaveBeenCalledTimes(4);
  });

  it("the arrows wrap past the last page", async () => {
    const { list, onpage } = tabs(VSCODE, 3);
    await fireEvent.keyDown(list, { key: "ArrowRight" });
    expect(onpage).toHaveBeenLastCalledWith(1);
  });

  it("offers adding a page as a control not built yet", async () => {
    tabs();
    const add = screen.getByRole("button", { name: "Add a page" });
    expect(add).toHaveAttribute("aria-disabled", "true");
    expect(add).toHaveTextContent("+");
    add.focus();
    await Promise.resolve();
    expect(add).toHaveAccessibleDescription(/M4a-4/);
  });

  it("shows the accent as a labelled swatch with its hex", () => {
    tabs();
    const accent = screen.getByRole("img", { name: "Accent #e07a1f" });
    expect(accent).toHaveStyle({ backgroundColor: "#e07a1f" });
    expect(screen.getByText("Accent")).toBeInTheDocument();
    expect(screen.getByText("#e07a1f")).toBeInTheDocument();
  });

  it("shows no swatch for a profile without an accent", () => {
    const { accent: _, ...plain } = VSCODE;
    tabs(plain);
    expect(screen.queryByRole("img", { name: /^Accent/ })).toBeNull();
    expect(screen.queryByText("Accent")).toBeNull();
  });

  it("shows only the + for a profile with no pages", async () => {
    const { list, onpage } = tabs({ ...VSCODE, pages: [] }, 1);
    expect(within(list).queryAllByRole("tab")).toHaveLength(0);
    expect(screen.getByRole("button", { name: "Add a page" })).toBeInTheDocument();
    await fireEvent.keyDown(list, { key: "End" });
    expect(onpage).not.toHaveBeenCalled();
  });
});
