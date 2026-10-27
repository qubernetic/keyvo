// Every icon the shell names is bundled.
import { expect, it } from "vitest";

import { icon, ICONS } from "./icons";

it.each(ICONS)("%s is an SVG drawn in the text colour", (name) => {
  const svg = icon(name);
  expect(svg).toMatch(/<svg[\s>]/);
  expect(svg).toContain('stroke="currentColor"');
});
