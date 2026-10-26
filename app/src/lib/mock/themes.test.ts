// The mock's theme switch: four token sets measured from the sources of
// docs/design/gui-ux.md §7, each setting every token tokens.css defines.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import { applyTheme, DEFAULT_THEME, THEMES, themeOf } from "./themes";

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
    expect(root.style.getPropertyValue("--kv-accent")).toBe("#3daee9");
    applyTheme("adwaita-dark", root);
    expect(root.style.getPropertyValue("--kv-bg")).toBe("#222226");
    expect(root.style.getPropertyValue("--kv-accent")).toBe("#81d0ff");
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

  it("defaults to breeze-dark for a missing or unknown theme", () => {
    expect(DEFAULT_THEME).toBe("breeze-dark");
    expect(themeOf("")).toBe("breeze-dark");
    expect(themeOf("?theme=solarized")).toBe("breeze-dark");
    expect(themeOf("?mock-error=unreachable&theme=adwaita-light")).toBe("adwaita-light");
  });
});
