// The one notice (docs/DESIGN.md "Notices", critique P1): a 1 px full
// border in the state colour, a leading word, the body in --kv-fg; an
// error is an alert, a note is not; no side stripe. jsdom applies no
// component styles, so the colours are read from the component's rules.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { render, screen } from "@testing-library/svelte";
import { createRawSnippet } from "svelte";
import { describe, expect, it } from "vitest";

import Notice from "./Notice.svelte";

const SOURCE = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "Notice.svelte"), "utf8");
const CSS = /<style>([\s\S]*?)<\/style>/.exec(SOURCE)?.[1] ?? "";

/** The declarations of the rule whose selector is exactly `selector`. */
function rule(selector: string): Map<string, string> {
  const found = [...CSS.matchAll(/([^{}]+)\{([^{}]*)\}/g)].find(
    (match) => (match[1] ?? "").trim() === selector,
  );
  return new Map(
    (found?.[2] ?? "")
      .split(";")
      .map((declaration) => declaration.split(":"))
      .filter((parts) => parts.length >= 2)
      .map(([name, ...value]) => [(name ?? "").trim(), value.join(":").trim()]),
  );
}

function body(text: string) {
  return createRawSnippet(() => ({ render: () => `<span>${text}</span>` }));
}

function notice(kind: "error" | "note", text: string): HTMLElement {
  const { container } = render(Notice, { props: { kind, children: body(text) } });
  const element = container.querySelector(".notice");
  if (!(element instanceof HTMLElement)) throw new Error("no notice rendered");
  return element;
}

describe("Notice", () => {
  it("leads an error with the word Error and announces it as an alert", () => {
    const element = notice("error", "Daemon unreachable: refused");
    expect(screen.getByRole("alert")).toBe(element);
    expect(element).toHaveTextContent(/^Error Daemon unreachable: refused$/);
    expect(element).toHaveClass("error");
  });

  it("leads a note with the word Note and is no alert", () => {
    const element = notice("note", "The Keypad is missing.");
    expect(screen.queryByRole("alert")).toBeNull();
    expect(element).toHaveTextContent(/^Note The Keypad is missing\.$/);
    expect(element).toHaveClass("note");
  });

  it("draws a 1 px full border, the body in --kv-fg on the panel surface", () => {
    const base = rule(".notice");
    expect(base.get("border")).toBe("1px solid var(--kv-fg-muted)");
    expect(base.get("color")).toBe("var(--kv-fg)");
    expect(base.get("background")).toBe("var(--kv-bg-alt)");
    expect(base.get("border-radius")).toBe("var(--kv-radius)");
  });

  it("takes the error's border colour from the state token", () => {
    expect(rule(".error").get("border-color")).toBe("var(--kv-state-error)");
  });

  it("has no side stripe", () => {
    expect(CSS).not.toMatch(/border-(left|right|inline-start|inline-end)/);
  });
});
