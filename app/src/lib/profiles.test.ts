import { describe, expect, it } from "vitest";

import {
  contextPage,
  fromHeaderForm,
  leftOfContext,
  ordered,
  pageList,
  pageName,
  sameHeader,
  toHeaderForm,
  type ProfileSummary,
} from "./profiles";

function summary(id: string, priority: number): ProfileSummary {
  return {
    id,
    name: id,
    priority,
    match: { class: [] },
    pages: ["context"],
    path: `/c/profiles/${id}.toml`,
    file: true,
  };
}

describe("ordered", () => {
  it("pins the global profile first, then sorts by priority, highest first", () => {
    const list = [summary("code", 1), summary("global", 0), summary("konsole", 5), summary("claude", 10)];
    expect(ordered(list).map((p) => p.id)).toEqual(["global", "claude", "konsole", "code"]);
  });

  it("keeps the global profile first even when another has a higher priority", () => {
    const list = [summary("konsole", 50), summary("global", -3)];
    expect(ordered(list).map((p) => p.id)).toEqual(["global", "konsole"]);
  });

  it("orders equal priorities by id and leaves the input as it was", () => {
    const list = [summary("zed", 2), summary("alacritty", 2), summary("global", 0)];
    expect(ordered(list).map((p) => p.id)).toEqual(["global", "alacritty", "zed"]);
    expect(list.map((p) => p.id)).toEqual(["zed", "alacritty", "global"]);
  });
});

describe("pageName", () => {
  it("is the page's name, or its number when it has none", () => {
    const profile = { ...summary("code", 0), pages: ["edit", null] };
    expect(pageName(profile, 1)).toBe("edit");
    expect(pageName(profile, 2)).toBe("Page 2");
    expect(pageName(profile, 3)).toBe("Page 3");
  });
});

describe("contextPage", () => {
  it("is the page named context, else the first, as the daemon picks the global layer", () => {
    expect(contextPage({ ...summary("global", 0), pages: ["general", "context"] })).toBe(2);
    expect(contextPage({ ...summary("global", 0), pages: ["general", null] })).toBe(1);
    expect(contextPage(undefined)).toBe(1);
  });
});

describe("pageList", () => {
  const global = { ...summary("global", 0), pages: ["context", "general", "apps"] };
  const tabs = (owner: ProfileSummary, of = global) =>
    pageList(owner, of).map((tab) => [tab.index, tab.profile, tab.page, tab.name]);

  it("puts the global pages from general on left of the shown profile's pages", () => {
    const vscode = { ...summary("vscode", 0), pages: ["context", null] };
    expect(tabs(vscode)).toEqual([
      [-1, "global", 3, "apps"],
      [0, "global", 2, "general"],
      [1, "vscode", 1, "context"],
      [2, "vscode", 2, "Page 2"],
    ]);
  });

  it("starts at the context page, the others in file order after it", () => {
    const code = { ...summary("code", 0), pages: ["edit", "context", "general"] };
    expect(tabs(code)).toEqual([
      [-1, "global", 3, "apps"],
      [0, "global", 2, "general"],
      [1, "code", 2, "context"],
      [2, "code", 1, "edit"],
      [3, "code", 3, "general"],
    ]);
  });

  it("walks the global pages after general in file order to the left", () => {
    const wide = { ...global, pages: ["context", "notes", "general", "apps", "media"] };
    expect(tabs(wide, wide)).toEqual([
      [-2, "global", 5, "media"],
      [-1, "global", 4, "apps"],
      [0, "global", 3, "general"],
      [1, "global", 1, "context"],
      [2, "global", 2, "notes"],
    ]);
  });

  it("is the shown profile's pages alone without a global general page", () => {
    const konsole = summary("konsole", 0);
    expect(tabs(konsole, { ...global, pages: ["context"] })).toEqual([[1, "konsole", 1, "context"]]);
    expect(pageList(konsole, undefined).map((tab) => tab.index)).toEqual([1]);
    expect(tabs({ ...konsole, pages: [] }).map(([index]) => index)).toEqual([-1, 0]);
  });

  it("tells the global pages left of a context page", () => {
    expect([1, 2, 3, 4].map((page) => leftOfContext(global, page))).toEqual([false, true, true, false]);
    expect(leftOfContext(undefined, 2)).toBe(false);
  });
});

describe("the header form", () => {
  it("round-trips the name and accent", () => {
    const form = toHeaderForm({ name: "Konsole", accent: "#2ecc71" });
    expect(form).toEqual({ name: "Konsole", accent: "#2ecc71" });
    expect(fromHeaderForm(form)).toEqual({ fields: { name: "Konsole", accent: "#2ecc71" } });
  });

  it("an empty accent removes it and a written one is lower-cased", () => {
    expect(fromHeaderForm({ name: "A", accent: " " })).toEqual({ fields: { name: "A" } });
    expect(fromHeaderForm({ name: "A", accent: "#ABCDEF" })).toEqual({
      fields: { name: "A", accent: "#abcdef" },
    });
  });

  it("refuses an empty name and an accent that is not #rrggbb, naming the field", () => {
    expect(fromHeaderForm({ name: "  ", accent: "" })).toEqual({
      field: "name",
      invalid: "Enter a name to write it.",
    });
    expect(fromHeaderForm({ name: "A", accent: "green" })).toEqual({
      field: "accent",
      invalid: "The accent green is not written #rrggbb.",
    });
  });

  it("compares accents without case", () => {
    expect(sameHeader({ name: "A", accent: "#ABCDEF" }, { name: "A", accent: "#abcdef" })).toBe(true);
    expect(sameHeader({ name: "A" }, { name: "A", accent: "#abcdef" })).toBe(false);
    expect(sameHeader({ name: "A" }, { name: "B" })).toBe(false);
  });
});
