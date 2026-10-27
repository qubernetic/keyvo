// The pinned inspector (#108 test plan) against a fake editor that holds
// the profile file of the shared steps in tests/protocol-fixtures/editor/:
// the file a step's fields give is the file keyvo_client::Editor writes for
// them (crates/keyvo-client/tests/editor.rs), so each test asserts the
// bytes on disk after the GUI's edit. The cases of the key editor it
// replaces on the Pad come first, then the inspector's own.
import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
import { afterEach, describe, expect, it, vi } from "vitest";

import Inspector from "./Inspector.svelte";
import { catalogueOf } from "./lib/catalogue";
import type { Tile } from "./lib/daemon";
import type { Edited, Editor, KeyFields, KeyView, Target } from "./lib/editor";
import { editorFile, editorSteps, nth, recipeReplies, recipeStep } from "./lib/fixtures";

const RECIPES = nth(recipeReplies(), 0).recipes;
const CATALOGUE = catalogueOf(RECIPES);
const STEPS = editorSteps();
const TARGET: Target = { profile: STEPS.profile, page: STEPS.page, key: STEPS.key };
const PATH = `/home/u/.config/keyvo/${STEPS.path}`;

afterEach(() => {
  vi.restoreAllMocks();
});

/** An editor on the fixture's file: a write of the fields one step away
 * moves the file to that step; anything else is refused. */
function fake(overrides: Partial<Editor> = {}) {
  const states = [STEPS.fields, ...STEPS.steps.map((step) => step.fields)];
  const files = [STEPS.file, ...STEPS.steps.map((step) => step.file)];
  let at = 0;
  const editor = {
    key: vi.fn(
      (_target: Target): Promise<KeyView> =>
        Promise.resolve({ path: PATH, fields: nth(states, at), line: STEPS.line }),
    ),
    editKey: vi.fn((_target: Target, fields: KeyFields): Promise<Edited> => {
      const next = states.findIndex((state) => isDeepStrictEqual(state, fields));
      if (next === -1 || Math.abs(next - at) !== 1) {
        return Promise.reject({ message: `no step writes ${JSON.stringify(fields)}` });
      }
      const before = nth(states, at);
      at = next;
      return Promise.resolve({ path: PATH, before, fields, line: STEPS.line, snapshot: "/s/1-terminal.toml" });
    }),
    ...overrides,
  };
  return {
    editor: editor satisfies Editor,
    /** The text of the file as the editor left it. */
    get file() {
      return editorFile(nth(files, at));
    },
  };
}

/** An editor whose writes all succeed, each answering the fields written. */
function accepting(fields: KeyFields = STEPS.fields) {
  return fake({
    editKey: vi.fn((_target: Target, written: KeyFields) =>
      Promise.resolve({ path: PATH, before: fields, fields: written, line: STEPS.line }),
    ),
  });
}

function field(name: string): HTMLInputElement {
  return screen.getByRole("textbox", { name }) as HTMLInputElement;
}

async function type(input: HTMLElement, value: string) {
  await fireEvent.input(input, { target: { value } });
  await fireEvent.change(input);
}

function action(): HTMLSelectElement {
  return screen.getByRole("combobox", { name: "Action" }) as HTMLSelectElement;
}

function result(): HTMLElement {
  return screen.getByRole("status", { name: "Inspector result" });
}

interface Mount {
  target?: Target | null;
  global?: Target;
  tile?: Tile;
  onpress?: () => void;
}

/** The inspector mounted on `target`, the fixture's key unless `null`. */
async function mounted(editor = fake(), { target = TARGET, global, tile, onpress }: Mount = {}) {
  const onedited = vi.fn();
  const view = render(Inspector, {
    props: { editor: editor.editor, target: target ?? undefined, global, tile, onpress, onedited },
  });
  if (target !== null && global === undefined) {
    await waitFor(() => expect(field("Label")).toHaveValue("Copy"));
  }
  return {
    editor: editor.editor,
    get file() {
      return editor.file;
    },
    onedited,
    view,
  };
}

/** The steps of the fixture, driven through the form. */
async function edit() {
  for (const step of STEPS.steps) {
    if (step.field === "label") {
      await type(field("Label"), step.fields.label ?? "");
    } else {
      const written = step.fields.action;
      if (written?.type !== "exec") throw new Error("the fixture's action step is an exec");
      await fireEvent.change(action(), { target: { value: "exec" } });
      await type(field("Command"), written.cmd);
    }
  }
}

describe("Inspector, the key editor's cases", () => {
  it("shows the key's fields and its file", async () => {
    const editor = await mounted();
    expect(editor.editor.key).toHaveBeenCalledWith(TARGET);
    expect(screen.getByRole("heading", { name: "Key 2: Chord" })).toBeInTheDocument();
    expect(screen.getByText(`${PATH} line ${STEPS.line}`)).toBeInTheDocument();
    expect(field("Color")).toHaveValue("");
    expect(screen.getByRole("combobox", { name: "Icon" })).toHaveValue("");
    expect(action()).toHaveValue("chord");
    expect(field("Chord")).toHaveValue("ctrl+shift+c");
  });

  it("writes each change at once and the file is the fixture's", async () => {
    const editor = await mounted();
    await type(field("Label"), "Paste");
    await waitFor(() => expect(editor.file).toBe(editorFile(nth(STEPS.steps, 0).file)));
    expect(editor.editor.editKey).toHaveBeenLastCalledWith(TARGET, nth(STEPS.steps, 0).fields);
    expect(editor.onedited).toHaveBeenCalledTimes(1);
    expect(result()).toHaveTextContent(`Wrote key 2 to ${PATH} at`);
  });

  it("changes the action type without writing until the command is entered", async () => {
    const editor = await mounted();
    await type(field("Label"), "Paste");
    await waitFor(() => expect(editor.editor.editKey).toHaveBeenCalledTimes(1));
    await fireEvent.change(action(), { target: { value: "exec" } });
    expect(field("Command")).toHaveValue("");
    expect(field("Command")).toHaveAccessibleDescription("Enter the command to write it.");
    expect(editor.editor.editKey).toHaveBeenCalledTimes(1);
    await type(field("Command"), "wl-paste");
    await waitFor(() => expect(editor.file).toBe(editorFile(nth(STEPS.steps, 1).file)));
  });

  it("undoes and redoes the session's writes with Ctrl+Z and Ctrl+Shift+Z", async () => {
    const editor = await mounted();
    await edit();
    await waitFor(() => expect(editor.file).toBe(editorFile(nth(STEPS.steps, 1).file)));

    await fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    await waitFor(() => expect(editor.file).toBe(editorFile(nth(STEPS.steps, 0).file)));
    await waitFor(() => expect(field("Chord")).toHaveValue("ctrl+shift+c"));
    await fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    await waitFor(() => expect(editor.file).toBe(editorFile(STEPS.file)));
    await waitFor(() => expect(field("Label")).toHaveValue("Copy"));
    expect(screen.getByRole("button", { name: "Undo" })).toBeDisabled();

    await fireEvent.keyDown(window, { key: "Z", ctrlKey: true, shiftKey: true });
    await waitFor(() => expect(editor.file).toBe(editorFile(nth(STEPS.steps, 0).file)));
    await fireEvent.click(screen.getByRole("button", { name: "Redo" }));
    await waitFor(() => expect(editor.file).toBe(editorFile(nth(STEPS.steps, 1).file)));
    expect(screen.getByRole("button", { name: "Redo" })).toBeDisabled();
    await fireEvent.click(screen.getByRole("button", { name: "Undo" }));
    await waitFor(() => expect(editor.file).toBe(editorFile(nth(STEPS.steps, 0).file)));
    expect(result()).toHaveTextContent("Undid the change to key 2 at");
  });

  it("leaves Ctrl+Z to a field that has the focus", async () => {
    const editor = await mounted();
    await type(field("Label"), "Paste");
    await waitFor(() => expect(editor.editor.editKey).toHaveBeenCalledTimes(1));
    await fireEvent.keyDown(screen.getByRole("combobox", { name: "Icon" }), { key: "z", ctrlKey: true });
    await fireEvent.keyDown(field("Label"), { key: "z", ctrlKey: true });
    expect(editor.editor.editKey).toHaveBeenCalledTimes(1);
  });

  it("leaves Ctrl+Z alone while the shell shows another screen", async () => {
    const editor = await mounted();
    await type(field("Label"), "Paste");
    await waitFor(() => expect(editor.editor.editKey).toHaveBeenCalledTimes(1));
    await editor.view.rerender({ active: false });
    await fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    expect(editor.editor.editKey).toHaveBeenCalledTimes(1);
    await editor.view.rerender({ active: true });
    await fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    await waitFor(() => expect(editor.editor.editKey).toHaveBeenCalledTimes(2));
  });

  it("drops what is left to redo on a new write", async () => {
    const editor = await mounted();
    await type(field("Label"), "Paste");
    await waitFor(() => expect(editor.file).toBe(editorFile(nth(STEPS.steps, 0).file)));
    await fireEvent.click(screen.getByRole("button", { name: "Undo" }));
    await waitFor(() => expect(editor.file).toBe(editorFile(STEPS.file)));
    await type(field("Label"), "Paste");
    await waitFor(() => expect(editor.file).toBe(editorFile(nth(STEPS.steps, 0).file)));
    expect(screen.getByRole("button", { name: "Redo" })).toBeDisabled();
  });

  it("does not write a field that did not change", async () => {
    const editor = await mounted();
    await type(field("Label"), "Copy");
    expect(editor.editor.editKey).not.toHaveBeenCalled();
  });

  it("refuses a color that is not #rrggbb without writing, the message under the field", async () => {
    const editor = await mounted();
    await type(field("Color"), "teal");
    expect(screen.getByRole("alert")).toHaveTextContent("The color teal is not written #rrggbb.");
    expect(field("Color")).toHaveAttribute("aria-invalid", "true");
    expect(field("Color")).toHaveAccessibleDescription("The color teal is not written #rrggbb.");
    expect(editor.editor.editKey).not.toHaveBeenCalled();
  });

  it("removes the action when it is set to none", async () => {
    const editor = await mounted(accepting());
    await fireEvent.change(action(), { target: { value: "none" } });
    await waitFor(() => expect(editor.editor.editKey).toHaveBeenCalledWith(TARGET, { label: "Copy" }));
    expect(screen.queryByRole("textbox", { name: "Chord" })).not.toBeInTheDocument();
  });

  it("shows a schema error of the file with its file, line and column", async () => {
    const problem = { message: "invalid type: integer `7`, expected a string", path: PATH, line: 4, column: 8 };
    render(Inspector, {
      props: { editor: fake({ key: () => Promise.reject(problem) }).editor, target: TARGET },
    });
    expect(await screen.findByRole("alert")).toHaveTextContent(
      `${PATH}:4:8: invalid type: integer \`7\`, expected a string`,
    );
    expect(screen.queryByRole("textbox", { name: "Label" })).not.toBeInTheDocument();
  });

  it("shows a refused write with its place, keeps the typed value and the history", async () => {
    const problem = { message: "invalid key fields: unknown key name `hyperr`", path: PATH, line: 17, column: 1 };
    const editor = await mounted(fake({ editKey: () => Promise.reject(problem) }));
    await type(field("Label"), "Paste");
    expect(await screen.findByRole("alert")).toHaveTextContent(`${PATH}:17:1: ${problem.message}`);
    expect(field("Label")).toHaveValue("Paste");
    expect(screen.getByRole("button", { name: "Undo" })).toBeDisabled();
    expect(editor.onedited).not.toHaveBeenCalled();
  });

  it("asks for a key until there is one", async () => {
    await mounted(fake(), { target: null });
    expect(screen.getByText("Select a key to edit it.")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Inspector" })).toBeInTheDocument();
    expect(screen.queryByRole("textbox", { name: "Label" })).not.toBeInTheDocument();
  });

  it("loads another key when the target changes", async () => {
    const editor = await mounted();
    await editor.view.rerender({ target: { ...TARGET, key: 5 } });
    await waitFor(() => expect(editor.editor.key).toHaveBeenLastCalledWith({ ...TARGET, key: 5 }));
    expect(screen.getByRole("heading", { name: "Key 5: Chord" })).toBeInTheDocument();
  });

  it("reads the key again on a new revision but keeps a field being edited", async () => {
    const editor = await mounted();
    await fireEvent.input(field("Label"), { target: { value: "Pas" } });
    await editor.view.rerender({ revision: 1 });
    await waitFor(() => expect(editor.editor.key).toHaveBeenCalledTimes(2));
    expect(field("Label")).toHaveValue("Pas");
  });
});

describe("Inspector", () => {
  it("names the key's kind and its file and line in the header", async () => {
    const exec: KeyFields = { label: "Test", action: { type: "exec", cmd: "cargo test" } };
    render(Inspector, {
      props: {
        editor: fake({ key: () => Promise.resolve({ path: PATH, fields: exec, line: 30 }) }).editor,
        target: { ...TARGET, key: 5 },
      },
    });
    expect(await screen.findByRole("heading", { name: "Key 5: Exec" })).toBeInTheDocument();
    expect(screen.getByText(`${PATH} line 30`)).toBeInTheDocument();
  });

  it("says a key the page does not define has no action and no line yet", async () => {
    render(Inspector, {
      props: {
        editor: fake({ key: () => Promise.resolve({ path: PATH, fields: {} }) }).editor,
        target: { ...TARGET, key: 8 },
      },
    });
    expect(await screen.findByRole("heading", { name: "Key 8: No action" })).toBeInTheDocument();
    expect(screen.getByText(`${PATH}, not on this page yet`)).toBeInTheDocument();
  });

  it("shows the tile image of the key", async () => {
    await mounted(fake(), { tile: { image: "data:image/png;base64,AAAA", layer: "app" } });
    expect(screen.getByRole("img", { name: "Tile of key 2" })).toHaveAttribute(
      "src",
      "data:image/png;base64,AAAA",
    );
  });

  describe("a key that falls through from the global profile", () => {
    const GLOBAL_PATH = "/home/u/.config/keyvo/profiles/global.toml";
    const GLOBAL: Target = { profile: "global", page: 1, key: 5 };
    const UNDER: Target = { profile: "terminal", page: 1, key: 5 };
    const FIELDS: KeyFields = { label: "Test", icon: "lucide:play", action: { type: "exec", cmd: "cargo test" } };

    function editor() {
      return fake({
        key: vi.fn((target: Target) =>
          Promise.resolve(
            target.profile === "global"
              ? { path: GLOBAL_PATH, fields: FIELDS, line: 41 }
              : { path: "/home/u/.config/keyvo/profiles/terminal.toml", fields: {} },
          ),
        ),
        editKey: vi.fn((_target: Target, fields: KeyFields) =>
          Promise.resolve({ path: "/home/u/.config/keyvo/profiles/terminal.toml", before: {}, fields, line: 12 }),
        ),
      });
    }

    it("names the global key, says where it comes from and offers to override it", async () => {
      await mounted(editor(), { target: UNDER, global: GLOBAL });
      expect(await screen.findByRole("heading", { name: "Key 5: Exec" })).toBeInTheDocument();
      expect(screen.getByText(`${GLOBAL_PATH} line 41`)).toBeInTheDocument();
      expect(screen.getByText("from global")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Override here" })).toBeInTheDocument();
    });

    it("writes the global key's fields into the pinned profile's key, undoably", async () => {
      const keys = editor();
      await mounted(keys, { target: UNDER, global: GLOBAL });
      await fireEvent.click(await screen.findByRole("button", { name: "Override here" }));
      await waitFor(() => expect(keys.editor.editKey).toHaveBeenCalledWith(UNDER, FIELDS));
      await waitFor(() => expect(field("Label")).toHaveValue("Test"));
      expect(result()).toHaveTextContent("Wrote key 5 to /home/u/.config/keyvo/profiles/terminal.toml at");
      await fireEvent.click(screen.getByRole("button", { name: "Undo" }));
      await waitFor(() => expect(keys.editor.editKey).toHaveBeenLastCalledWith(UNDER, {}));
    });

    it("does not offer the override for a key of the page", async () => {
      await mounted();
      expect(screen.queryByText("from global")).not.toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Override here" })).not.toBeInTheDocument();
    });
  });

  it("clears the key with every field absent, undoably", async () => {
    const editor = await mounted(accepting());
    await fireEvent.click(screen.getByRole("button", { name: "Clear" }));
    await waitFor(() => expect(editor.editor.editKey).toHaveBeenCalledWith(TARGET, {}));
    await waitFor(() => expect(field("Label")).toHaveValue(""));
    expect(screen.getByRole("heading", { name: "Key 2: No action" })).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Undo" }));
    await waitFor(() => expect(editor.editor.editKey).toHaveBeenLastCalledWith(TARGET, STEPS.fields));
  });

  it("presses the key as a dry run through the caller", async () => {
    const onpress = vi.fn();
    await mounted(fake(), { onpress });
    await fireEvent.click(screen.getByRole("button", { name: "Press" }));
    expect(onpress).toHaveBeenCalledTimes(1);
  });

  it("stamps the result line with the time of the write", async () => {
    vi.spyOn(Date, "now").mockReturnValue(new Date(2026, 9, 26, 14, 5, 9).getTime());
    await mounted();
    await type(field("Label"), "Paste");
    await waitFor(() => expect(result()).toHaveTextContent(`Wrote key 2 to ${PATH} at 14:05:09`));
  });

  it("keeps the colour swatch and its text in step", async () => {
    const editor = await mounted(accepting());
    const swatch = screen.getByLabelText("Color swatch");
    await type(field("Color"), "#AA3300");
    expect(swatch).toHaveValue("#aa3300");
    await waitFor(() =>
      expect(editor.editor.editKey).toHaveBeenLastCalledWith(TARGET, { ...STEPS.fields, color: "#aa3300" }),
    );
    await fireEvent.input(swatch, { target: { value: "#123456" } });
    expect(field("Color")).toHaveValue("#123456");
    await fireEvent.change(swatch);
    await waitFor(() =>
      expect(editor.editor.editKey).toHaveBeenLastCalledWith(TARGET, { ...STEPS.fields, color: "#123456" }),
    );
  });

  it("writes a picked icon as lucide:<name>", async () => {
    const editor = await mounted(accepting());
    const icon = screen.getByRole("combobox", { name: "Icon" });
    await fireEvent.input(icon, { target: { value: "termi" } });
    await fireEvent.keyDown(icon, { key: "ArrowDown" });
    await fireEvent.keyDown(icon, { key: "Enter" });
    await waitFor(() =>
      expect(editor.editor.editKey).toHaveBeenLastCalledWith(TARGET, { ...STEPS.fields, icon: "lucide:terminal" }),
    );
  });

  it("caps the form at 48ch", () => {
    const source = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "Inspector.svelte"), "utf8");
    expect(source).toMatch(/form\s*\{[^}]*max-width:\s*48ch/);
  });
});

describe("the actions catalogue", () => {
  it("applies an assigned tool: the action type set, the field empty, nothing written", async () => {
    const editor = await mounted(accepting());
    await editor.view.rerender({ assign: { entry: "tool:exec", target: TARGET, seq: 1 } });
    await waitFor(() => expect(action()).toHaveValue("exec"));
    expect(field("Command")).toHaveValue("");
    expect(field("Command")).not.toHaveAccessibleDescription();
    expect(field("Label")).toHaveValue("Copy");
    expect(editor.editor.editKey).not.toHaveBeenCalled();
    await type(field("Command"), "wl-paste");
    await waitFor(() =>
      expect(editor.editor.editKey).toHaveBeenLastCalledWith(TARGET, {
        ...STEPS.fields,
        action: { type: "exec", cmd: "wl-paste" },
      }),
    );
  });

  it("applies an assignment for another key once that key is read, and only once", async () => {
    const other = { ...TARGET, key: 7 };
    const editor = await mounted();
    const assign = { entry: "tool:exec", target: other, seq: 1 };
    await editor.view.rerender({ target: other, assign });
    await waitFor(() => expect(editor.editor.key).toHaveBeenLastCalledWith(other));
    await waitFor(() => expect(action()).toHaveValue("exec"));
    expect(field("Command")).toHaveValue("");
    // Away and back: the key reads as the file has it, the tool is not applied again.
    await editor.view.rerender({ target: TARGET, assign });
    await waitFor(() => expect(action()).toHaveValue("chord"));
    await editor.view.rerender({ target: other, assign });
    await waitFor(() => expect(editor.editor.key).toHaveBeenCalledTimes(4));
    await waitFor(() => expect(field("Chord")).toHaveValue("ctrl+shift+c"));
    expect(action()).toHaveValue("chord");
  });

  it("carries the catalogue's entries in the Action select while the column is collapsed", async () => {
    const editor = await mounted();
    expect(screen.queryAllByRole("group")).toEqual([]);
    await editor.view.rerender({ entries: CATALOGUE, recipes: RECIPES });
    const groups = screen.getAllByRole("group");
    expect(groups.map((group) => group.getAttribute("label"))).toEqual(["General", "System", "Apps"]);
    const exec = screen.getByRole("option", { name: "Exec" }) as HTMLOptionElement;
    expect(exec).toHaveValue("exec");
    expect(exec).toBeEnabled();
    expect(screen.getByRole("option", { name: "Spectacle" })).toBeEnabled();
    await fireEvent.change(action(), { target: { value: "exec" } });
    expect(field("Command")).toHaveValue("");
    expect(editor.editor.editKey).not.toHaveBeenCalled();
  });
});

describe("a preset", () => {
  const STEP = recipeStep();
  const KEY7: Target = { profile: STEP.profile, page: STEP.page, key: STEP.key };
  const DEFAULT: KeyFields = {
    icon: "lucide:camera",
    label: "Capture",
    action: { type: "exec", cmd: "spectacle --fullscreen" },
  };

  /** Key 7 of the recipe fixture: blank, then whatever was written last. */
  function blank() {
    let fields = STEP.before;
    const editor = {
      key: vi.fn((_target: Target): Promise<KeyView> => Promise.resolve({ path: PATH, fields })),
      editKey: vi.fn((_target: Target, written: KeyFields): Promise<Edited> => {
        const before = fields;
        fields = written;
        return Promise.resolve({ path: PATH, before, fields: written, line: STEP.line });
      }),
    };
    return editor satisfies Editor;
  }

  function whatItDoes(): HTMLSelectElement {
    return screen.getByRole("combobox", { name: "What it does" }) as HTMLSelectElement;
  }

  async function assigned(entry: string) {
    const editor = blank();
    const view = render(Inspector, { props: { editor, target: KEY7, recipes: RECIPES } });
    await waitFor(() => expect(editor.key).toHaveBeenCalled());
    expect(screen.queryByRole("combobox", { name: "What it does" })).toBeNull();
    await view.rerender({ assign: { entry, target: KEY7, seq: 1 } });
    return { editor, view };
  }

  it("is written with its default variant, label and icon when assigned", async () => {
    const { editor } = await assigned("recipe:spectacle");
    await waitFor(() => expect(editor.editKey).toHaveBeenCalledWith(KEY7, DEFAULT));
    expect(field("Label")).toHaveValue("Capture");
    expect(field("Command")).toHaveValue("spectacle --fullscreen");
    await waitFor(() => expect(whatItDoes()).toHaveValue("fullscreen"));
  });

  it("What it does lists the variants and rewrites the action, as apply_recipe writes it", async () => {
    const { editor } = await assigned("recipe:spectacle");
    await waitFor(() => expect(whatItDoes()).toHaveValue("fullscreen"));
    const select = whatItDoes();
    expect([...select.options].map((option) => option.text)).toEqual([
      "Full screen",
      "Region",
      "Recording",
    ]);
    await fireEvent.change(select, { target: { value: STEP.variant } });
    await waitFor(() => expect(editor.editKey).toHaveBeenLastCalledWith(KEY7, STEP.fields));
    expect(field("Command")).toHaveValue("spectacle --region");
    expect(whatItDoes()).toHaveValue("region");
    // The written key reads back as the preset's, so the select stays.
    expect(editor.editKey).toHaveBeenCalledTimes(2);
  });

  it("What it does switches the action type for a variant of another kind", async () => {
    const { editor } = await assigned("recipe:volume-up");
    await waitFor(() => expect(whatItDoes()).toHaveValue("plasma"));
    await fireEvent.change(whatItDoes(), { target: { value: "key" } });
    await waitFor(() =>
      expect(editor.editKey).toHaveBeenLastCalledWith(KEY7, {
        icon: "lucide:volume-2",
        label: "Vol +",
        action: { type: "chord", keys: "volumeup" },
      }),
    );
    expect(action()).toHaveValue("chord");
  });

  it("is applied from the Action select while the column is collapsed", async () => {
    const editor = blank();
    render(Inspector, { props: { editor, target: KEY7, recipes: RECIPES, entries: CATALOGUE } });
    await waitFor(() => expect(editor.key).toHaveBeenCalled());
    await fireEvent.change(action(), { target: { value: "recipe:spectacle" } });
    await waitFor(() => expect(editor.editKey).toHaveBeenCalledWith(KEY7, DEFAULT));
    expect(action()).toHaveValue("exec");
  });

  it("an unknown recipe assigns nothing", async () => {
    const { editor } = await assigned("recipe:nope");
    await waitFor(() => expect(field("Label")).toHaveValue(""));
    expect(editor.editKey).not.toHaveBeenCalled();
  });
});

describe("the inspector's rules", () => {
  const css = readFileSync(join(dirname(fileURLToPath(import.meta.url)), "Inspector.svelte"), "utf8");

  it("sizes a label inside a field once, at the label size", () => {
    // `.field` and `label` both carry 0.85em; nested, the Command label
    // rendered at 9.6 px (owner, 2026-10-27).
    const nested = /\.field label \{([^}]*)\}/.exec(css)?.[1] ?? "";
    expect(nested).toMatch(/font-size: 1em/);
  });

  it("reserves one line for the result, so the first message shifts nothing", () => {
    expect(css).toMatch(/\.result \{[^}]*min-height: 1\.4em/);
  });
});
