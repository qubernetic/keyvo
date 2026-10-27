// The one i18n module (docs/design/gui-ux.md §6): English only, every
// string by key, placeholders filled by name.
import { describe, expect, it } from "vitest";

import { messages, t } from "./i18n";

describe("t", () => {
  it("returns the English string of a key", () => {
    expect(t("app.name")).toBe("keyvo");
  });

  it("fills the named placeholders", () => {
    expect(t("editor.wrote", { key: 3, path: "/c/p.toml" })).toBe("Wrote key 3 to /c/p.toml");
  });

  it("refuses a placeholder the caller does not fill", () => {
    expect(() => t("editor.wrote", { key: 3 })).toThrow("editor.wrote needs {path}");
  });
});

describe("messages", () => {
  it("has no empty string", () => {
    for (const [key, text] of Object.entries(messages)) {
      expect(text, key).not.toBe("");
    }
  });

  it("follows the house typography: no em dash", () => {
    for (const text of Object.values(messages)) {
      expect(text).not.toContain("—");
    }
  });
});
