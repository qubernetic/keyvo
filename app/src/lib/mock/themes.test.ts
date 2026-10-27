// The mock's theme switch: four token sets measured from the sources of
// docs/design/gui-ux.md §7, each setting every token tokens.css defines.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  accentOf,
  applyTheme,
  contrast,
  DEFAULT_THEME,
  MEASURED,
  mix,
  mutedOf,
  type ThemeName,
  THEMES,
  themeOf,
} from "./themes";

const tokensCss = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), "../../styles/tokens.css"),
  "utf8",
);
/** Every --kv-* token tokens.css gives a default. */
const TOKENS = [...new Set([...tokensCss.matchAll(/(--kv-[a-z-]+):/g)].map((match) => match[1]))];

describe("the mock themes", () => {
  it("has the four token sets of §7", () => {
    expect(Object.keys(THEMES)).toEqual([
      "breeze-dark",
      "breeze-light",
      "adwaita-dark",
      "adwaita-light",
    ]);
    expect(TOKENS.length).toBeGreaterThanOrEqual(15);
  });

  it.each(Object.keys(THEMES))("%s sets every token and the color-scheme", (name) => {
    const root = document.createElement("div");
    applyTheme(themeOf(`?theme=${name}`), root);
    for (const token of TOKENS) {
      expect(root.style.getPropertyValue(token ?? ""), `${name} ${token}`).not.toBe("");
    }
    expect(root.style.getPropertyValue("color-scheme")).toBe(
      name.endsWith("dark") ? "dark" : "light",
    );
  });

  it("takes the measured values", () => {
    const root = document.createElement("div");
    applyTheme("breeze-light", root);
    expect(root.style.getPropertyValue("--kv-bg")).toBe("#eff0f1");
    expect(root.style.getPropertyValue("--kv-fg")).toBe("#232629");
    applyTheme("adwaita-dark", root);
    expect(root.style.getPropertyValue("--kv-bg")).toBe("#222226");
    expect(root.style.getPropertyValue("--kv-accent")).toBe("#81d0ff");
  });

  it("applies the measured values except the derived muted text and accent", () => {
    for (const [name, measured] of Object.entries(MEASURED)) {
      const applied: Record<string, string> = { ...THEMES[name as ThemeName] };
      delete applied["--kv-fg-muted"];
      delete applied["--kv-accent"];
      const rest: Record<string, string> = { ...measured };
      delete rest["--kv-fg-muted"];
      delete rest["--kv-accent"];
      expect(applied, name).toEqual(rest);
    }
  });

  it("keeps the feedback states of the pad theme in every set", () => {
    for (const theme of Object.values(THEMES)) {
      expect(theme["--kv-state-running"]).toBe("#f08c00");
      expect(theme["--kv-state-success"]).toBe("#2f9e44");
      expect(theme["--kv-state-error"]).toBe("#e03131");
    }
  });

  it("follows the system's reduced motion", () => {
    const root = document.createElement("div");
    applyTheme("breeze-dark", root, () => true);
    expect(root.style.getPropertyValue("--kv-motion")).toBe("reduced");
    applyTheme("breeze-dark", root, () => false);
    expect(root.style.getPropertyValue("--kv-motion")).toBe("full");
  });

  it.each(["breeze-dark", "adwaita-dark"] as const)(
    "%s keeps its measured muted text, which passes 4.5:1",
    (name) => {
      const muted = MEASURED[name]["--kv-fg-muted"] ?? "";
      expect(THEMES[name]["--kv-fg-muted"]).toBe(muted);
      expect(contrast(muted, MEASURED[name]["--kv-bg"] ?? "")).toBeGreaterThanOrEqual(4.5);
      expect(contrast(muted, MEASURED[name]["--kv-bg-alt"] ?? "")).toBeGreaterThanOrEqual(4.5);
    },
  );

  it.each(["breeze-light", "adwaita-light"] as const)(
    "%s derives its muted text: the largest mix of the text toward the window that passes 4.5:1",
    (name) => {
      const { "--kv-fg": fg = "", "--kv-bg": bg = "", "--kv-bg-alt": alt = "" } = MEASURED[name];
      const measured = MEASURED[name]["--kv-fg-muted"] ?? "";
      expect(Math.min(contrast(measured, bg), contrast(measured, alt))).toBeLessThan(4.5);
      const muted = THEMES[name]["--kv-fg-muted"] ?? "";
      expect(muted).not.toBe(measured);
      expect(contrast(muted, bg)).toBeGreaterThanOrEqual(4.5);
      expect(contrast(muted, alt)).toBeGreaterThanOrEqual(4.5);
      const step = [...Array(101).keys()].find((percent) => mix(fg, bg, percent) === muted);
      expect(step).toBeGreaterThan(0);
      const next = mix(fg, bg, (step ?? 0) + 1);
      expect(Math.min(contrast(next, bg), contrast(next, alt))).toBeLessThan(4.5);
    },
  );

  it.each(["breeze-dark", "adwaita-dark", "adwaita-light"] as const)(
    "%s keeps its measured accent, which passes 3:1 as the focus ring",
    (name) => {
      const accent = MEASURED[name]["--kv-accent"] ?? "";
      expect(THEMES[name]["--kv-accent"]).toBe(accent);
      expect(contrast(accent, MEASURED[name]["--kv-bg"] ?? "")).toBeGreaterThanOrEqual(3);
      expect(contrast(accent, MEASURED[name]["--kv-bg-alt"] ?? "")).toBeGreaterThanOrEqual(3);
    },
  );

  it("breeze-light derives its accent: the smallest mix toward the text that passes 3:1", () => {
    const {
      "--kv-fg": fg = "",
      "--kv-bg": bg = "",
      "--kv-bg-alt": alt = "",
      "--kv-accent": measured = "",
    } = MEASURED["breeze-light"];
    expect(contrast(measured, alt)).toBeLessThan(3);
    const accent = THEMES["breeze-light"]["--kv-accent"] ?? "";
    expect(contrast(accent, bg)).toBeGreaterThanOrEqual(3);
    expect(contrast(accent, alt)).toBeGreaterThanOrEqual(3);
    const step = [...Array(101).keys()].find((percent) => mix(measured, fg, percent) === accent);
    expect(step).toBeGreaterThan(0);
    const previous = mix(measured, fg, (step ?? 0) - 1);
    expect(Math.min(contrast(previous, bg), contrast(previous, alt))).toBeLessThan(3);
  });

  it("mutedOf and accentOf take a set's own values", () => {
    const set = { "--kv-bg": "#ffffff", "--kv-bg-alt": "#ffffff", "--kv-fg": "#000000" };
    expect(mutedOf({ ...set, "--kv-fg-muted": "#595959" })).toBe("#595959");
    const derived = mutedOf({ ...set, "--kv-fg-muted": "#aaaaaa" });
    expect(contrast(derived, "#ffffff")).toBeGreaterThanOrEqual(4.5);
    expect(accentOf({ ...set, "--kv-accent": "#0461be" })).toBe("#0461be");
    expect(accentOf({ ...set, "--kv-accent": "#ffffff" })).not.toBe("#ffffff");
  });

  it("mixes in OKLCH as color-mix() does", () => {
    expect(mix("#3daee9", "#232629", 0)).toBe("#3daee9");
    expect(mix("#3daee9", "#232629", 100)).toBe("#232629");
    // OKLab lightness 0.5 is linear 0.125, sRGB 99.
    expect(mix("#000000", "#ffffff", 50)).toBe("#636363");
    expect(contrast("#000000", "#ffffff")).toBeCloseTo(21);
    expect(contrast("#ffffff", "#000000")).toBeCloseTo(21);
  });

  it("defaults to breeze-dark for a missing or unknown theme", () => {
    expect(DEFAULT_THEME).toBe("breeze-dark");
    expect(themeOf("")).toBe("breeze-dark");
    expect(themeOf("?theme=solarized")).toBe("breeze-dark");
    expect(themeOf("?mock-error=unreachable&theme=adwaita-light")).toBe("adwaita-light");
  });
});
