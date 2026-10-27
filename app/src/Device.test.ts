// The Keypad drawing (docs/design/gui-ux.md §3.1) against the geometry of
// the shared socket fixtures: the tiles snippet gets one call per key with
// the key and the percent style of its cutout, the drawing scales up to
// the panel's pixel width, a geometry the drawing does not fit falls back
// to the plain panel, and the colours derive from --kv-device.
import { render, screen, within } from "@testing-library/svelte";
import { createRawSnippet, type Snippet } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";

import Device from "./Device.svelte";
import { nth, responses } from "./lib/fixtures";
import { t } from "./lib/i18n";
import type { Geometry, Status } from "./lib/protocol";
import DeviceRows from "./testing/DeviceRows.svelte";

const geometry = (responses()[0] as Status).device.geometry as Geometry;
const twelve: Geometry = { ...geometry, rects: [...geometry.rects, ...geometry.rects.slice(0, 3)] };

/** The Keypad's cutouts (src/assets/device/keypad.svg, 512 by 610), key 1 first. */
const CUTOUTS = [60, 197, 334].flatMap((y) => [57, 194, 331].map((x) => ({ x, y })));

/** A drawing with four cutouts, which the nine keys do not fit. */
const FOUR_KEYS = `<svg viewBox="0 0 100 100">${[1, 2, 3, 4]
  .map((key) => `<rect id="key-${key}" x="${key * 20}" y="10" width="15" height="15"/>`)
  .join("")}</svg>`;

interface Call {
  key: number;
  style: string;
}

/** A tiles snippet that records its calls and draws a cell with the style. */
function recorder(): { calls: Call[]; tile: Snippet<[number, string]> } {
  const calls: Call[] = [];
  const tile = createRawSnippet((key: () => number, style: () => string) => {
    calls.push({ key: key(), style: style() });
    return { render: () => `<div data-key="${key()}" style="${style()}"></div>` };
  });
  return { calls, tile };
}

/** The percent value of `property` in an inline `style`. */
function percent(style: string, property: string): number {
  const value = new RegExp(`(?:^|;)\\s*${property}:\\s*([\\d.]+)%`).exec(style)?.[1];
  return value === undefined ? Number.NaN : Number(value);
}

function root(container: HTMLElement): HTMLElement {
  return container.querySelector(".device") as HTMLElement;
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("the drawing", () => {
  it("hands the snippet nine calls with the key and its cutout in percent", () => {
    const { calls, tile } = recorder();
    render(Device, { geometry, tile });
    expect(calls.map((call) => call.key)).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9]);
    calls.forEach((call, index) => {
      const cutout = nth(CUTOUTS, index);
      expect(Math.abs(percent(call.style, "left") - (cutout.x / 512) * 100)).toBeLessThan(0.5);
      expect(Math.abs(percent(call.style, "top") - (cutout.y / 610) * 100)).toBeLessThan(0.5);
      expect(Math.abs(percent(call.style, "width") - (117 / 512) * 100)).toBeLessThan(0.5);
      expect(Math.abs(percent(call.style, "height") - (117 / 610) * 100)).toBeLessThan(0.5);
    });
  });

  it("places the snippet's nine calls in its own cutouts without a geometry", () => {
    const { calls, tile } = recorder();
    const { container } = render(Device, { tile });
    expect(calls.map((call) => call.key)).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9]);
    calls.forEach((call, index) => {
      const cutout = nth(CUTOUTS, index);
      expect(Math.abs(percent(call.style, "left") - (cutout.x / 512) * 100)).toBeLessThan(0.5);
      expect(Math.abs(percent(call.style, "top") - (cutout.y / 610) * 100)).toBeLessThan(0.5);
    });
    expect(root(container).style.aspectRatio).toMatch(/^512 ?\/ ?610$/);
    expect(screen.getByRole("img", { name: t("device.keypad") })).toBeInTheDocument();
  });

  it("wraps the cells in the caller's rows and still places them by key", () => {
    render(DeviceRows, { geometry });
    const rows = screen.getAllByRole("row");
    expect(rows.map((row) => within(row).getAllByRole("gridcell").length)).toEqual([3, 3, 3]);
    const fifth = nth(within(nth(rows, 1)).getAllByRole("gridcell"), 1);
    expect(fifth.dataset.key).toBe("5");
    expect(
      Math.abs(percent(fifth.getAttribute("style") ?? "", "left") - (194 / 512) * 100),
    ).toBeLessThan(0.5);
  });

  it("is the Keypad, titled from the strings", () => {
    const { tile } = recorder();
    render(Device, { geometry, tile });
    expect(screen.getByRole("img", { name: t("device.keypad") })).toBeInTheDocument();
    expect(document.querySelector("svg title")).toHaveTextContent(t("device.keypad"));
  });

  // jsdom lays nothing out, so the scaling is checked on the rules that do
  // it: all of a 300 px container, 480 px (the panel, 1:1) of a 600 px one.
  it.each([300, 600])(
    "takes min(%i px, the panel's 480 px) of its container, in the device's aspect",
    (width) => {
      const host = document.body.appendChild(document.createElement("div"));
      host.style.width = `${width}px`;
      const { tile } = recorder();
      const { container } = render(Device, { target: host, props: { geometry, tile } });
      expect(root(container).parentElement).toBe(host);
      expect(root(container).style.width).toBe("100%");
      expect(root(container).style.maxWidth).toBe("480px");
      expect(root(container).style.aspectRatio).toMatch(/^512 ?\/ ?610$/);
    },
  );

  it("gives two instances distinct gradient ids", () => {
    const { tile } = recorder();
    render(Device, { geometry, tile });
    render(Device, { geometry, tile });
    const ids = [...document.querySelectorAll("linearGradient")].map((gradient) => gradient.id);
    expect(ids).toHaveLength(4);
    expect(new Set(ids).size).toBe(4);
    for (const svg of document.querySelectorAll("svg")) {
      for (const filled of svg.querySelectorAll("[fill^='url(']")) {
        const id = /url\(#([^)]+)\)/.exec(filled.getAttribute("fill") ?? "")?.[1] ?? "";
        expect(svg.querySelector(`linearGradient[id="${id}"]`), id).not.toBeNull();
      }
    }
  });

  it("derives the drawing's colours from --kv-device on its root", () => {
    const { tile } = recorder();
    const { container } = render(Device, { geometry, tile });
    const style = root(container).style;
    for (const part of ["light", "shade", "button", "metal", "well", "rim", "edge", "glyph"]) {
      expect(style.getPropertyValue(`--kv-device-${part}`), part).toMatch(
        /^color-mix\(in oklab, var\(--kv-device(-shade|-light)?\)/,
      );
    }
    expect(style.getPropertyValue("--kv-device-light")).toContain("var(--kv-device)");
    expect(style.getPropertyValue("--kv-device-well")).toContain("var(--kv-device)");
    expect(style.getPropertyValue("--kv-device-rim")).toContain("var(--kv-device-shade)");
    expect(style.getPropertyValue("--kv-device-edge")).toContain("var(--kv-device-shade)");
    expect(style.getPropertyValue("--kv-device-glyph")).toContain("var(--kv-device-light)");
  });
});

describe("the fallback panel", () => {
  it("hands the snippet nine calls with the key and its rect in percent of the panel", () => {
    vi.spyOn(console, "warn").mockImplementation(() => {});
    const { calls, tile } = recorder();
    const { container } = render(Device, { geometry, tile, svg: FOUR_KEYS });
    expect(container.querySelector("svg")).toBeNull();
    expect(calls.map((call) => call.key)).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9]);
    calls.forEach((call, index) => {
      const rect = nth(geometry.rects, index);
      expect(Math.abs(percent(call.style, "left") - (rect.x / 480) * 100)).toBeLessThan(0.5);
      expect(Math.abs(percent(call.style, "top") - (rect.y / 480) * 100)).toBeLessThan(0.5);
      expect(Math.abs(percent(call.style, "width") - (rect.width / 480) * 100)).toBeLessThan(0.5);
      expect(Math.abs(percent(call.style, "height") - (rect.height / 480) * 100)).toBeLessThan(
        0.5,
      );
    });
  });

  it("wraps the cells in the caller's rows too", () => {
    vi.spyOn(console, "warn").mockImplementation(() => {});
    render(DeviceRows, { geometry, svg: FOUR_KEYS });
    const rows = screen.getAllByRole("row");
    expect(rows.map((row) => within(row).getAllByRole("gridcell").length)).toEqual([3, 3, 3]);
  });

  it("takes over from the Keypad for a twelve-rect geometry, logging once", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const { calls, tile } = recorder();
    const { container } = render(Device, { geometry: twelve, tile });
    render(Device, { geometry: twelve, tile });
    expect(container.querySelector("svg")).toBeNull();
    expect(warn).toHaveBeenCalledTimes(1);
    expect(calls.slice(0, 12).map((call) => call.key)).toEqual([
      1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
    ]);
    expect(root(container).style.width).toBe("100%");
    expect(root(container).style.maxWidth).toBe("480px");
    expect(root(container).style.aspectRatio).toMatch(/^480 ?\/ ?480$/);
  });
});
