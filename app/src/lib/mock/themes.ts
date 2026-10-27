// The mock's token sets (docs/design/gui-ux.md §7, "Measured token sets"):
// the --kv-* properties as the theme bridge (M4a-1) would read them from
// the Breeze colour schemes and the libadwaita defaults, so a layout can be
// judged in light and dark on the dev server. `?theme=` picks one. The
// applied sets follow the bridge's rules (§7, "Derived muted text and
// ring"): the muted text and the accent are derived where the measured
// ones fall short of 4.5:1 and 3:1 on both surfaces.

export type ThemeName = "breeze-dark" | "breeze-light" | "adwaita-dark" | "adwaita-light";

export const DEFAULT_THEME: ThemeName = "breeze-dark";

/** The tokens that do not come from the desktop theme: the derived
 * indicators keep the pre-alpha defaults until M4a-1 defines their
 * derivation, the feedback states are the pad theme's, the device colour
 * is the graphite edition's. */
const SHARED = {
  "--kv-sandbox": "#b48ef0",
  "--kv-project": "#3cbfae",
  "--kv-state-running": "#f08c00",
  "--kv-state-success": "#2f9e44",
  "--kv-state-error": "#e03131",
  "--kv-contrast": "normal",
  "--kv-device": "#2f3133",
};

const BREEZE = {
  ...SHARED,
  "--kv-accent": "#3daee9",
  "--kv-font": '"Noto Sans", sans-serif',
  "--kv-font-size": "10pt",
  "--kv-radius": "5px",
};

const ADWAITA = {
  ...SHARED,
  "--kv-font": '"Adwaita Sans", sans-serif',
  "--kv-font-size": "11pt",
  "--kv-radius": "9px",
};

/** The sets as measured from the desktop sources, before the rules. */
export const MEASURED: Record<ThemeName, Record<string, string>> = {
  "breeze-dark": {
    ...BREEZE,
    "color-scheme": "dark",
    "--kv-bg": "#202326",
    "--kv-bg-alt": "#292c30",
    "--kv-fg": "#fcfcfc",
    "--kv-fg-muted": "#a1a9b1",
  },
  "breeze-light": {
    ...BREEZE,
    "color-scheme": "light",
    "--kv-bg": "#eff0f1",
    "--kv-bg-alt": "#e3e5e7",
    "--kv-fg": "#232629",
    "--kv-fg-muted": "#707d8a",
  },
  "adwaita-dark": {
    ...ADWAITA,
    "color-scheme": "dark",
    "--kv-bg": "#222226",
    "--kv-bg-alt": "#2e2e32",
    "--kv-fg": "#ffffff",
    "--kv-fg-muted": "#9c9c9d",
    "--kv-accent": "#81d0ff",
  },
  "adwaita-light": {
    ...ADWAITA,
    "color-scheme": "light",
    "--kv-bg": "#fafafb",
    "--kv-bg-alt": "#ebebed",
    "--kv-fg": "#323237",
    "--kv-fg-muted": "#8c8c8f",
    "--kv-accent": "#0461be",
  },
};

/** The muted text needs 4.5:1, the focus ring 3:1 (WCAG 2.2, 1.4.3 and
 * 1.4.11), on both `--kv-bg` and `--kv-bg-alt`. */
const TEXT = 4.5;
const RING = 3;

function channels(hex: string): number[] {
  return [1, 3, 5].map((at) => Number.parseInt(hex.slice(at, at + 2), 16) / 255);
}

function toLinear(channel: number): number {
  return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
}

function toGamma(channel: number): number {
  return channel <= 0.0031308 ? channel * 12.92 : 1.055 * channel ** (1 / 2.4) - 0.055;
}

/** WCAG 2 relative luminance of `#rrggbb`. */
function luminance(hex: string): number {
  const [r = 0, g = 0, b = 0] = channels(hex).map(toLinear);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG 2 contrast ratio of two `#rrggbb` colours, in either order. */
export function contrast(a: string, b: string): number {
  const [light = 0, dark = 0] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (light + 0.05) / (dark + 0.05);
}

/** Below this chroma a colour is grey and its hue is missing, as CSS
 * treats a powerless hue in color-mix(). */
const GREY = 1e-4;

type Oklch = [lightness: number, chroma: number, hue: number];

/** `#rrggbb` as OKLCH, the hue in degrees or NaN for a grey (Ottosson's
 * OKLab matrices). */
function oklch(hex: string): Oklch {
  const [r = 0, g = 0, b = 0] = channels(hex).map(toLinear);
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  const lightness = 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s;
  const green = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s;
  const blue = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
  const chroma = Math.hypot(green, blue);
  const hue = chroma < GREY ? Number.NaN : (Math.atan2(blue, green) * 180) / Math.PI;
  return [lightness, chroma, hue];
}

/** OKLCH back to `#rrggbb`, each sRGB channel clipped to the gamut. */
function hexOf([lightness, chroma, hue]: Oklch): string {
  const radians = ((Number.isNaN(hue) ? 0 : hue) * Math.PI) / 180;
  const green = chroma * Math.cos(radians);
  const blue = chroma * Math.sin(radians);
  const l = (lightness + 0.3963377774 * green + 0.2158037573 * blue) ** 3;
  const m = (lightness - 0.1055613458 * green - 0.0638541728 * blue) ** 3;
  const s = (lightness - 0.0894841775 * green - 1.291485548 * blue) ** 3;
  return `#${[
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ]
    .map((channel) => Math.round(Math.min(1, Math.max(0, toGamma(channel))) * 255))
    .map((channel) => channel.toString(16).padStart(2, "0"))
    .join("")}`;
}

/** `from` mixed `percent` % toward `to` in OKLCH along the shorter hue
 * arc, as `color-mix(in oklch, from, to percent%)`. */
export function mix(from: string, to: string, percent: number): string {
  const t = percent / 100;
  const [l1, c1, h1] = oklch(from);
  const [l2, c2, h2] = oklch(to);
  const start = Number.isNaN(h1) ? h2 : h1;
  const end = Number.isNaN(h2) ? h1 : h2;
  const turn = ((((end - start) % 360) + 540) % 360) - 180;
  return hexOf([l1 + (l2 - l1) * t, c1 + (c2 - c1) * t, start + turn * t]);
}

const PERCENTS = [...Array(101).keys()];

function passes(colour: string, set: Record<string, string>, ratio: number): boolean {
  return (
    contrast(colour, set["--kv-bg"] ?? "") >= ratio &&
    contrast(colour, set["--kv-bg-alt"] ?? "") >= ratio
  );
}

/** The muted rule: the desktop's inactive text where it passes 4.5:1 on
 * both surfaces, else the text mixed toward the window at the largest
 * whole percent that still passes. */
export function mutedOf(set: Record<string, string>): string {
  const measured = set["--kv-fg-muted"] ?? "";
  if (passes(measured, set, TEXT)) return measured;
  const fg = set["--kv-fg"] ?? "";
  const mixes = PERCENTS.map((percent) => mix(fg, set["--kv-bg"] ?? "", percent));
  return mixes.filter((colour) => passes(colour, set, TEXT)).pop() ?? fg;
}

/** The ring rule: the desktop's accent where it passes 3:1 on both
 * surfaces, else the accent mixed toward the text at the smallest whole
 * percent that passes. */
export function accentOf(set: Record<string, string>): string {
  const measured = set["--kv-accent"] ?? "";
  if (passes(measured, set, RING)) return measured;
  const fg = set["--kv-fg"] ?? "";
  const mixes = PERCENTS.map((percent) => mix(measured, fg, percent));
  return mixes.find((colour) => passes(colour, set, RING)) ?? fg;
}

/** The sets the mock applies: the measured ones under the two rules,
 * computed here and not with color-mix() so a test can measure them. */
export const THEMES: Record<ThemeName, Record<string, string>> = {
  "breeze-dark": applied(MEASURED["breeze-dark"]),
  "breeze-light": applied(MEASURED["breeze-light"]),
  "adwaita-dark": applied(MEASURED["adwaita-dark"]),
  "adwaita-light": applied(MEASURED["adwaita-light"]),
};

function applied(set: Record<string, string>): Record<string, string> {
  return { ...set, "--kv-fg-muted": mutedOf(set), "--kv-accent": accentOf(set) };
}

function isTheme(name: string | null): name is ThemeName {
  return name !== null && Object.hasOwn(THEMES, name);
}

/** The theme a page's query string names, else the default. */
export function themeOf(search: string): ThemeName {
  const name = new URLSearchParams(search).get("theme");
  return isTheme(name) ? name : DEFAULT_THEME;
}

function systemReducesMotion(): boolean {
  return globalThis.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
}

/** Sets the tokens of `name` on `root`; `--kv-motion` follows the system,
 * as the reduced-motion rule of tokens.css does. */
export function applyTheme(
  name: ThemeName,
  root: HTMLElement,
  reducesMotion: () => boolean = systemReducesMotion,
): void {
  for (const [property, value] of Object.entries(THEMES[name])) {
    root.style.setProperty(property, value);
  }
  root.style.setProperty("--kv-motion", reducesMotion() ? "reduced" : "full");
}
