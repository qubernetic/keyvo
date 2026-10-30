// The actions catalogue's pure side (pad screen brief §5.2): the entries
// from a `recipes` reply with the raw tools, the search over group, name
// and variant names, what an entry does to the form, the drag data a tile
// reads, and the recipe a key carries.
import { describe, expect, it } from "vitest";

import {
  applied,
  appliedToControl,
  catalogueOf,
  dragData,
  dropped,
  GROUPS,
  recipeOf,
  search,
  TOOLS,
  withVariant,
  type CatalogueEntry,
} from "./catalogue";
import { toControlForm, toForm } from "./editor";
import { nth, recipeReplies } from "./fixtures";
import { t } from "./i18n";
import type { Recipe } from "./protocol";
import { TILE_ICONS } from "./tileIcons";

const RECIPES = nth(recipeReplies(), 0).recipes;
const CATALOGUE = catalogueOf(RECIPES);

function entry(id: string): CatalogueEntry {
  const found = CATALOGUE.find((candidate) => candidate.id === id);
  if (found === undefined) throw new Error(`no entry ${id}`);
  return found;
}

function recipe(id: string): Recipe {
  const found = RECIPES.find((candidate) => candidate.id === id);
  if (found === undefined) throw new Error(`no recipe ${id}`);
  return found;
}

describe("the entries", () => {
  it("are the groups General, System and Apps from the reply, the raw tools at the end of General", () => {
    expect(GROUPS.map((group) => t(group.label))).toEqual(["General", "System", "Apps"]);
    const names = (group: string) =>
      CATALOGUE.filter((candidate) => candidate.group === group).map((candidate) => candidate.name);
    expect(names("general")).toEqual(["New email", "Open folder", "Open website", "Chord", "Exec", "Launch"]);
    expect(names("system")).toEqual([
      "Brightness down",
      "Brightness up",
      "Lock the desktop",
      "Mute",
      "Open a settings page",
      "Volume down",
      "Volume up",
    ]);
    expect(names("apps")).toEqual(["Calculator", "Konsole", "Screenshot", "Spectacle", "VS Code"]);
  });

  it("are the raw tools alone without recipes", () => {
    expect(catalogueOf([])).toEqual(TOOLS);
    expect(TOOLS.map((tool) => [tool.key, tool.name])).toEqual([
      ["tool:chord", "Chord"],
      ["tool:exec", "Exec"],
      ["tool:launch", "Launch"],
    ]);
    expect(TOOLS.map((tool) => tool.icon)).toEqual(["square", "terminal", "app-window"]);
  });

  it("carry the recipe and draw it with its tile icon", () => {
    expect(entry("spectacle")).toMatchObject({ key: "recipe:spectacle", kind: "recipe", icon: "camera" });
    expect(entry("spectacle").recipe).toBe(recipe("spectacle"));
    for (const candidate of CATALOGUE) expect(TILE_ICONS).toContain(candidate.icon);
  });
});

describe("search", () => {
  it("narrows over the name, case-insensitive", () => {
    expect(search(CATALOGUE, "eXe").map((c) => c.id)).toEqual(["exec"]);
  });

  it("matches the group's name", () => {
    expect(search(CATALOGUE, "apps").map((c) => c.id)).toEqual([
      "calculator",
      "konsole",
      "screenshot",
      "spectacle",
      "vscode",
    ]);
  });

  it("matches a variant's name", () => {
    expect(search(CATALOGUE, "region").map((c) => c.id)).toEqual(["spectacle"]);
    expect(search(CATALOGUE, "command palette").map((c) => c.id)).toEqual(["vscode"]);
  });

  it("keeps every entry for an empty or blank text and none for a text nothing has", () => {
    expect(search(CATALOGUE, "  ")).toEqual(CATALOGUE);
    expect(search(CATALOGUE, "zzz")).toEqual([]);
  });
});

describe("applied", () => {
  const form = toForm({
    label: "Build",
    color: "#ff0000",
    action: { type: "chord", keys: "ctrl+b" },
  });

  it("sets the action type of Chord, Exec and Launch with an empty field and keeps the rest", () => {
    expect(applied(entry("exec"), form)).toEqual({ ...form, actionType: "exec", action: "" });
    expect(applied(entry("launch"), form)).toEqual({ ...form, actionType: "launch", action: "" });
    expect(applied(entry("chord"), toForm({}))).toEqual({ ...toForm({}), actionType: "chord", action: "" });
  });

  it("writes a preset's default variant with its label and icon, the colour kept", () => {
    expect(applied(entry("spectacle"), form)).toEqual({
      label: "Capture",
      icon: "lucide:camera",
      color: "#ff0000",
      actionType: "exec",
      action: "spectacle --fullscreen",
      windowClass: "",
      waitFocus: true,
    });
  });

  it("writes the Calculator recipe as a launch", () => {
    expect(applied(entry("calculator"), form)).toEqual({
      label: "Calc",
      icon: "lucide:calculator",
      color: "#ff0000",
      actionType: "launch",
      action: "kcalc",
      windowClass: "",
      waitFocus: true,
    });
  });

  it("drops the window class and the wait of the launch it replaces", () => {
    const launch = toForm({ action: { type: "launch", cmd: "kcalc", class: "org.kde.kcalc", wait_focus: false } });
    expect(applied(entry("calculator"), launch)).toMatchObject({ windowClass: "", waitFocus: true });
    expect(applied(entry("launch"), launch)).toMatchObject({ action: "", windowClass: "", waitFocus: true });
  });

  it("rewrites the action for another variant, across action types", () => {
    const capture = applied(entry("spectacle"), form);
    expect(withVariant(recipe("spectacle"), "region", capture)).toEqual({
      ...capture,
      action: "spectacle --region",
    });
    const volume = applied(entry("volume-up"), form);
    expect(withVariant(recipe("volume-up"), "key", volume)).toEqual({
      ...volume,
      actionType: "chord",
      action: "volumeup",
    });
    expect(withVariant(recipe("volume-up"), "nope", volume)).toBe(volume);
  });
});

describe("appliedToControl", () => {
  const dial = toControlForm({
    cw: { type: "chord", keys: "KEY_VOLUMEUP" },
    ccw: { type: "chord", keys: "KEY_VOLUMEDOWN" },
    step: 5,
    label: "Volume",
  });

  it("sets a button's action and the preset's label, as a recipe does for a key", () => {
    const form = appliedToControl(entry("mute"), "button:1", "cw", toControlForm({ label: "Old" }));
    expect(form.label).toBe("Mute");
    expect(form.action).toEqual({
      actionType: "exec",
      action: expect.stringContaining("invokeShortcut mute"),
      windowClass: "",
      waitFocus: true,
    });
    expect(form.cw).toEqual(toControlForm({}).cw);
    expect(appliedToControl(entry("chord"), "button:2", "cw", form)).toEqual({
      ...form,
      action: { actionType: "chord", action: "", windowClass: "", waitFocus: true },
    });
  });

  it("sets the direction a rotary was last edited in, the rest kept", () => {
    const up = appliedToControl(entry("volume-up"), "dial", "cw", dial);
    expect(up).toEqual({ ...dial, cw: { ...dial.cw, actionType: "exec", action: expect.stringContaining("increase_volume") } });
    const down = appliedToControl(entry("exec"), "roller", "ccw", dial);
    expect(down).toEqual({ ...dial, ccw: { ...dial.ccw, actionType: "exec", action: "" } });
  });
});

describe("the drag data", () => {
  it("carries an entry's key, which a drop reads back", () => {
    expect(dragData(entry("exec"))).toBe("tool:exec");
    expect(dropped("tool:exec")).toBe("tool:exec");
    expect(dragData(entry("launch"))).toBe("tool:launch");
    expect(dropped("tool:launch")).toBe("tool:launch");
    expect(dragData(entry("spectacle"))).toBe("recipe:spectacle");
    expect(dropped("recipe:spectacle")).toBe("recipe:spectacle");
  });

  it("drops nothing for an unknown tool, an empty recipe id or foreign data", () => {
    expect(dropped("tool:nope")).toBeUndefined();
    expect(dropped("recipe:")).toBeUndefined();
    expect(dropped("recipe:a b")).toBeUndefined();
    expect(dropped("https://example.org")).toBeUndefined();
  });
});

describe("recipeOf", () => {
  it("names the recipe and variant of a key with its icon and one of its actions", () => {
    const fields = {
      icon: "lucide:camera",
      label: "Capture",
      action: { type: "exec" as const, cmd: "spectacle --region" },
    };
    expect(recipeOf(RECIPES, fields)).toEqual({ recipe: recipe("spectacle"), variant: "region" });
  });

  it("prefers the pack whose label the key shows when two packs share icon and action", () => {
    const action = { type: "exec" as const, cmd: "spectacle --fullscreen" };
    expect(recipeOf(RECIPES, { icon: "lucide:camera", label: "Shot", action })).toEqual({
      recipe: recipe("screenshot"),
      variant: "full",
    });
    expect(recipeOf(RECIPES, { icon: "lucide:camera", label: "Snap", action })?.recipe.id).toBe(
      "screenshot",
    );
  });

  it("tells a launch from an exec of the same command", () => {
    const launch = { type: "launch" as const, cmd: "flatpak run org.kde.kcalc" };
    expect(recipeOf(RECIPES, { icon: "lucide:calculator", action: launch })).toEqual({
      recipe: recipe("calculator"),
      variant: "flatpak",
    });
    expect(recipeOf(RECIPES, { icon: "lucide:calculator", action: { ...launch, type: "exec" } })).toBeUndefined();
  });

  it("names none for another icon, another action or no action", () => {
    const action = { type: "exec" as const, cmd: "spectacle --region" };
    expect(recipeOf(RECIPES, { icon: "lucide:code", action })).toBeUndefined();
    expect(recipeOf(RECIPES, { icon: "lucide:camera", action: { ...action, cmd: "spectacle" } })).toBeUndefined();
    expect(recipeOf(RECIPES, { icon: "lucide:camera" })).toBeUndefined();
  });
});
