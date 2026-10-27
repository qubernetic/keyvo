import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import { cutoutGeometry, drawing, inline, panelPlacement, placement, styles } from "./device";
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

describe("cutoutGeometry", () => {
  it("makes the Keypad's view box the panel and its nine cutouts the keys", () => {
    const keypad = drawing(KEYPAD);
    const geometry = cutoutGeometry(keypad);
    expect(geometry.panel).toEqual({ width: 512, height: 610 });
    expect(geometry.tile).toEqual({ width: 117, height: 117 });
    expect(geometry.rects).toEqual(keypad.cutouts);
    expect(placement(keypad, geometry)).toHaveLength(9);
  });

  it("has no keys and an empty tile for a drawing without cutouts", () => {
    const geometry = cutoutGeometry(drawing('<svg viewBox="0 0 100 50"></svg>'));
    expect(geometry).toEqual({
      panel: { width: 100, height: 50 },
      tile: { width: 0, height: 0 },
      rects: [],
    });
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

describe("the rim and the corner of a cutout", () => {
  const keypad = drawing(KEYPAD);

  it("reads the corner radius and the rim stroke of the key cutouts", () => {
    expect(keypad.corner).toBe(23);
    expect(keypad.stroke).toBe(2);
  });

  it("insets each tile by half the rim stroke, so the rim stays whole", () => {
    const place = nth(placement(keypad, fixtureGeometry()) ?? [], 0);
    expect(place.left).toBeCloseTo((58 / 512) * 100, 3);
    expect(place.top).toBeCloseTo((61 / 610) * 100, 3);
    expect(place.width).toBeCloseTo((115 / 512) * 100, 3);
    expect(place.height).toBeCloseTo((115 / 610) * 100, 3);
  });

  it("carries the corner inside the rim as a percent of the tile", () => {
    const place = nth(placement(keypad, fixtureGeometry()) ?? [], 4);
    expect(place.corner).toBeCloseTo((22 / 115) * 100, 3);
  });

  it("names the corner as --kv-tile-radius in the place style, the plain panel none", () => {
    const fitted = styles(placement(keypad, fixtureGeometry()) ?? []);
    expect(fitted.get(1)).toMatch(/; --kv-tile-radius: 19\.13\d*%$/);
    const plain = styles(panelPlacement(fixtureGeometry()));
    expect(plain.get(1)).not.toContain("--kv-tile-radius");
  });
});
