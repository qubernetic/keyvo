// The Profiles screen (docs/design/gui-ux.md §3.2, §4.1) against a fake
// daemon and fake profile files: the list order, the page tabs, a key
// selected into the key editor and the key card, the header edits of the
// shared steps in tests/protocol-fixtures/editor/profile.json (the file a
// step gives is the file keyvo_client::Editor writes for it,
// crates/keyvo-client/tests/editor.rs), and the controls not built yet.
import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { isDeepStrictEqual } from "node:util";
import { describe, expect, it, vi } from "vitest";

import Profiles from "./Profiles.svelte";
import type { Daemon, Tile } from "./lib/daemon";
import type { Editor, KeyFields, Target } from "./lib/editor";
import { editorFile, nth, profileSteps, responses } from "./lib/fixtures";
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
const VIEW: View = { profile: STEPS.profile, page: 1, mode: "auto" };

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
  summary("claude", "Claude Code", 10, { match: { class: ["konsole"], title: "(?i)claude" } }),
  summary("code", "VS Code", 1, { match: { class: ["code"] } }),
  summary("global", "Everywhere", 0, { file: false }),
  summary(STEPS.profile, "Terminal", 5, {
    accent: "#2ecc71",
    match: { class: ["org.kde.konsole"] },
    pages: ["context", "general", null],
  }),
];

/** Key 2 is the profile's own, key 3 a global key under it, key 5 has no
 * layer; the global profile's keys are global. */
function tiles(profile: string, page: number, keys: number): Tile[] {
  return Array.from({ length: keys }, (_, index) => ({
    image: `data:image/png;base64,${profile}-${page}-${index + 1}`,
    ...(index === 1
      ? {
          label: `Copy ${page}`,
          layer: profile === "global" ? ("global" as const) : ("app" as const),
        }
      : {}),
    ...(index === 2 ? { label: "Type", layer: "global" as const } : {}),
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

function editor(): Editor {
  return {
    key: vi.fn((_target: Target) =>
      Promise.resolve({ path: `${CONFIG}/profiles/terminal.toml`, fields: { label: "Copy" } }),
    ),
    editKey: vi.fn((_target: Target, fields: KeyFields) =>
      Promise.resolve({
        path: `${CONFIG}/profiles/terminal.toml`,
        before: { label: "Copy" },
        fields,
      }),
    ),
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
  options: { files?: ReturnType<typeof files>; geometry?: Geometry | null; view?: View } = {},
) {
  const fake = options.files ?? files();
  const props = {
    daemon: daemon(),
    editor: editor(),
    files: fake.files,
    columns: layout(1200),
    view: options.view ?? VIEW,
    ...(options.geometry === null ? {} : { geometry: options.geometry ?? GEOMETRY }),
  };
  render(Profiles, { props });
  return { ...props, fake };
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

function field(name: string): HTMLInputElement {
  return screen.getByRole("textbox", { name }) as HTMLInputElement;
}

async function type(input: HTMLElement, value: string) {
  await fireEvent.input(input, { target: { value } });
  await fireEvent.change(input);
}

function card(): HTMLElement {
  return screen.getByRole("region", { name: "Selected key" });
}

describe("the list column", () => {
  it("pins the global profile first, then orders by priority and marks the active one", async () => {
    mount();
    expect(await listed()).toEqual([
      "Everywhere (global), priority 0",
      "Claude Code (claude), priority 10",
      "Terminal (terminal), priority 5, active",
      "VS Code (code), priority 1",
    ]);
    const active = within(list()).getByRole("button", { name: /, active$/ });
    expect(active).toHaveTextContent("active");
    expect(within(list()).getByRole("button", { name: /^Everywhere/ })).not.toHaveTextContent(
      "active",
    );
  });

  it("selects the active profile first and another one on a click", async () => {
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
    await waitFor(() => expect(field("Name")).toHaveValue("VS Code"));
    expect(fake.files.profile).toHaveBeenLastCalledWith("code");
    expect(screen.getByText("code", { selector: "dd" })).toBeInTheDocument();
  });

  it("selects the global profile when the pad shows none", async () => {
    mount({ view: { profile: "gone", page: 1, mode: "auto" } });
    await listed();
    expect(within(list()).getByRole("button", { name: /^Everywhere/ })).toHaveAttribute(
      "aria-current",
      "true",
    );
    expect(screen.getByText(/The global profile has no match rules/)).toBeInTheDocument();
  });

  it("shows why the profiles did not load", async () => {
    mount({ files: files({ list: () => Promise.reject(new Error("profiles/x.toml: bad")) }) });
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Profiles not loaded: profiles/x.toml: bad",
    );
  });
});

describe("the page tabs", () => {
  it("names each page, its number when it has no name, and starts on page 1", async () => {
    mount();
    const tabs = await screen.findAllByRole("tab");
    expect(tabs.map((tab) => tab.textContent?.trim())).toEqual(["context", "general", "Page 3"]);
    expect(nth(tabs, 0)).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tabpanel")).toHaveAccessibleName("context");
  });

  it("switches the key grid and the mirror to the clicked page", async () => {
    const props = mount();
    const general = await screen.findByRole("tab", { name: "general" });
    await fireEvent.click(general);
    expect(general).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tab", { name: "context" })).toHaveAttribute("aria-selected", "false");
    expect(screen.getByRole("grid", { name: "Keys of page general" })).toBeInTheDocument();
    expect(
      screen.getByRole("grid", { name: "Page preview: profile terminal, page 2" }),
    ).toBeInTheDocument();
    await waitFor(() =>
      expect(props.daemon.tiles).toHaveBeenLastCalledWith(
        "terminal",
        2,
        GEOMETRY.rects.length,
        GEOMETRY.tile,
      ),
    );
    expect(await screen.findAllByRole("button", { name: "Key 2: Copy 2" })).toHaveLength(2);
  });

  it("moves between tabs with the arrow keys, Home and End", async () => {
    mount();
    const context = await screen.findByRole("tab", { name: "context" });
    await fireEvent.keyDown(context, { key: "ArrowRight" });
    expect(screen.getByRole("tab", { name: "general" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tab", { name: "general" })).toHaveFocus();
    await fireEvent.keyDown(screen.getByRole("tab", { name: "general" }), { key: "End" });
    expect(screen.getByRole("tab", { name: "Page 3" })).toHaveAttribute("aria-selected", "true");
    await fireEvent.keyDown(screen.getByRole("tab", { name: "Page 3" }), { key: "ArrowRight" });
    expect(context).toHaveAttribute("aria-selected", "true");
    await fireEvent.keyDown(context, { key: "ArrowLeft" });
    expect(screen.getByRole("tab", { name: "Page 3" })).toHaveAttribute("aria-selected", "true");
    await fireEvent.keyDown(screen.getByRole("tab", { name: "Page 3" }), { key: "Home" });
    expect(context).toHaveAttribute("aria-selected", "true");
  });

  it("starts a newly selected profile on its first page", async () => {
    mount();
    await fireEvent.click(await screen.findByRole("tab", { name: "general" }));
    await fireEvent.click(within(list()).getByRole("button", { name: /^VS Code/ }));
    expect(screen.getAllByRole("tab").map((tab) => tab.textContent?.trim())).toEqual(["context"]);
    expect(screen.getByRole("tab", { name: "context" })).toHaveAttribute("aria-selected", "true");
  });
});

describe("a selected key", () => {
  it("opens the key editor on that key of the selected page", async () => {
    const props = mount();
    await fireEvent.click(await screen.findByRole("tab", { name: "general" }));
    expect(screen.queryByRole("heading", { name: "Key 2" })).toBeNull();
    await fireEvent.click(
      within(screen.getByRole("grid", { name: "Keys of page general" })).getByRole("button", {
        name: "Key 2: Copy 2",
      }),
    );
    expect(await screen.findByRole("heading", { name: "Key 2" })).toBeInTheDocument();
    expect(props.editor.key).toHaveBeenLastCalledWith({ profile: "terminal", page: 2, key: 2 });
    await waitFor(() => expect(field("Label")).toHaveValue("Copy"));
    await type(field("Label"), "Paste");
    await waitFor(() =>
      expect(props.editor.editKey).toHaveBeenCalledWith(
        { profile: "terminal", page: 2, key: 2 },
        { label: "Paste" },
      ),
    );
  });

  it("is selected from the mirror and the keyboard too", async () => {
    mount();
    const mirror = await screen.findByRole("grid", { name: "Page preview: profile terminal, page 1" });
    await fireEvent.click(await within(mirror).findByRole("button", { name: "Key 3: Type" }));
    expect(await screen.findByRole("heading", { name: "Key 3" })).toBeInTheDocument();
    const keys = screen.getByRole("grid", { name: "Keys of page context" });
    await fireEvent.keyDown(keys, { key: "ArrowDown" });
    expect(await screen.findByRole("heading", { name: "Key 6" })).toBeInTheDocument();
    await fireEvent.keyDown(keys, { key: "1" });
    expect(await screen.findByRole("heading", { name: "Key 1" })).toBeInTheDocument();
    expect(within(keys).getByRole("button", { name: "Key 1" })).toHaveFocus();
  });

  it("shows the key card with position, page and the layer it comes from", async () => {
    mount();
    await screen.findByRole("region", { name: "Selected key" });
    expect(within(card()).getByText("Select a key to see its card.")).toBeInTheDocument();
    const keys = await screen.findByRole("grid", { name: "Keys of page context" });
    await fireEvent.click(await within(keys).findByRole("button", { name: "Key 2: Copy 1" }));
    expect(within(card()).getByText("Position").nextElementSibling).toHaveTextContent("Key 2");
    expect(within(card()).getByText("Page").nextElementSibling).toHaveTextContent("1, context");
    expect(within(card()).getByText("Layer origin").nextElementSibling).toHaveTextContent(
      "app layer",
    );
    await fireEvent.click(within(keys).getByRole("button", { name: "Key 3: Type" }));
    expect(within(card()).getByText("Layer origin").nextElementSibling).toHaveTextContent(
      "global layer",
    );
    await fireEvent.click(within(keys).getByRole("button", { name: "Key 5" }));
    expect(within(card()).getByText("Layer origin").nextElementSibling).toHaveTextContent(
      "none: no layer defines this key",
    );
  });

  it("without a geometry keeps the nine key slots and says the mirror waits", async () => {
    mount({ geometry: null });
    const keys = await screen.findByRole("grid", { name: "Keys of page context" });
    expect(within(keys).getAllByRole("row")).toHaveLength(3);
    expect(within(keys).getAllByRole("button")).toHaveLength(9);
    expect(
      screen.getByText("The mirror appears once a Keypad has reported its geometry."),
    ).toBeInTheDocument();
    await fireEvent.click(within(keys).getByRole("button", { name: "Key 4" }));
    expect(within(card()).getByText("Layer origin").nextElementSibling).toHaveTextContent(
      "known once a Keypad has reported its geometry",
    );
  });
});

describe("the profile header", () => {
  it("shows the id, priority and file, and writes the shared steps byte for byte", async () => {
    const fake = files();
    mount({ files: fake });
    await waitFor(() => expect(field("Name")).toHaveValue(STEPS.fields.name));
    expect(field("Accent")).toHaveValue(STEPS.fields.accent);
    expect(screen.getByText("Priority").nextElementSibling).toHaveTextContent("5");
    expect(screen.getByText(`${CONFIG}/${STEPS.path}`)).toBeInTheDocument();
    for (const step of STEPS.steps) {
      const value = step.field === "name" ? step.fields.name : (step.fields.accent ?? "");
      await type(field(step.field === "name" ? "Name" : "Accent"), value);
      await waitFor(() => expect(fake.file).toBe(editorFile(step.file)));
    }
    expect(fake.files.editProfile).toHaveBeenCalledTimes(STEPS.steps.length);
    expect(screen.getByRole("status", { name: "Profile result" })).toHaveTextContent(
      `Wrote the header of terminal to ${CONFIG}/${STEPS.path}`,
    );
    // The list follows the write.
    await waitFor(() =>
      expect(within(list()).getByRole("button", { name: /^Shell work/ })).toBeInTheDocument(),
    );
  });

  it("refuses an empty name and a bad accent without writing", async () => {
    const fake = files();
    mount({ files: fake });
    await waitFor(() => expect(field("Name")).toHaveValue("Terminal"));
    await type(field("Name"), " ");
    expect(await screen.findByRole("alert")).toHaveTextContent("Enter a name to write it.");
    await type(field("Name"), "Terminal");
    await type(field("Accent"), "teal");
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "The accent teal is not written #rrggbb.",
    );
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

  it("disables Open in editor for the built-in global profile", async () => {
    mount();
    await listed();
    await fireEvent.click(within(list()).getByRole("button", { name: /^Everywhere/ }));
    expect(screen.getByRole("button", { name: "Open in editor" })).toBeDisabled();
    expect(screen.getByText("Built in: the first edit creates this file.")).toBeInTheDocument();
  });

  it("lists the match rules as written", async () => {
    mount();
    await listed();
    await fireEvent.click(within(list()).getByRole("button", { name: /^Claude Code/ }));
    const rules = screen.getByRole("region", { name: "Match rules" });
    expect(within(rules).getByText("konsole")).toBeInTheDocument();
    expect(within(rules).getByText("(?i)claude")).toBeInTheDocument();
  });
});

describe("what is not built yet", () => {
  it.each([
    ["New profile", /New profiles from a template/],
    ["Edit match rules", /Editing match rules arrives with M4a-4/],
    ["Move key", /drag and drop, or Ctrl\+X then Ctrl\+V/],
    ["Open in Behaviors", /Behaviors are not built yet/],
  ])("%s is disabled and says why on focus", async (name, sentence) => {
    mount();
    await screen.findAllByRole("tab");
    const control = screen.getByRole("button", { name });
    expect(control).toHaveAttribute("aria-disabled", "true");
    expect(control).toHaveAccessibleDescription("Coming soon");
    await fireEvent.focus(control);
    expect(control).toHaveAccessibleDescription(sentence);
    const tooltip = document.getElementById(control.getAttribute("aria-describedby") ?? "");
    await fireEvent.blur(control);
    expect(tooltip).not.toBeVisible();
    await fireEvent.mouseEnter(control);
    expect(tooltip).toBeVisible();
    expect(tooltip).toHaveTextContent(/^Coming soon$/);
  });

  it("a click on a control not built yet does nothing", async () => {
    const props = mount();
    await listed();
    await fireEvent.click(screen.getByRole("button", { name: "New profile" }));
    expect(await listed()).toHaveLength(LIST.length);
    expect(props.fake.files.editProfile).not.toHaveBeenCalled();
  });
});
