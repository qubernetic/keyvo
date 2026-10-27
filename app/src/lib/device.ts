// The device drawings of src/assets/device/ (docs/design/gui-ux.md §3.1):
// where their key cutouts are, where the mirror tiles go (in the cutouts
// when the drawing has one per key of the daemon's geometry, else on the
// plain proportional panel), and the drawing ready to inline once per
// instance, its ids prefixed so several instances share a page.

import type { Geometry, Rect } from "./protocol";

/** The drawings' colours, derived from the one `--kv-device` token (gui-ux
 * §7): light and shade around it, the well darker, the rim and the edge
 * from the shade, the glyph from the light. Measured against the
 * drawings' own fallbacks for graphite. The component sets them on its
 * root; the white and black here are mixing ends, not theme colours. */
export const DEVICE_COLOURS: Readonly<Record<string, string>> = {
  "--kv-device-light": "color-mix(in oklab, var(--kv-device), white 10%)",
  "--kv-device-shade": "color-mix(in oklab, var(--kv-device), black 22%)",
  "--kv-device-button": "color-mix(in oklab, var(--kv-device), white 12%)",
  "--kv-device-metal": "color-mix(in oklab, var(--kv-device), white 25%)",
  "--kv-device-well": "color-mix(in oklab, var(--kv-device), black 50%)",
  "--kv-device-rim": "color-mix(in oklab, var(--kv-device-shade), white 35%)",
  "--kv-device-edge": "color-mix(in oklab, var(--kv-device-shade), black 40%)",
  "--kv-device-glyph": "color-mix(in oklab, var(--kv-device-light), white 30%)",
};

/** A drawing: its view box, its `key-N` cutouts, key 1 first, the corner
 * radius of the cutouts and the stroke of their rim, in view box units
 * (0 when the drawing has none). */
export interface Drawing {
  width: number;
  height: number;
  cutouts: Rect[];
  corner: number;
  stroke: number;
}

/** Where one key's tile goes, in percent of the drawing or the panel, and
 * the tile's corner radius in percent of its own width, when the drawing
 * gives one. */
export interface Placement {
  key: number;
  left: number;
  top: number;
  width: number;
  height: number;
  corner?: number;
}

function attribute(tag: string, name: string): number {
  const value = new RegExp(`\\s${name}="([-\\d.]+)"`).exec(tag)?.[1];
  return value === undefined ? Number.NaN : Number(value);
}

/** The view box, the cutouts, their corner and their rim of the drawing
 * `svg`: the corner is the `rx` of the key rects, the rim the
 * `stroke-width` of the `keys` group that holds them. */
export function drawing(svg: string): Drawing {
  const box = /<svg\b[^>]*\sviewBox="([^"]+)"/.exec(svg)?.[1]?.trim().split(/[\s,]+/).map(Number);
  const keys = [...svg.matchAll(/<rect\b[^>]*\sid="key-(\d+)"[^>]*>/g)];
  const group = /<g\b[^>]*\sid="keys"[^>]*>/.exec(svg)?.[0] ?? "";
  const cutouts = keys
    .map((match) => ({
      key: Number(match[1]),
      rect: {
        x: attribute(match[0], "x"),
        y: attribute(match[0], "y"),
        width: attribute(match[0], "width"),
        height: attribute(match[0], "height"),
      },
    }))
    .sort((a, b) => a.key - b.key)
    .map((cutout) => cutout.rect);
  const corner = keys[0] === undefined ? 0 : attribute(keys[0][0], "rx");
  const stroke = attribute(group, "stroke-width");
  return {
    width: box?.[2] ?? Number.NaN,
    height: box?.[3] ?? Number.NaN,
    cutouts,
    corner: Number.isNaN(corner) ? 0 : corner,
    stroke: Number.isNaN(stroke) ? 0 : stroke,
  };
}

/** `rect` in percent of a `width` by `height` surface. */
function percent(key: number, rect: Rect, width: number, height: number): Placement {
  return {
    key,
    left: (rect.x / width) * 100,
    top: (rect.y / height) * 100,
    width: (rect.width / width) * 100,
    height: (rect.height / height) * 100,
  };
}

/** The tiles of `geometry` in the cutouts of `device`, by index, each
 * inset by half the rim stroke so the rim stays whole around it, with the
 * corner the rim leaves inside; undefined when the two differ in count and
 * the tiles belong on the plain panel. */
export function placement(device: Drawing, geometry: Geometry): Placement[] | undefined {
  if (device.cutouts.length !== geometry.rects.length) return undefined;
  const inset = device.stroke / 2;
  return device.cutouts.map((cutout, index) => {
    const inner = {
      x: cutout.x + inset,
      y: cutout.y + inset,
      width: cutout.width - device.stroke,
      height: cutout.height - device.stroke,
    };
    return {
      ...percent(index + 1, inner, device.width, device.height),
      corner: (Math.max(device.corner - inset, 0) / inner.width) * 100,
    };
  });
}

/** The tiles of `geometry` on the plain panel, at the daemon's rects. */
export function panelPlacement(geometry: Geometry): Placement[] {
  return geometry.rects.map((rect, index) =>
    percent(index + 1, rect, geometry.panel.width, geometry.panel.height),
  );
}

/** The mismatches `fallBack` has logged, so each is logged once per app
 * and not once per instance (the Profiles thumbnails). */
const logged = new Set<string>();

/** Logs, once, that a drawing with `cutouts` cutouts does not fit a
 * geometry of `keys` keys and the tiles go on the plain panel. */
export function fallBack(cutouts: number, keys: number): void {
  const mismatch = `${cutouts}/${keys}`;
  if (logged.has(mismatch)) return;
  logged.add(mismatch);
  console.warn(
    `keyvo: the device drawing has ${cutouts} key cutouts, the geometry ${keys} keys; the mirror uses the plain panel`,
  );
}

/** The tiles of `places` as inline styles, by key: absolute, in percent
 * of the nearest positioned ancestor, the component's tile layer, and the
 * corner as `--kv-tile-radius` for the tile's own rules to read. */
export function styles(places: readonly Placement[]): Map<number, string> {
  return new Map(
    places.map((place) => [
      place.key,
      `position: absolute; left: ${place.left}%; top: ${place.top}%; width: ${place.width}%; height: ${place.height}%` +
        (place.corner === undefined ? "" : `; --kv-tile-radius: ${place.corner}%`),
    ]),
  );
}

function escape(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

/** `svg` ready to inline: without its header comment, every id and every
 * `url(#...)` reference prefixed with `prefix`, titled `title`. */
export function inline(svg: string, prefix: string, title: string): string {
  return svg
    .slice(svg.indexOf("<svg"))
    .trim()
    .replace(/\sid="([^"]+)"/g, ` id="${prefix}-$1"`)
    .replace(/url\(#([^)]+)\)/g, `url(#${prefix}-$1)`)
    .replace(
      /^<svg\b([^>]*)>/,
      `<svg$1 role="img" aria-labelledby="${prefix}-title"><title id="${prefix}-title">${escape(title)}</title>`,
    );
}
