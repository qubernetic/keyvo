// The actions catalogue's pure side (pad screen brief §5.2): the v1 list,
// the search over group and label, what a tool does to the form, and the
// drag data a tile reads.
import { describe, expect, it } from "vitest";

import {
  applied,
  CATALOGUE,
  dragData,
  dropped,
  GROUPS,
  search,
  type CatalogueEntry,
} from "./catalogue";
import { toForm } from "./editor";
import { t } from "./i18n";
import { TILE_ICONS } from "./tileIcons";

function entry(id: string): CatalogueEntry {
  const found = CATALOGUE.find((candidate) => candidate.id === id);
  if (found === undefined) throw new Error(`no entry ${id}`);
  return found;
}

describe("the v1 list", () => {
  it("has the groups General, System and Apps, the raw tools at the end of General", () => {
    expect(GROUPS.map((group) => t(group.label))).toEqual(["General", "System", "Apps"]);
    const labels = (group: string) =>
      CATALOGUE.filter((candidate) => candidate.group === group).map((candidate) => t(candidate.label));
    expect(labels("general")).toEqual(["Open folder", "Open website", "New email", "Chord", "Exec"]);
    expect(labels("system")).toEqual(["Lock the desktop", "Volume", "Brightness", "Open a settings page"]);
    expect(labels("apps")).toEqual(["Spectacle", "Calculator", "VS Code", "Konsole"]);
  });

  it("makes Chord and Exec tools and every other entry a preset that is not built yet", () => {
    expect(CATALOGUE.filter((candidate) => candidate.kind === "tool").map((c) => c.id)).toEqual([
      "chord",
      "exec",
    ]);
    for (const candidate of CATALOGUE.filter((c) => c.kind === "recipe")) {
      expect(candidate.soon).toBe("soon.presets");
    }
  });

  it("draws every entry with a bundled tile icon", () => {
    for (const candidate of CATALOGUE) expect(TILE_ICONS).toContain(candidate.icon);
  });
});

describe("search", () => {
  it("narrows over the label, case-insensitive", () => {
    expect(search(CATALOGUE, "eXe").map((c) => c.id)).toEqual(["exec"]);
  });

  it("matches the group's name", () => {
    expect(search(CATALOGUE, "apps").map((c) => c.id)).toEqual([
      "spectacle",
      "calculator",
      "vscode",
      "konsole",
    ]);
  });

  it("keeps every entry for an empty or blank text and none for a text nothing has", () => {
    expect(search(CATALOGUE, "  ")).toEqual(CATALOGUE);
    expect(search(CATALOGUE, "zzz")).toEqual([]);
  });
});

describe("applied", () => {
  const form = toForm({ label: "Build", action: { type: "chord", keys: "ctrl+b" } });

  it("sets the action type of Chord and Exec with an empty field and keeps the rest", () => {
    expect(applied(entry("exec"), form)).toEqual({ ...form, actionType: "exec", action: "" });
    expect(applied(entry("chord"), toForm({}))).toEqual({ ...toForm({}), actionType: "chord", action: "" });
  });

  it("leaves the form alone for a preset", () => {
    expect(applied(entry("spectacle"), form)).toBe(form);
  });
});

describe("the drag data", () => {
  it("carries a tool's id, which a drop reads back", () => {
    expect(dragData(entry("exec"))).toBe("tool:exec");
    expect(dropped("tool:exec")).toBe("exec");
  });

  it("drops nothing for a preset, an unknown tool or foreign data", () => {
    expect(dropped("recipe:spectacle")).toBeUndefined();
    expect(dropped("tool:nope")).toBeUndefined();
    expect(dropped("https://example.org")).toBeUndefined();
  });
});
