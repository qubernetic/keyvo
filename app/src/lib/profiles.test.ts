import { describe, expect, it } from "vitest";

import {
  fromHeaderForm,
  ordered,
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

  it("refuses an empty name and an accent that is not #rrggbb", () => {
    expect(fromHeaderForm({ name: "  ", accent: "" })).toEqual({
      invalid: "Enter a name to write it.",
    });
    expect(fromHeaderForm({ name: "A", accent: "green" })).toEqual({
      invalid: "The accent green is not written #rrggbb.",
    });
  });

  it("compares accents without case", () => {
    expect(sameHeader({ name: "A", accent: "#ABCDEF" }, { name: "A", accent: "#abcdef" })).toBe(true);
    expect(sameHeader({ name: "A" }, { name: "A", accent: "#abcdef" })).toBe(false);
    expect(sameHeader({ name: "A" }, { name: "B" })).toBe(false);
  });
});
