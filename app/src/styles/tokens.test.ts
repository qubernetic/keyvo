// The theme tokens of docs/design/gui-ux.md §7 and the rules that use them
// (§6): every token has a fixed default until the theme bridge (M4a-1)
// reads it from the desktop; every rule that colours text sets the text
// and its background together, from tokens only; every such pair passes
// 4.5:1; every form control, the option list included, has its pair; and
// the document's color-scheme matches the token set, so the webview's
// native popups follow it.
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const STYLES = dirname(fileURLToPath(import.meta.url));
const SRC = join(STYLES, "..");

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
  "--kv-device",
];

/** Every form control the app can show, with the selector its base rule
 * in controls.css uses. */
const CONTROLS = [
  "button",
  "input",
  "select",
  "textarea",
  "option",
  '[role="menu"]',
  '[role="menuitem"]',
  '[role="tooltip"]',
];

interface Rule {
  /** Where the rule is, for the failure message. */
  file: string;
  selector: string;
  declarations: Map<string, string>;
}

function read(path: string): string {
  return readFileSync(path, "utf8");
}

/** The innermost rules of `css`; an @media block yields the rules inside. */
function rules(file: string, css: string): Rule[] {
  const plain = css.replace(/\/\*[\s\S]*?\*\//g, "");
  return [...plain.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((match) => ({
    file,
    selector: (match[1] ?? "").trim(),
    declarations: new Map(
      (match[2] ?? "")
        .split(";")
        .map((declaration) => declaration.split(":"))
        .filter((parts) => parts.length >= 2)
        .map(([name, ...value]) => [(name ?? "").trim(), value.join(":").trim()]),
    ),
  }));
}

const sheets = readdirSync(STYLES)
  .filter((file) => file.endsWith(".css"))
  .map((file) => ({ file: `styles/${file}`, css: read(join(STYLES, file)) }));
const components = readdirSync(SRC)
  .filter((file) => file.endsWith(".svelte"))
  .map((file) => ({
    file,
    css: [...read(join(SRC, file)).matchAll(/<style>([\s\S]*?)<\/style>/g)]
      .map((match) => match[1] ?? "")
      .join("\n"),
  }));
const all = [...sheets, ...components].flatMap(({ file, css }) => rules(file, css));

const tokensCss = read(join(STYLES, "tokens.css"));
const root = rules("styles/tokens.css", tokensCss).find((rule) => rule.selector === ":root");

/** The default `token` gets on :root. */
function token(name: string): string | undefined {
  return root?.declarations.get(name);
}

/** The token a `var(--kv-...)` value names. */
function tokenOf(value: string | undefined): string | undefined {
  return /^var\((--kv-[a-z-]+)\)$/.exec(value ?? "")?.[1];
}

function background(rule: Rule): string | undefined {
  return rule.declarations.get("background") ?? rule.declarations.get("background-color");
}

/** WCAG 2 relative luminance of `#rrggbb`. */
function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((at) => {
    const channel = Number.parseInt(hex.slice(at, at + 2), 16) / 255;
    return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * (r ?? 0) + 0.7152 * (g ?? 0) + 0.0722 * (b ?? 0);
}

/** WCAG 2 contrast ratio of two tokens' colours. */
function contrast(a: string, b: string): number {
  const [light, dark] = [luminance(token(a) ?? ""), luminance(token(b) ?? "")].sort(
    (x, y) => y - x,
  );
  return ((light ?? 0) + 0.05) / ((dark ?? 0) + 0.05);
}

/** Rules outside the token definitions that set a colour. */
const coloured = all.filter(
  (rule) =>
    rule.selector !== ":root" &&
    (rule.declarations.has("color") || background(rule) !== undefined),
);

/** Every text and background token pair a rule sets. */
const pairs = coloured.map((rule) => ({
  rule,
  fg: tokenOf(rule.declarations.get("color")) ?? "",
  bg: tokenOf(background(rule)) ?? "",
}));

describe("the tokens", () => {
  it.each(TOKENS)("%s has a default on :root", (name) => {
    expect(token(name)).toBeTruthy();
  });

  it("the feedback states match the pad theme in keyvo-core", () => {
    // keyvo_core::render::Theme::default(): running, success, error.
    expect(token("--kv-state-running")).toBe("#f08c00");
    expect(token("--kv-state-success")).toBe("#2f9e44");
    expect(token("--kv-state-error")).toBe("#e03131");
  });

  it("the device drawing is graphite by default, the first of the three editions", () => {
    expect(token("--kv-device")).toBe("#2f3133");
  });

  it("reduced motion from the system turns --kv-motion to reduced", () => {
    expect(tokensCss).toMatch(
      /@media \(prefers-reduced-motion: reduce\)\s*\{\s*:root\s*\{\s*--kv-motion:\s*reduced;/,
    );
  });

  it("the color-scheme of the document matches the token set", () => {
    const dark = luminance(token("--kv-bg") ?? "") < luminance(token("--kv-fg") ?? "");
    expect(token("color-scheme")).toBe(dark ? "dark" : "light");
  });
});

describe("the rules", () => {
  it("finds the stylesheets and the components", () => {
    expect(sheets.map((sheet) => sheet.file)).toContain("styles/controls.css");
    expect(components.map((component) => component.file)).toContain("Pad.svelte");
  });

  it.each(CONTROLS)("%s has a base rule with its text and background tokens", (control) => {
    const base = all.find(
      (rule) =>
        rule.file === "styles/controls.css" &&
        rule.selector.split(",").some((selector) => selector.trim() === control),
    );
    expect(base, control).toBeDefined();
    expect(tokenOf(base?.declarations.get("color")), control).toBeDefined();
    expect(tokenOf(base === undefined ? undefined : background(base)), control).toBeDefined();
  });

  it("every rule that sets a colour sets the text and its background from tokens", () => {
    const bad = pairs
      .filter(({ fg, bg }) => fg === "" || bg === "")
      .map(({ rule }) => `${rule.file}: ${rule.selector}`);
    expect(bad).toEqual([]);
  });

  it("no rule draws a side stripe (DESIGN.md Don'ts: no side border over 1 px)", () => {
    const bad = all.flatMap((rule) =>
      [...rule.declarations]
        .filter(([name]) => /^border-(left|right|inline-start|inline-end)(-width)?$/.test(name))
        .filter(([, value]) => !/^(0|1px)\b/.test(value))
        .map(([name, value]) => `${rule.file}: ${rule.selector}: ${name}: ${value}`),
    );
    expect(bad).toEqual([]);
  });

  it("only Notice colours a border with a state token", () => {
    const bad = all
      .filter((rule) => rule.file !== "Notice.svelte")
      .flatMap((rule) =>
        [...rule.declarations]
          .filter(([name, value]) => name.startsWith("border") && value.includes("--kv-state-"))
          .map(([name, value]) => `${rule.file}: ${rule.selector}: ${name}: ${value}`),
      );
    expect(bad).toEqual([]);
  });

  it("no rule outside :root writes a literal colour", () => {
    const literal = /#[0-9a-fA-F]{3,8}\b|\b(rgba?|hsla?)\(|\b(white|black|red|green|blue|gray|grey)\b/;
    const bad = all
      .filter((rule) => rule.selector !== ":root")
      .flatMap((rule) =>
        [...rule.declarations.values()]
          .filter((value) => literal.test(value))
          .map((value) => `${rule.file}: ${rule.selector}: ${value}`),
      );
    expect(bad).toEqual([]);
  });
});

describe("contrast", () => {
  it("covers the control and the option list pairs", () => {
    const selectors = pairs.map(({ rule }) => rule.selector).join(",");
    for (const control of ["select", "option", "button", "input", '[role="tooltip"]']) {
      expect(selectors).toContain(control);
    }
  });

  it("every text pair passes 4.5:1", () => {
    const failing = pairs
      .filter(({ fg, bg }) => fg !== "" && bg !== "" && contrast(fg, bg) < 4.5)
      .map(({ rule, fg, bg }) => `${rule.file}: ${rule.selector}: ${fg} on ${bg}`);
    expect(pairs.length).toBeGreaterThan(CONTROLS.length);
    expect(failing).toEqual([]);
  });

  it("the focus ring passes 3:1 on both backgrounds", () => {
    expect(contrast("--kv-accent", "--kv-bg")).toBeGreaterThanOrEqual(3);
    expect(contrast("--kv-accent", "--kv-bg-alt")).toBeGreaterThanOrEqual(3);
  });
});

describe("hover", () => {
  const hovered = all.filter(
    (rule) => rule.file === "styles/controls.css" && rule.selector.includes(":hover"),
  );

  it.each([
    'button:where(:not(:disabled, [aria-disabled="true"]))',
    "input",
    "select",
    "textarea",
    '[role="menuitem"]',
  ])(
    "%s moves its border to the text colour on hover (DESIGN.md Hover)",
    (control) => {
      // The selector list is split by hand: a `:where(:not(a, b))` carries
      // commas of its own.
      const rule = hovered.find((candidate) =>
        candidate.selector.replace(/\s+/g, " ").includes(`${control}:hover`),
      );
      expect(rule, control).toBeDefined();
      expect(rule?.declarations.get("border-color")).toBe("var(--kv-fg)");
      expect([...(rule?.declarations.keys() ?? [])]).toEqual(["border-color"]);
    },
  );

  it("transitions the border colour only, in 150 ms, easing out", () => {
    const transitions = all
      .filter((rule) => !rule.selector.startsWith("*"))
      .flatMap((rule) => [...rule.declarations].filter(([name]) => name === "transition"));
    expect(transitions.length).toBeGreaterThan(0);
    for (const [, value] of transitions) {
      expect(value).toMatch(/^border-color 150ms cubic-bezier\(0\.22, 1, 0\.36, 1\)$/);
    }
  });
});

describe("targets and motion", () => {
  it("buttons, inputs and selects are at least 32 px high", () => {
    const sized = all.find(
      (rule) => rule.file === "styles/controls.css" && rule.selector.includes("button"),
    );
    expect(sized?.declarations.get("min-height")).toBe("32px");
    expect(sized?.selector).toMatch(/input/);
    expect(sized?.selector).toMatch(/select/);
  });

  it("reduced motion turns every transition and animation off", () => {
    const reduced = /@media \(prefers-reduced-motion: reduce\)\s*\{\s*\*[^{]*\{([^}]*)\}/.exec(
      read(join(STYLES, "controls.css")),
    );
    expect(reduced?.[1]).toMatch(/animation:\s*none/);
    expect(reduced?.[1]).toMatch(/transition:\s*none/);
  });
});
