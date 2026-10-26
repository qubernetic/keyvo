// The mock's token sets (docs/design/gui-ux.md §7, "Measured token sets"):
// the --kv-* properties as the theme bridge (M4a-1) would read them from
// the Breeze colour schemes and the libadwaita defaults, so a layout can be
// judged in light and dark on the dev server. `?theme=` picks one.

export type ThemeName = "breeze-dark" | "breeze-light" | "adwaita-dark" | "adwaita-light";

export const DEFAULT_THEME: ThemeName = "breeze-dark";

/** The tokens that do not come from the desktop theme: the derived
 * indicators keep the pre-alpha defaults until M4a-1 defines their
 * derivation, the feedback states are the pad theme's. */
const SHARED = {
  "--kv-sandbox": "#b48ef0",
  "--kv-project": "#3cbfae",
  "--kv-state-running": "#f08c00",
  "--kv-state-success": "#2f9e44",
  "--kv-state-error": "#e03131",
  "--kv-contrast": "normal",
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

export const THEMES: Record<ThemeName, Record<string, string>> = {
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
