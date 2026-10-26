// The tile icons the picker offers are the ones the renderer bundles: the
// names are read from the same directory, so the two cannot drift.
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";

import { TILE_ICONS, tileIcon, tileIconValue } from "./tileIcons";

const LUCIDE = join(dirname(fileURLToPath(import.meta.url)), "../../../crates/keyvo-core/assets/lucide");

it("names every icon under crates/keyvo-core/assets/lucide, sorted, without the aliases", () => {
  const svgs = readdirSync(LUCIDE).filter((file) => file.endsWith(".svg"));
  const drawn = svgs.filter((file) => readFileSync(join(LUCIDE, file), "utf8").includes("<svg"));
  expect(TILE_ICONS).toEqual(drawn.map((file) => file.slice(0, -".svg".length)).sort());
  // The one alias file today: the renderer draws it as circle-question-mark.
  expect(svgs.length - drawn.length).toBe(1);
  expect(TILE_ICONS).not.toContain("circle-help");
  expect(TILE_ICONS).toContain("circle-question-mark");
});

it.each(TILE_ICONS)("%s is an SVG drawn in the text colour", (name) => {
  const svg = tileIcon(name);
  expect(svg).toMatch(/<svg[\s>]/);
  expect(svg).toContain('stroke="currentColor"');
});

it("has no markup for a name it does not bundle", () => {
  expect(tileIcon("no-such-icon")).toBeUndefined();
});

it("writes a name as lucide:<name> and reads it back", () => {
  expect(tileIconValue.write("terminal")).toBe("lucide:terminal");
  expect(tileIconValue.read("lucide:terminal")).toBe("terminal");
  expect(tileIconValue.read("emoji:x")).toBeUndefined();
  expect(tileIconValue.read("")).toBeUndefined();
});
