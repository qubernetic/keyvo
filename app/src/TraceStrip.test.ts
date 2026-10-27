// The trace strip under the inspector (pad screen brief §5.2, decision 6 of
// #108): one line with the newest entries first and their 24 h times, each
// opening the drawer on itself; the chevron expands the strip in place to
// the entries the Pad keeps.
import { fireEvent, render, screen, within } from "@testing-library/svelte";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it, vi } from "vitest";

import TraceStrip from "./TraceStrip.svelte";
import { eventsOf, nth } from "./lib/fixtures";
import { t } from "./lib/i18n";
import { PAD_TRACE, STRIP_LINE } from "./lib/pad";
import type { TraceRecord } from "./lib/protocol";

/** The fixture entries, newest first: chord sent, exec exited 101, exec
 * failed, dry run, unbound. */
const FIXTURE = eventsOf("trace").map((event) => event.entry);

/** `count` entries one second apart from 15:04:05 local time on, newest first. */
function entries(count: number): TraceRecord[] {
  const start = new Date(2026, 9, 26, 15, 4, 5).getTime();
  return Array.from({ length: count }, (_, index) => ({
    ...nth(FIXTURE, index % FIXTURE.length),
    key: (index % 9) + 1,
    timestamp_ms: start + (count - 1 - index) * 1000,
  }));
}

function strip(list: TraceRecord[], props: { connecting?: boolean } = {}) {
  const onopen = vi.fn<(entry: TraceRecord) => void>();
  render(TraceStrip, { props: { entries: list, onopen, ...props } });
  return { onopen, region: screen.getByRole("region", { name: t("trace.strip") }) };
}

function items(region: HTMLElement): HTMLElement[] {
  return within(region).queryAllByRole("listitem");
}

describe("the trace strip", () => {
  it("shows one line of the newest entries first, each with its 24 h time", () => {
    const list = entries(5);
    const { region } = strip(list);
    const shown = items(region);
    expect(shown).toHaveLength(STRIP_LINE);
    expect(within(nth(shown, 0)).getByText("15:04:09", { selector: "time" })).toBeInTheDocument();
    expect(within(nth(shown, 1)).getByText("15:04:08", { selector: "time" })).toBeInTheDocument();
    expect(nth(shown, 0)).toHaveTextContent(/Key 1, app layer: chord ctrl\+p, sent/);
  });

  it("reads the newest entry in full and lets the older ones give way", () => {
    const { region } = strip(entries(5));
    const shown = items(region);
    expect(nth(shown, 0)).toHaveClass("newest");
    expect(nth(shown, 1)).not.toHaveClass("newest");
    // The newest never shrinks below its text; the others share what is
    // left and shrink to nothing before it loses a character.
    const css = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "TraceStrip.svelte"), "utf8");
    const newest = /\.strip:not\(\.expanded\) li\.newest \{([^}]*)\}/.exec(css)?.[1] ?? "";
    expect(newest).toMatch(/flex: 0 1 auto/);
    expect(newest).toMatch(/max-width: 100%/);
    const older = /\.strip:not\(\.expanded\) li:not\(\.newest\) \{([^}]*)\}/.exec(css)?.[1] ?? "";
    expect(older).toMatch(/flex: 1 1 0/);
    expect(older).toMatch(/min-width: 0/);
    // An older entry squeezed to nothing clips its time instead of letting
    // it run past the strip (and past the window at 960 px).
    expect(older).toMatch(/overflow: hidden/);
  });

  it("opens the drawer on the entry that was clicked", async () => {
    const list = entries(5);
    const { region, onopen } = strip(list);
    await fireEvent.click(within(nth(items(region), 1)).getByRole("button"));
    expect(onopen).toHaveBeenCalledWith(nth(list, 1));
    expect(within(nth(items(region), 1)).getByRole("button")).toHaveAccessibleName(
      /^15:04:08 Key 2, app layer: exec cargo build, exited 101 after 2318 ms, open the trace entry$/,
    );
  });

  it("expands in place to every entry the Pad keeps and collapses again", async () => {
    const { region } = strip(entries(PAD_TRACE));
    const chevron = within(region).getByRole("button", { name: t("trace.expand") });
    expect(chevron).toHaveAttribute("aria-expanded", "false");
    await fireEvent.click(chevron);
    expect(items(region)).toHaveLength(PAD_TRACE);
    const collapse = within(region).getByRole("button", { name: t("trace.collapse") });
    expect(collapse).toHaveAttribute("aria-expanded", "true");
    await fireEvent.click(collapse);
    expect(items(region)).toHaveLength(STRIP_LINE);
  });

  it("has no chevron while the line shows every entry", () => {
    const { region } = strip(entries(2));
    expect(items(region)).toHaveLength(2);
    expect(within(region).queryByRole("button", { name: t("trace.expand") })).toBeNull();
  });

  it("says how to fill it while it is empty, and nothing while connecting", async () => {
    const { region } = strip([]);
    expect(region).toHaveTextContent(t("home.noEvents"));
    document.body.innerHTML = "";
    strip([], { connecting: true });
    expect(screen.queryByText(t("home.noEvents"))).toBeNull();
  });
});
