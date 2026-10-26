// Every string a component shows comes from lib/i18n.ts (docs/design/gui-ux.md
// §6): the markup of a component holds no literal words, neither as text
// nor in an attribute a user reads or hears. Every component under src/
// counts, in a folder too.
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const SRC = dirname(fileURLToPath(import.meta.url));

/** Every component under `root`, as paths relative to it. */
function components(root: string): string[] {
  return readdirSync(root, { recursive: true, encoding: "utf8" })
    .filter((file) => file.endsWith(".svelte"))
    .sort();
}

/** The markup of a component: no script, no style, no expressions. */
function markup(source: string): string {
  let text = source
    .replace(/<script[\s\S]*?<\/script>/g, "")
    .replace(/<style[\s\S]*?<\/style>/g, "")
    .replace(/<!--[\s\S]*?-->/g, "");
  // Innermost braces first, so nested template literals go too.
  for (let before = ""; before !== text; ) {
    before = text;
    text = text.replace(/\{[^{}]*\}/g, " ");
  }
  return text;
}

/** The first literal word between the tags of `source`, if any. */
function literalText(source: string): string | undefined {
  const text = markup(source).replace(/<[^>]*>/g, " ");
  return text.trim().match(/[A-Za-z].*/)?.[0];
}

/** The first read or announced attribute of `source` with a literal word. */
function literalAttribute(source: string): string | undefined {
  return /\b(aria-label|title|placeholder|alt)="([^"]*[A-Za-z][^"]*)"/.exec(markup(source))?.[0];
}

function read(file: string): string {
  return readFileSync(join(SRC, file), "utf8");
}

describe("the walk", () => {
  it("finds the components, in folders too", () => {
    const found = components(SRC);
    expect(found).toContain("App.svelte");
    expect(found).toContain(join("testing", "DeviceRows.svelte"));
  });

  it("fails a component with a literal word in a folder", () => {
    const root = mkdtempSync(join(tmpdir(), "kv-strings-"));
    try {
      mkdirSync(join(root, "nested", "deeper"), { recursive: true });
      writeFileSync(join(root, "nested", "deeper", "Bad.svelte"), '<p title="Hi">Hello</p>\n');
      const found = components(root);
      expect(found).toEqual([join("nested", "deeper", "Bad.svelte")]);
      const source = readFileSync(join(root, found[0] ?? ""), "utf8");
      expect(literalText(source)).toBe("Hello");
      expect(literalAttribute(source)).toBe('title="Hi"');
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});

describe.each(components(SRC))("%s", (file) => {
  it("has no literal text between its tags", () => {
    expect(literalText(read(file))).toBeUndefined();
  });

  it("has no literal words in a read or announced attribute", () => {
    expect(literalAttribute(read(file))).toBeUndefined();
  });
});
