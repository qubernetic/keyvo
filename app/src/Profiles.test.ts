// The Profiles screen as the profile manager (docs/design/gui-ux.md §3.2;
// pad screen brief §5.3, §6) against a fake daemon and fake profile files:
// the list by priority with the pad's profile marked, the header form and
// the shared steps of tests/protocol-fixtures/editor/profile.json (the file
// a step gives is the file keyvo_client::Editor writes for it,
// crates/keyvo-client/tests/editor.rs), the match rules, the page
// thumbnails, "Edit keys on the Pad", and the controls not built yet.
import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { isDeepStrictEqual } from "node:util";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it, vi } from "vitest";

import Profiles from "./Profiles.svelte";
import type { Daemon, Tile } from "./lib/daemon";
import { editorFile, nth, profileSteps, responses } from "./lib/fixtures";
import type { Pin } from "./lib/pin";
import type {
  ProfileEdited,
  ProfileFields,
  ProfileFiles,
  ProfileSummary,
  ProfileView,
} from "./lib/profiles";
import type { Geometry, Size, Status, View } from "./lib/protocol";
import { layout } from "./lib/shell";

const STATUS = responses()[0] as Status;
const GEOMETRY = STATUS.device.geometry as Geometry;
const STEPS = profileSteps();
const CONFIG = "/home/u/.config/keyvo";
const VIEW: View = { profile: STEPS.profile, page: 1, mode: "auto", index: 1 };

function summary(id: string, name: string, priority: number, extra: Partial<ProfileSummary> = {}) {
  return {
    id,
    name,
    priority,
    match: { class: [] },
    pages: ["context"],
    path: `${CONFIG}/profiles/${id}.toml`,
    file: true,
    ...extra,
  } satisfies ProfileSummary;
}

/** In id order, as the shell's `profile_list` answers. */
const LIST: ProfileSummary[] = [
  summary("claude", "Claude Code", 10, {
    accent: "#d97757",
    match: { class: ["konsole", "org.kde.konsole"], title: "(?i)claude" },
  }),
  summary("code", "VS Code", 1, { match: { class: ["code"] } }),
  summary("global", "Everywhere", 0, { file: false }),
  summary("notes", "Notes", 2, { pages: [] }),
  summary(STEPS.profile, "Terminal", 5, {
    accent: "#2ecc71",
    match: { class: ["org.kde.konsole"] },
    pages: ["context", "general", null],
  }),
];

function tiles(profile: string, page: number, keys: number): Tile[] {
  return Array.from({ length: keys }, (_, index) => ({
    image: `data:image/png;base64,${profile}-${page}-${index + 1}`,
  }));
}

function daemon(): Daemon {
  return {
    status: () => Promise.resolve(STATUS),
    tiles: vi.fn((profile: string, page: number, keys: number, _tile: Size) =>
      Promise.resolve(tiles(profile, page, keys)),
    ),
    outline: () => Promise.resolve({ pages: 3 }),
    recipes: () => Promise.resolve({ recipes: [] }),
    press: () => Promise.reject(new Error("not in this test")),
    subscribe: () => Promise.resolve(() => {}),
  };
}

/** Profile files whose terminal profile is the fixture's file: a header
 * write of the fields one step away moves the file to that step; anything
 * else is refused. */
function files(overrides: Partial<ProfileFiles> = {}) {
  const states = [STEPS.fields, ...STEPS.steps.map((step) => step.fields)];
  const texts = [STEPS.file, ...STEPS.steps.map((step) => step.file)];
  let at = 0;
  const path = `${CONFIG}/${STEPS.path}`;
  const fake = {
    list: vi.fn(() =>
      Promise.resolve(
        LIST.map((profile) =>
          profile.id === STEPS.profile ? { ...profile, ...nth(states, at) } : profile,
        ),
      ),
    ),
    profile: vi.fn(
      (id: string): Promise<ProfileView> =>
        id === STEPS.profile
          ? Promise.resolve({ path, fields: nth(states, at) })
          : Promise.resolve({
              path: `${CONFIG}/profiles/${id}.toml`,
              fields: { name: LIST.find((p) => p.id === id)?.name ?? id },
            }),
    ),
    editProfile: vi.fn((id: string, fields: ProfileFields): Promise<ProfileEdited> => {
      const next = states.findIndex((state) => isDeepStrictEqual(state, fields));
      if (id !== STEPS.profile || next === -1 || Math.abs(next - at) !== 1) {
        return Promise.reject({ message: `no step writes ${JSON.stringify(fields)}` });
      }
      const before = nth(states, at);
      at = next;
      return Promise.resolve({ path, before, fields, snapshot: "/s/1-terminal.toml" });
    }),
    open: vi.fn((_id: string) => Promise.resolve()),
    ...overrides,
  };
  return {
    files: fake satisfies ProfileFiles,
    /** The text of the terminal profile as the writes left it. */
    get file() {
      return editorFile(nth(texts, at));
    },
  };
}

function mount(
  options: {
    files?: ReturnType<typeof files>;
    geometry?: Geometry | null;
    view?: View;
    width?: number;
    pinned?: Pin;
  } = {},
) {
  const fake = options.files ?? files();
  // The pin goes back in as the shell's, the way App.svelte holds it.
  const onpin = vi.fn((next: Pin | undefined) => void rendered.rerender({ pinned: next }));
  const props = {
    daemon: daemon(),
    files: fake.files,
    columns: layout(options.width ?? 1200),
    view: options.view ?? VIEW,
    pinned: options.pinned,
    onpin,
    onedit: vi.fn((_pin: Pin) => {}),
    ...(options.geometry === null ? {} : { geometry: options.geometry ?? GEOMETRY }),
  };
  const rendered = render(Profiles, { props });
  return { ...props, fake, rendered };
}

function list(): HTMLElement {
  return screen.getByRole("list", { name: "Profiles" });
}

async function listed(): Promise<string[]> {
  await waitFor(() => expect(within(list()).getAllByRole("button")).toHaveLength(LIST.length));
  return within(list())
    .getAllByRole("button")
    .map((button) => button.getAttribute("aria-label") ?? "");
}

async function pick(name: RegExp) {
  await listed();
  await fireEvent.click(within(list()).getByRole("button", { name }));
}

function field(name: string): HTMLInputElement {
  return screen.getByRole("textbox", { name }) as HTMLInputElement;
}

async function type(input: HTMLElement, value: string) {
  await fireEvent.input(input, { target: { value } });
  await fireEvent.change(input);
}

function rules(): HTMLElement {
  return screen.getByRole("region", { name: "Match rules" });
}

function thumbnails(): HTMLElement {
  return screen.getByRole("list", { name: "Pages" });
}

/** The body of one rule of the component's style; jsdom applies none. */
function rule(selector: string): string {
  const css = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "Profiles.svelte"), "utf8");
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return new RegExp(`\\n  ${escaped} \\{([^}]*)\\}`).exec(css)?.[1] ?? "";
}

describe("the list column", () => {
  it("pins the global profile first, then orders by priority and marks the pad's one", async () => {
    mount();
    expect(await listed()).toEqual([
      "Everywhere (global), priority 0",
      "Claude Code (claude), priority 10",
      "Terminal (terminal), priority 5, on the pad",
      "Notes (notes), priority 2",
      "VS Code (code), priority 1",
    ]);
    const live = within(list()).getByRole("button", { name: /, on the pad$/ });
    expect(live).toHaveTextContent("on the pad");
    expect(within(list()).getByRole("button", { name: /^Everywhere/ })).not.toHaveTextContent(
      "on the pad",
    );
  });

  it("puts a square accent swatch after the name of a profile that has an accent", async () => {
    mount();
    await listed();
    const row = within(list()).getByRole("button", { name: /^Claude Code/ });
    const swatch = row.querySelector(".swatch");
    expect(swatch).toHaveStyle({ backgroundColor: "#d97757" });
    expect(swatch?.previousElementSibling).toHaveTextContent("Claude Code");
    expect(
      within(list()).getByRole("button", { name: /^VS Code/ }).querySelector(".swatch"),
    ).toBeNull();
  });

  it("selects the pad's profile first and another one on a click", async () => {
    const { fake } = mount();
    await listed();
    expect(within(list()).getByRole("button", { name: /^Terminal/ })).toHaveAttribute(
      "aria-current",
      "true",
    );
    await waitFor(() => expect(field("Name")).toHaveValue("Terminal"));
    await fireEvent.click(within(list()).getByRole("button", { name: /^VS Code/ }));
    expect(within(list()).getByRole("button", { name: /^VS Code/ })).toHaveAttribute(
      "aria-current",
      "true",
    );
    expect(screen.getByRole("heading", { level: 2, name: "VS Code" })).toBeInTheDocument();
    await waitFor(() => expect(field("Name")).toHaveValue("VS Code"));
    expect(fake.files.profile).toHaveBeenLastCalledWith("code");
  });

  it("follows the pad while nothing is pinned; a field taking the focus pins the pad's place", async () => {
    const { fake, onpin, rendered: view } = mount();
    await screen.findByRole("heading", { level: 2, name: "Terminal" });
    // The pad moves on: the selection follows it.
    await view.rerender({ view: { ...VIEW, profile: "claude", page: 2 } });
    await screen.findByRole("heading", { level: 2, name: "Claude Code" });
    expect(onpin).not.toHaveBeenCalled();
    // A field taking the focus is the user at work on this profile: it
    // pins it, so the header says so and the pad moving on changes nothing.
    await fireEvent.focusIn(field("Name"));
    expect(onpin).toHaveBeenLastCalledWith({ profile: "claude", page: 2 });
    await view.rerender({ view: VIEW });
    await waitFor(() => expect(fake.files.profile).toHaveBeenCalled());
    expect(screen.getByRole("heading", { level: 2, name: "Claude Code" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { level: 2, name: "Terminal" })).toBeNull();
    // Focus again on the pinned profile pins nothing new.
    await fireEvent.focusIn(field("Name"));
    expect(onpin).toHaveBeenCalledTimes(1);
  });

  it("shows the pinned profile, not the pad's, and keeps it while the pad moves", async () => {
    const { rendered: view } = mount({ pinned: { profile: "code", page: 1 } });
    await screen.findByRole("heading", { level: 2, name: "VS Code" });
    expect(within(list()).getByRole("button", { name: /^VS Code/ })).toHaveAttribute("aria-current", "true");
    await view.rerender({ view: { ...VIEW, profile: "claude" } });
    await waitFor(() => expect(within(list()).getByRole("button", { name: /, on the pad$/ })).toHaveAccessibleName(/^Claude Code/));
    expect(screen.getByRole("heading", { level: 2, name: "VS Code" })).toBeInTheDocument();
  });

  it("a pick in the list pins that profile, as the header dropdown does", async () => {
    const { onpin } = mount();
    await pick(/^Notes/);
    expect(onpin).toHaveBeenLastCalledWith({ profile: "notes", page: 1 });
    expect(screen.getByRole("heading", { level: 2, name: "Notes" })).toBeInTheDocument();
    // The pinned profile again keeps the pin as it is.
    await fireEvent.click(within(list()).getByRole("button", { name: /^Notes/ }));
    expect(onpin).toHaveBeenCalledTimes(1);
  });

  it("follows the pad again once the pin is gone (Follow in the header)", async () => {
    const { rendered: view } = mount({ pinned: { profile: "code", page: 1 } });
    await screen.findByRole("heading", { level: 2, name: "VS Code" });
    await view.rerender({ pinned: undefined });
    await screen.findByRole("heading", { level: 2, name: "Terminal" });
  });

  it("starts on the pinned page of the pinned profile", async () => {
    mount({ pinned: { profile: "terminal", page: 2 } });
    await waitFor(() => expect(within(thumbnails()).getAllByRole("button")).toHaveLength(3));
    expect(within(thumbnails()).getByRole("button", { name: "general" })).toHaveAttribute("aria-pressed", "true");
  });

  it("selects the global profile when the pad shows none", async () => {
    mount({ view: { profile: "gone", page: 1, mode: "auto", index: 1 } });
    await listed();
    expect(within(list()).getByRole("button", { name: /^Everywhere/ })).toHaveAttribute(
      "aria-current",
      "true",
    );
    expect(
      within(rules()).getByText("The global profile is under every window."),
    ).toBeInTheDocument();
  });

  it("shows skeleton rows while the first read runs", () => {
    mount({ files: files({ list: () => new Promise(() => {}) }) });
    expect(screen.getByRole("list", { name: "Loading the profiles" })).toHaveAttribute(
      "aria-busy",
      "true",
    );
  });

  it("shows why the profiles did not load", async () => {
    mount({ files: files({ list: () => Promise.reject(new Error("profiles/x.toml: bad")) }) });
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Profiles not loaded: profiles/x.toml: bad",
    );
  });

  it("keeps the list beside the editor from 1000 px and above it below", async () => {
    mount({ width: 960 });
    expect(await listed()).toHaveLength(LIST.length);
    expect(document.querySelector(".profiles-screen")).toHaveClass("stacked");
  });

  it("offers the templates behind + as controls not built yet", async () => {
    mount();
    await listed();
    const add = screen.getByRole("button", { name: "New profile" });
    expect(add).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("list", { name: "Templates" })).toBeNull();
    await fireEvent.click(add);
    expect(add).toHaveAttribute("aria-expanded", "true");
    const templates = screen.getByRole("list", { name: "Templates" });
    expect(
      within(templates)
        .getAllByRole("button")
        .map((button) => button.getAttribute("aria-label")),
    ).toEqual(["Empty profile", "Copy of the active profile", "Starter pack"]);
    await fireEvent.click(add);
    expect(screen.queryByRole("list", { name: "Templates" })).toBeNull();
  });
});

describe("the profile header", () => {
  it("names the profile, its file once and Open in editor in the heading row", async () => {
    mount();
    const heading = await screen.findByRole("heading", { level: 2, name: "Terminal" });
    const row = heading.parentElement as HTMLElement;
    expect(within(row).getByRole("button", { name: "Open in editor" })).toBeInTheDocument();
    await waitFor(() => expect(field("Name")).toHaveValue("Terminal"));
    expect(screen.getAllByText(`${CONFIG}/${STEPS.path}`)).toHaveLength(1);
    expect(screen.queryByText("File:")).toBeNull();
  });

  it("puts every label above its field and ties it to the field", async () => {
    mount();
    await waitFor(() => expect(field("Name")).toHaveValue("Terminal"));
    const form = document.querySelector("form.fields") as HTMLElement;
    for (const name of ["Name", "Accent", "Priority", "Enabled"]) {
      const label = within(form).getByText(name, { selector: "label" }) as HTMLLabelElement;
      const control = form.querySelector(`#${CSS.escape(label.htmlFor)}`);
      expect(control, name).not.toBeNull();
      expect(
        label.compareDocumentPosition(control as Node) & Node.DOCUMENT_POSITION_FOLLOWING,
        name,
      ).toBeTruthy();
      // Above, not beside: the label and its field share one column.
      expect(label.parentElement, name).toHaveClass("field");
      expect((control as HTMLElement).closest(".field"), name).toBe(label.parentElement);
    }
  });

  it("stacks the form in one column at 48ch with muted labels, as the Inspector does", () => {
    expect(rule(".fields")).toMatch(/flex-direction: column;/);
    expect(rule(".fields")).toMatch(/gap: 8px;/);
    expect(rule(".fields")).toMatch(/max-width: 48ch;/);
    expect(rule(".fields")).not.toMatch(/grid-template-columns/);
    expect(rule(".field")).toMatch(/flex-direction: column;/);
    expect(rule(".field")).toMatch(/gap: 2px;/);
    expect(rule(".field > label")).toMatch(/font-size: 0\.85em;/);
    expect(rule(".field > label")).toMatch(/color: var\(--kv-fg-muted\);/);
  });

  it("shows the priority with what it means and Enabled as a read value", async () => {
    mount();
    await waitFor(() => expect(field("Name")).toHaveValue("Terminal"));
    const form = document.querySelector("form.fields") as HTMLElement;
    expect(within(form).getByText("Priority").parentElement).toHaveTextContent(
      "Priority 5 higher wins the window",
    );
    // Every profile is on until the schema can switch one off.
    const enabled = within(form).getByText("Enabled").parentElement as HTMLElement;
    expect(enabled).toHaveTextContent(
      "Enabled On Switching a profile off is not built yet.",
    );
    expect(within(enabled).queryByRole("button")).toBeNull();
    expect(within(enabled).queryByRole("switch")).toBeNull();
  });

  it("keeps Open in editor and + rule at the end of the 48ch content", async () => {
    mount();
    await pick(/^Claude Code/);
    const open = screen.getByRole("button", { name: "Open in editor" });
    expect(open.parentElement).toHaveClass("row");
    const add = within(rules()).getByRole("button", { name: "Add a rule" });
    expect(add.closest(".row")?.querySelector("h3")).toHaveTextContent("Match rules");
    expect(rule(".row")).toMatch(/max-width: 48ch;/);
  });

  it("wraps a long name in the heading and keeps Open in editor on one line beside it", async () => {
    mount();
    await screen.findByRole("heading", { level: 2, name: "Terminal" });
    // A name without spaces breaks anywhere instead of pushing the button
    // out of the 48ch row, and the button never wraps into three lines.
    expect(rule(".heading h2")).toMatch(/min-width: 0;/);
    expect(rule(".heading h2")).toMatch(/overflow-wrap: anywhere;/);
    expect(rule(".heading > button")).toMatch(/flex: none;/);
    expect(rule(".heading > button")).toMatch(/white-space: nowrap;/);
  });

  it("shows the accent as a swatch beside its text", async () => {
    mount();
    await waitFor(() => expect(field("Accent")).toHaveValue("#2ecc71"));
    const swatch = field("Accent").parentElement?.querySelector(".swatch");
    expect(swatch).toHaveStyle({ backgroundColor: "#2ecc71" });
  });

  it("writes the shared steps byte for byte and says so under the header", async () => {
    const fake = files();
    mount({ files: fake });
    await waitFor(() => expect(field("Name")).toHaveValue(STEPS.fields.name));
    expect(field("Accent")).toHaveValue(STEPS.fields.accent);
    for (const step of STEPS.steps) {
      const value = step.field === "name" ? step.fields.name : (step.fields.accent ?? "");
      await type(field(step.field === "name" ? "Name" : "Accent"), value);
      await waitFor(() => expect(fake.file).toBe(editorFile(step.file)));
    }
    expect(fake.files.editProfile).toHaveBeenCalledTimes(STEPS.steps.length);
    // The note carries the time, as the inspector's does, so a second
    // write of the same value is seen to have happened.
    expect(screen.getByRole("status", { name: "Profile result" })).toHaveTextContent(
      new RegExp(`^Wrote the header of terminal to ${CONFIG}/${STEPS.path} at \\d\\d:\\d\\d:\\d\\d$`),
    );
    // The list follows the write.
    await waitFor(() =>
      expect(within(list()).getByRole("button", { name: /^Shell work/ })).toBeInTheDocument(),
    );
  });

  it("puts the message of an invalid field under it and writes nothing", async () => {
    const fake = files();
    mount({ files: fake });
    await waitFor(() => expect(field("Name")).toHaveValue("Terminal"));
    await type(field("Name"), " ");
    expect(field("Name")).toHaveAttribute("aria-invalid", "true");
    expect(field("Name")).toHaveAccessibleDescription("Enter a name to write it.");
    await type(field("Name"), "Terminal");
    expect(field("Name")).not.toHaveAttribute("aria-invalid");
    await type(field("Accent"), "teal");
    expect(field("Accent")).toHaveAccessibleDescription("The accent teal is not written #rrggbb.");
    expect(fake.files.editProfile).not.toHaveBeenCalled();
  });

  it("shows a refused write with its place", async () => {
    mount({
      files: files({
        editProfile: () =>
          Promise.reject({ message: "bad", path: "/c/profiles/terminal.toml", line: 6, column: 10 }),
      }),
    });
    await waitFor(() => expect(field("Name")).toHaveValue("Terminal"));
    await type(field("Name"), "Other");
    expect(await screen.findByRole("alert")).toHaveTextContent("/c/profiles/terminal.toml:6:10: bad");
  });

  it("opens the profile file in the editor", async () => {
    const fake = files();
    mount({ files: fake });
    await waitFor(() => expect(field("Name")).toHaveValue("Terminal"));
    await fireEvent.click(screen.getByRole("button", { name: "Open in editor" }));
    await waitFor(() => expect(fake.files.open).toHaveBeenCalledWith("terminal"));
    expect(screen.getByRole("status", { name: "Profile result" })).toHaveTextContent(
      `Opened ${CONFIG}/profiles/terminal.toml`,
    );
  });

  it("shows why the file did not open", async () => {
    mount({ files: files({ open: () => Promise.reject({ message: "cannot run xdg-open" }) }) });
    await waitFor(() => expect(field("Name")).toHaveValue("Terminal"));
    await fireEvent.click(screen.getByRole("button", { name: "Open in editor" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("cannot run xdg-open");
  });

  it("keeps Open in editor from the built-in global profile until its first edit", async () => {
    const fake = files();
    mount({ files: fake });
    await pick(/^Everywhere/);
    const open = screen.getByRole("button", { name: "Open in editor" });
    expect(open).toHaveAttribute("aria-disabled", "true");
    await fireEvent.click(open);
    expect(fake.files.open).not.toHaveBeenCalled();
    expect(screen.getByText("Built in: the first edit creates this file.")).toBeInTheDocument();
  });
});

describe("the match rules", () => {
  it("are one list of kind and pattern rows, each kind named once", async () => {
    mount();
    await pick(/^Claude Code/);
    expect(within(rules()).getAllByText("Window class")).toHaveLength(1);
    expect(within(rules()).getAllByText("Title regex")).toHaveLength(1);
    for (const pattern of ["konsole", "org.kde.konsole", "(?i)claude"]) {
      expect(within(rules()).getByText(pattern)).toBeInTheDocument();
    }
  });

  it("say when focus never selects the profile", async () => {
    mount();
    await pick(/^Notes/);
    expect(
      within(rules()).getByText("No match rules: focus never selects this profile."),
    ).toBeInTheDocument();
  });
});

describe("the pages", () => {
  it("are thumbnails of the pages as keyvo-core renders them, the first one selected", async () => {
    const props = mount();
    await waitFor(() => expect(within(thumbnails()).getAllByRole("button")).toHaveLength(3));
    const pages = within(thumbnails()).getAllByRole("button");
    expect(pages.map((page) => page.getAttribute("aria-label"))).toEqual([
      "context",
      "general",
      "Page 3",
    ]);
    expect(nth(pages, 0)).toHaveAttribute("aria-pressed", "true");
    expect(nth(pages, 1)).toHaveAttribute("aria-pressed", "false");
    for (const page of [1, 2, 3]) {
      expect(props.daemon.tiles).toHaveBeenCalledWith(
        "terminal",
        page,
        GEOMETRY.rects.length,
        GEOMETRY.tile,
      );
    }
    await waitFor(() =>
      expect(
        nth(pages, 1).querySelector(`img[src="data:image/png;base64,terminal-2-1"]`),
      ).not.toBeNull(),
    );
  });

  it("show the page names only until a Keypad reports its geometry", async () => {
    const props = mount({ geometry: null });
    await waitFor(() => expect(within(thumbnails()).getAllByRole("button")).toHaveLength(3));
    expect(thumbnails().querySelector("img")).toBeNull();
    expect(
      screen.getByText("The thumbnails appear once a Keypad has reported its geometry."),
    ).toBeInTheDocument();
    expect(props.daemon.tiles).not.toHaveBeenCalled();
  });

  it("say when a profile has no pages", async () => {
    mount();
    await pick(/^Notes/);
    expect(screen.getByText("This profile has no pages.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Edit keys on the Pad" })).toBeNull();
  });

  it("open the Pad pinned to the profile and the selected page", async () => {
    const props = mount();
    await waitFor(() => expect(within(thumbnails()).getAllByRole("button")).toHaveLength(3));
    await fireEvent.click(within(thumbnails()).getByRole("button", { name: "general" }));
    expect(within(thumbnails()).getByRole("button", { name: "general" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    await fireEvent.click(screen.getByRole("button", { name: "Edit keys on the Pad" }));
    expect(props.onedit).toHaveBeenCalledWith({ profile: "terminal", page: 2 });
  });

  it("start a newly selected profile on its first page", async () => {
    const props = mount();
    await waitFor(() => expect(within(thumbnails()).getAllByRole("button")).toHaveLength(3));
    await fireEvent.click(within(thumbnails()).getByRole("button", { name: "Page 3" }));
    await pick(/^VS Code/);
    await fireEvent.click(screen.getByRole("button", { name: "Edit keys on the Pad" }));
    expect(props.onedit).toHaveBeenCalledWith({ profile: "code", page: 1 });
  });
});

describe("what is not built yet", () => {
  it.each([
    ["Empty profile", /An empty profile arrives with M4a-4/],
    ["Copy of the active profile", /A copy of the active profile arrives with M4a-4/],
    ["Starter pack", /A profile from a starter pack arrives with M4a-4/],
    ["Add a rule", /Editing match rules arrives with M4a-4/],
    ["Edit the rule org.kde.konsole", /Editing match rules arrives with M4a-4/],
  ])("%s is disabled and says why on focus", async (name, sentence) => {
    mount();
    await waitFor(() => expect(field("Name")).toHaveValue("Terminal"));
    await fireEvent.click(screen.getByRole("button", { name: "New profile" }));
    const control = screen.getByRole("button", { name });
    expect(control).toHaveAttribute("aria-disabled", "true");
    expect(control).toHaveAccessibleDescription("Coming soon");
    await fireEvent.focus(control);
    expect(control).toHaveAccessibleDescription(sentence);
  });

  it("a click on a template does nothing", async () => {
    const props = mount();
    await listed();
    await fireEvent.click(screen.getByRole("button", { name: "New profile" }));
    await fireEvent.click(screen.getByRole("button", { name: "Empty profile" }));
    expect(await listed()).toHaveLength(LIST.length);
    expect(props.fake.files.editProfile).not.toHaveBeenCalled();
  });
});

describe("the thumbnail tiles", () => {
  it("take the corner of their cutout from the drawing, as the mirror does", () => {
    const css = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "Profiles.svelte"), "utf8");
    const rule = /\.thumb img \{([^}]*)\}/.exec(css)?.[1] ?? "";
    expect(rule).toMatch(/border-radius: var\(--kv-tile-radius, var\(--kv-radius\)\)/);
  });
});
