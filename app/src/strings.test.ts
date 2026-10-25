// Every string a component shows comes from lib/i18n.ts (docs/design/gui-ux.md
// §6): the markup of a component holds no literal words, neither as text
// nor in an attribute a user reads or hears.
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";

const SRC = dirname(fileURLToPath(import.meta.url));

const components = readdirSync(SRC).filter((file) => file.endsWith(".svelte"));

/** The markup of a component: no script, no style, no expressions. */
function markup(file: string): string {
  let text = readFileSync(join(SRC, file), "utf8")
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

it("finds the components", () => {
  expect(components).toContain("App.svelte");
});

it.each(components)("%s has no literal text between its tags", (file) => {
  const text = markup(file).replace(/<[^>]*>/g, " ");
  expect(text.trim().match(/[A-Za-z].*/)?.[0]).toBeUndefined();
});

it.each(components)("%s has no literal words in a read or announced attribute", (file) => {
  const literal = /\b(aria-label|title|placeholder|alt)="([^"]*[A-Za-z][^"]*)"/.exec(markup(file));
  expect(literal?.[0]).toBeUndefined();
});
