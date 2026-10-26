import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import { drawing, inline, panelPlacement, placement } from "./device";
import { nth, responses } from "./fixtures";
import type { Geometry, Status } from "./protocol";

const KEYPAD = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), "../assets/device/keypad.svg"),
  "utf8",
);

/** The geometry of the connected status reply in the socket fixtures. */
function fixtureGeometry(): Geometry {
  const geometry = (responses()[0] as Status).device.geometry;
  if (geometry === undefined) throw new Error("the fixtures have no geometry");
  return geometry;
}

describe("drawing", () => {
  it("reads the view box and the nine key cutouts of the Keypad", () => {
    const keypad = drawing(KEYPAD);
    expect(keypad.width).toBe(512);
    expect(keypad.height).toBe(610);
    expect(keypad.cutouts).toHaveLength(9);
    expect(keypad.cutouts[0]).toEqual({ x: 57, y: 60, width: 117, height: 117 });
    expect(keypad.cutouts[8]).toEqual({ x: 331, y: 334, width: 117, height: 117 });
  });
});

describe("placement", () => {
  it("puts the fixture's nine keys at the cutouts in percent", () => {
    const keypad = drawing(KEYPAD);
    const places = placement(keypad, fixtureGeometry()) ?? [];
    expect(places).toHaveLength(9);
    places.forEach((place, index) => {
      const cutout = nth(keypad.cutouts, index);
      expect(place.key).toBe(index + 1);
      expect(Math.abs(place.left - (cutout.x / 512) * 100)).toBeLessThan(0.5);
      expect(Math.abs(place.top - (cutout.y / 610) * 100)).toBeLessThan(0.5);
      expect(Math.abs(place.width - (cutout.width / 512) * 100)).toBeLessThan(0.5);
      expect(Math.abs(place.height - (cutout.height / 610) * 100)).toBeLessThan(0.5);
    });
  });

  it("gives up when the cutouts and the rects differ in count", () => {
    const geometry = fixtureGeometry();
    const twelve = { ...geometry, rects: [...geometry.rects, ...geometry.rects.slice(0, 3)] };
    expect(placement(drawing(KEYPAD), twelve)).toBeUndefined();
  });
});

describe("panelPlacement", () => {
  it("puts every rect on the plain panel in percent of the panel", () => {
    const geometry = fixtureGeometry();
    const places = panelPlacement(geometry);
    expect(places).toHaveLength(9);
    expect(places[4]).toEqual({
      key: 5,
      left: (181 / 480) * 100,
      top: (163 / 480) * 100,
      width: (118 / 480) * 100,
      height: (118 / 480) * 100,
    });
  });
});

describe("inline", () => {
  const svg = inline(KEYPAD, "d7", "The Keypad & co");

  it("drops the header comment and titles the drawing", () => {
    expect(svg.startsWith("<svg")).toBe(true);
    expect(svg).toMatch(/^<svg[^>]*role="img"[^>]*aria-labelledby="d7-title"[^>]*><title id="d7-title">The Keypad &amp; co<\/title>/);
  });

  it("prefixes every id and every reference with the instance", () => {
    const ids = [...svg.matchAll(/\bid="([^"]+)"/g)].map((match) => match[1]);
    expect(ids.length).toBeGreaterThan(10);
    expect(ids.every((id) => id?.startsWith("d7-"))).toBe(true);
    expect(svg).toContain('fill="url(#d7-kp-body)"');
    expect(svg).toContain('fill="url(#d7-kp-btn)"');
    expect(svg).not.toMatch(/url\(#(?!d7-)/);
  });

  it("gives two instances distinct gradient ids", () => {
    const other = inline(KEYPAD, "d8", "The Keypad");
    expect(other).toContain('id="d8-kp-body"');
    expect(other).not.toContain('id="d7-kp-body"');
  });
});
