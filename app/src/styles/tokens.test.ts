// The theme tokens of docs/design/gui-ux.md §7 all have a fixed default
// until the theme bridge (M4a-1) reads them from the desktop.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";

const TOKENS = [
  "--kv-bg",
  "--kv-bg-alt",
  "--kv-fg",
  "--kv-fg-muted",
  "--kv-accent",
  "--kv-sandbox",
  "--kv-project",
  "--kv-state-running",
  "--kv-state-success",
  "--kv-state-error",
  "--kv-font",
  "--kv-font-size",
  "--kv-radius",
  "--kv-contrast",
  "--kv-motion",
];

const css = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "tokens.css"), "utf8");

/** The value `token` gets in the :root block. */
function root(token: string): string | undefined {
  const block = /:root\s*\{([^}]*)\}/.exec(css)?.[1] ?? "";
  return new RegExp(`${token}:\\s*([^;]+);`).exec(block)?.[1]?.trim();
}

it.each(TOKENS)("%s has a default on :root", (token) => {
  expect(root(token)).toBeTruthy();
});

it("the feedback states match the pad theme in keyvo-core", () => {
  // keyvo_core::render::Theme::default(): running, success, error.
  expect(root("--kv-state-running")).toBe("#f08c00");
  expect(root("--kv-state-success")).toBe("#2f9e44");
  expect(root("--kv-state-error")).toBe("#e03131");
});

it("reduced motion from the system turns --kv-motion to reduced", () => {
  expect(css).toMatch(
    /@media \(prefers-reduced-motion: reduce\)\s*\{\s*:root\s*\{\s*--kv-motion:\s*reduced;/,
  );
});
