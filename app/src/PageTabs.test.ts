// The page tabs under the drawing (pad screen brief §5.2, decision Q5; #151,
// #156): the page list the pad walks, the global pages at index 0 and below
// muted and marked as the global profile's, then the shown profile's pages
// by name with the ones switched off muted among them, the "+" after them,
// the accent at the right.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";

import PageTabs from "./PageTabs.svelte";
import { pageList, type ProfileSummary } from "./lib/profiles";

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

const GLOBAL: ProfileSummary = {
  id: "global",
  name: "Global",
  priority: 0,
  match: { class: [] },
  pages: ["context", "general", "apps"],
  path: "/home/u/.config/keyvo/profiles/global.toml",
  file: true,
};

function tabs(profile: ProfileSummary = VSCODE, selected: number | null = 2, pinned?: number) {
  const onpage = vi.fn<(profile: string, page: number) => void>();
  const onadd = vi.fn<() => void>();
  const onenable = vi.fn<(page: number) => void>();
  render(PageTabs, {
    props: {
      profile,
      global: GLOBAL,
      tabs: pageList(profile, GLOBAL),
      selected: selected ?? undefined,
      pinned,
      onpage,
      onadd,
      onenable,
    },
  });
  return { onpage, onadd, onenable, list: screen.getByRole("tablist", { name: "Pages" }) };
}

const texts = (list: HTMLElement) => within(list).getAllByRole("tab").map((tab) => tab.textContent?.trim());
/** Every name of the strip, the tabs' and the switched-off pages', in order. */
const strip = (list: HTMLElement) => [...list.querySelectorAll(".name")].map((name) => name.textContent);

describe("the page tabs", () => {
  it("show the list in order: the global pages, then the shown profile's", () => {
    const { list } = tabs();
    expect(texts(list)).toEqual(["apps", "general", "context", "Page 2", "git"]);
  });

  it("mark the global pages at index 0 and below as the global profile's, muted", () => {
    const { list } = tabs();
    const all = within(list).getAllByRole("tab");
    expect(all.map((tab) => tab.getAttribute("aria-label"))).toEqual([
      "apps, a page of Global",
      "general, a page of Global",
      "context",
      "Page 2",
      "git",
    ]);
    expect(all.map((tab) => tab.classList.contains("global"))).toEqual([true, true, false, false, false]);
    expect(all[0]).toHaveAttribute("title", "apps, a page of Global");
    expect(all[2]).toHaveAttribute("title", "context");
  });

  it("truncate a long page name with an ellipsis and keep the whole name in the title", () => {
    const name = "a page with a name so long that it would push the strip past the drawing";
    const { list } = tabs({ ...VSCODE, pages: ["context", name] }, 1);
    const tab = within(list).getByRole("tab", { name });
    expect(tab).toHaveAttribute("title", name);
    const css = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "PageTabs.svelte"), "utf8");
    const rule = /\n  \[role="tab"\] \{([^}]*)\}/.exec(css)?.[1] ?? "";
    expect(rule).toMatch(/max-width: 16ch;/);
    expect(rule).toMatch(/overflow: hidden;/);
    expect(rule).toMatch(/text-overflow: ellipsis;/);
    expect(rule).toMatch(/white-space: nowrap;/);
  });

  it("select the tab at the given index", () => {
    const { list } = tabs();
    expect(within(list).getByRole("tab", { selected: true })).toHaveTextContent("Page 2");
    const all = within(list).getAllByRole("tab");
    expect(all.map((tab) => tab.getAttribute("tabindex"))).toEqual(["-1", "-1", "-1", "0", "-1"]);
  });

  it("mark no tab as pinned unless asked", () => {
    const { list } = tabs();
    for (const tab of within(list).getAllByRole("tab")) {
      expect(tab.querySelector(".icon")).toBeNull();
      expect(tab.getAttribute("aria-label")).not.toMatch(/pinned/);
    }
  });

  it("mark the pinned tab with the pin glyph and say so in its name and title", () => {
    const { list } = tabs(VSCODE, 2, 2);
    const pinned = within(list).getByRole("tab", { name: "Page 2, pinned" });
    expect(pinned).toHaveAttribute("title", "Page 2, pinned");
    expect(pinned).toHaveAttribute("aria-selected", "true");
    expect(pinned.querySelector(".icon svg.lucide-pin")).not.toBeNull();
    expect(pinned).toHaveTextContent("Page 2");
    expect(within(list).getAllByRole("tab").filter((tab) => tab.querySelector(".icon") !== null)).toEqual([pinned]);
  });

  it("mark a pinned global tab, keeping its global name", () => {
    const { list } = tabs(VSCODE, 0, 0);
    expect(within(list).getByRole("tab", { name: "general, a page of Global, pinned" })).toBeInTheDocument();
  });

  it("select a global tab at index 0", () => {
    const { list } = tabs(VSCODE, 0);
    expect(within(list).getByRole("tab", { selected: true })).toHaveTextContent("general");
  });

  it("let the first tab take the focus when none is selected", () => {
    const { list } = tabs(VSCODE, null);
    expect(within(list).queryByRole("tab", { selected: true })).toBeNull();
    const all = within(list).getAllByRole("tab");
    expect(all.map((tab) => tab.getAttribute("tabindex"))).toEqual(["0", "-1", "-1", "-1", "-1"]);
  });

  it("a click pins the page of the profile that owns it", async () => {
    const { list, onpage } = tabs();
    await fireEvent.click(within(list).getByRole("tab", { name: "git" }));
    expect(onpage).toHaveBeenLastCalledWith("vscode", 3);
    await fireEvent.click(within(list).getByRole("tab", { name: "general, a page of Global" }));
    expect(onpage).toHaveBeenLastCalledWith("global", 2);
  });

  it("the arrows wrap, Home and End go to the ends", async () => {
    const { list, onpage } = tabs();
    await fireEvent.keyDown(list, { key: "ArrowRight" });
    expect(onpage).toHaveBeenLastCalledWith("vscode", 3);
    await fireEvent.keyDown(list, { key: "ArrowLeft" });
    expect(onpage).toHaveBeenLastCalledWith("vscode", 1);
    await fireEvent.keyDown(list, { key: "Home" });
    expect(onpage).toHaveBeenLastCalledWith("global", 3);
    await fireEvent.keyDown(list, { key: "End" });
    expect(onpage).toHaveBeenLastCalledWith("vscode", 3);
    await fireEvent.keyDown(list, { key: "a" });
    expect(onpage).toHaveBeenCalledTimes(4);
  });

  it("the arrows wrap past either end", async () => {
    const { list, onpage } = tabs(VSCODE, 3);
    await fireEvent.keyDown(list, { key: "ArrowRight" });
    expect(onpage).toHaveBeenLastCalledWith("global", 3);
  });

  it("adds a page with the + after the shown profile's tabs", async () => {
    const { list, onadd } = tabs();
    const add = screen.getByRole("button", { name: "Add a page" });
    expect(add).toBeEnabled();
    expect(add).toHaveTextContent("+");
    const last = within(list).getAllByRole("tab").at(-1);
    expect(last?.compareDocumentPosition(add)).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    await fireEvent.click(add);
    expect(onadd).toHaveBeenCalledTimes(1);
  });

  it("keeps the + disabled without a way to add a page", () => {
    render(PageTabs, {
      props: { profile: VSCODE, global: GLOBAL, tabs: pageList(VSCODE, GLOBAL), selected: 1, onpage: vi.fn() },
    });
    expect(screen.getByRole("button", { name: "Add a page" })).toBeDisabled();
  });

  it("shows a page switched off muted in file order among the shown profile's tabs, no tab", () => {
    const off = { ...VSCODE, pages: ["context", "edit", null, "git"], disabled: [2, 3] };
    const { list } = tabs(off, 1);
    expect(texts(list)).toEqual(["apps", "general", "context", "git"]);
    expect(strip(list)).toEqual(["apps", "general", "context", "edit", "Page 3", "git"]);
    const muted = list.querySelectorAll(".off");
    expect([...muted].map((item) => item.getAttribute("title"))).toEqual([
      "edit, switched off",
      "Page 3, switched off",
    ]);
    // The arrows pass it by: from context the next tab is git.
    expect(within(list).getAllByRole("tab").map((tab) => tab.getAttribute("tabindex"))).toEqual([
      "-1",
      "-1",
      "0",
      "-1",
    ]);
  });

  it("puts a page switched off after the shown profile's last tab, and the context page first", () => {
    const off = { ...VSCODE, pages: ["edit", "context", "git"], disabled: [1, 3] };
    const { list } = tabs(off, 1);
    expect(strip(list)).toEqual(["apps", "general", "context", "edit", "git"]);
  });

  it("switches a page that is off on with its switch", async () => {
    const off = { ...VSCODE, disabled: [3] };
    const { list, onenable, onpage } = tabs(off, 1);
    const toggle = within(list).getByRole("switch", { name: "Switch git on" });
    expect(toggle).not.toBeChecked();
    await fireEvent.click(toggle);
    expect(onenable).toHaveBeenCalledWith(3);
    expect(onpage).not.toHaveBeenCalled();
    // It stays off until the list says otherwise.
    expect(toggle).not.toBeChecked();
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

  it("shows only the + for an empty list", async () => {
    const onpage = vi.fn<(profile: string, page: number) => void>();
    render(PageTabs, { props: { profile: VSCODE, global: GLOBAL, tabs: [], selected: 1, onpage } });
    const list = screen.getByRole("tablist", { name: "Pages" });
    expect(within(list).queryAllByRole("tab")).toHaveLength(0);
    expect(screen.getByRole("button", { name: "Add a page" })).toBeInTheDocument();
    await fireEvent.keyDown(list, { key: "End" });
    expect(onpage).not.toHaveBeenCalled();
  });
});
