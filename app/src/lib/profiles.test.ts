import { describe, expect, it } from "vitest";

import { pageListCases, type PageListProfile } from "./fixtures";
import {
  contextPage,
  enabled,
  enabledCount,
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

  it("is the first enabled page without one named context, else page 1", () => {
    expect(contextPage({ ...summary("code", 0), pages: ["a", "b"], disabled: [1] })).toBe(2);
    expect(contextPage({ ...summary("code", 0), pages: ["a", "context"], disabled: [2] })).toBe(2);
    expect(contextPage({ ...summary("code", 0), pages: ["a", "b"], disabled: [1, 2] })).toBe(1);
  });
});

describe("enabled", () => {
  it("is false for the pages a profile switched off", () => {
    const code = { ...summary("code", 0), pages: ["a", "b", "c"], disabled: [2] };
    expect([1, 2, 3].map((page) => enabled(code, page))).toEqual([true, false, true]);
    expect(enabledCount(code)).toBe(2);
    expect(enabledCount(summary("code", 0))).toBe(1);
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

  it("leaves a disabled page out, the context page in all the same", () => {
    const code = { ...summary("code", 0), pages: ["edit", "context", "debug"], disabled: [1, 2] };
    expect(tabs(code, { ...global, disabled: [2] })).toEqual([
      [0, "global", 3, "apps"],
      [1, "code", 2, "context"],
      [2, "code", 3, "debug"],
    ]);
  });

  it("leaves the global pages out of a profile with global_pages false", () => {
    const code = { ...summary("code", 0), global_pages: false };
    expect(tabs(code)).toEqual([[1, "code", 1, "context"]]);
  });

  it.each(pageListCases())("is the shared case: $name", (fixture) => {
    const profile = (id: string, of: PageListProfile): ProfileSummary => ({
      ...summary(id, 0),
      pages: of.pages ?? [],
      ...(of.disabled === undefined ? {} : { disabled: of.disabled }),
      ...(of.global_pages === undefined ? {} : { global_pages: of.global_pages }),
    });
    const global = profile("global", fixture.global);
    const owner = fixture.owner.id === "global" ? global : profile(fixture.owner.id ?? "", fixture.owner);
    expect(pageList(owner, global).map(({ index, profile, page }) => ({ index, profile, page }))).toEqual(
      fixture.list,
    );
  });

  it("tells the global pages left of a context page", () => {
    expect([1, 2, 3, 4].map((page) => leftOfContext(global, page))).toEqual([false, true, true, false]);
    expect(leftOfContext(undefined, 2)).toBe(false);
  });
});

describe("the header form", () => {
  it("round-trips the name and accent", () => {
    const form = toHeaderForm({ name: "Konsole", accent: "#2ecc71" });
    expect(form).toEqual({ name: "Konsole", accent: "#2ecc71", globalPages: true });
    expect(fromHeaderForm(form)).toEqual({ fields: { name: "Konsole", accent: "#2ecc71" } });
  });

  it("round-trips global_pages, absent for true", () => {
    const form = toHeaderForm({ name: "Konsole", global_pages: false });
    expect(form.globalPages).toBe(false);
    expect(fromHeaderForm(form)).toEqual({ fields: { name: "Konsole", global_pages: false } });
    expect(fromHeaderForm({ ...form, globalPages: true })).toEqual({ fields: { name: "Konsole" } });
    expect(toHeaderForm({ name: "A", global_pages: true }).globalPages).toBe(true);
  });

  it("an empty accent removes it and a written one is lower-cased", () => {
    expect(fromHeaderForm({ name: "A", accent: " ", globalPages: true })).toEqual({ fields: { name: "A" } });
    expect(fromHeaderForm({ name: "A", accent: "#ABCDEF", globalPages: true })).toEqual({
      fields: { name: "A", accent: "#abcdef" },
    });
  });

  it("refuses an empty name and an accent that is not #rrggbb, naming the field", () => {
    expect(fromHeaderForm({ name: "  ", accent: "", globalPages: true })).toEqual({
      field: "name",
      invalid: "Enter a name to write it.",
    });
    expect(fromHeaderForm({ name: "A", accent: "green", globalPages: true })).toEqual({
      field: "accent",
      invalid: "The accent green is not written #rrggbb.",
    });
  });

  it("compares accents without case", () => {
    expect(sameHeader({ name: "A", accent: "#ABCDEF" }, { name: "A", accent: "#abcdef" })).toBe(true);
    expect(sameHeader({ name: "A" }, { name: "A", accent: "#abcdef" })).toBe(false);
    expect(sameHeader({ name: "A" }, { name: "B" })).toBe(false);
  });

  it("takes an absent global_pages for true", () => {
    expect(sameHeader({ name: "A" }, { name: "A", global_pages: true })).toBe(true);
    expect(sameHeader({ name: "A" }, { name: "A", global_pages: false })).toBe(false);
  });
});
