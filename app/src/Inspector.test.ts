// The pinned inspector (#108 test plan) against a fake editor that holds
// the profile file of the shared steps in tests/protocol-fixtures/editor/:
// the file a step's fields give is the file keyvo_client::Editor writes for
// them (crates/keyvo-client/tests/editor.rs), so each test asserts the
// bytes on disk after the GUI's edit. The cases of the key editor it
// replaces on the Pad come first, then the inspector's own, then its
// control editor for the Dialpad (#188) against an editor of controls held
// in memory.
import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
import { afterEach, describe, expect, it, vi } from "vitest";

import Inspector from "./Inspector.svelte";
import { catalogueOf } from "./lib/catalogue";
import type { Tile } from "./lib/daemon";
import type {
  ControlFields,
  ControlTarget,
  Edited,
  Editor,
  KeyFields,
  KeyView,
  Target,
} from "./lib/editor";
import {
  editorFile,
  editorSteps,
  nth,
  recipeReplies,
  recipeStep,
  UNBOUND_CONTROLS,
} from "./lib/fixtures";
import type { Control } from "./lib/protocol";

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
    ...UNBOUND_CONTROLS,
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

/** Lets the answers already given land in the form. */
async function settled() {
  await new Promise((resolve) => setTimeout(resolve, 0));
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

function waitSwitch(): HTMLInputElement {
  return screen.getByRole("switch", { name: "Wait for the window" }) as HTMLInputElement;
}

/** The steps of the fixture, driven through the form, each written before
 * the next. */
async function edit(editor: { readonly file: string }) {
  for (const step of STEPS.steps) {
    const written = step.fields.action;
    if (step.field === "label") {
      await type(field("Label"), step.fields.label ?? "");
    } else if (step.field === "action") {
      if (written === undefined || written.type === "chord") throw new Error("an action step writes a command");
      await fireEvent.change(action(), { target: { value: written.type } });
      await type(field("Command"), written.cmd);
    } else if (step.field === "class") {
      if (written?.type !== "launch") throw new Error("the class step is a launch's");
      await type(field("Window class"), written.class ?? "");
    } else {
      await fireEvent.click(waitSwitch());
    }
    await waitFor(() => expect(editor.file).toBe(editorFile(step.file)));
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

  it("explains the chord syntax under the Chord field, and the command field has no hint", async () => {
    await mounted();
    const hint = "Keys joined with +, for example ctrl+shift+g; modifiers ctrl, alt, shift, super.";
    expect(screen.getByText(hint)).toBeInTheDocument();
    expect(field("Chord")).toHaveAccessibleDescription(hint);
    await fireEvent.change(action(), { target: { value: "exec" } });
    expect(screen.queryByText(hint)).not.toBeInTheDocument();
    expect(field("Command")).toHaveAccessibleDescription("Enter the command to write it.");
  });

  it("names the wrong token of a chord under the field and writes nothing until it parses", async () => {
    const editor = await mounted(accepting());
    await type(field("Chord"), "ctrl+nope");
    await settled();
    expect(screen.getByRole("alert")).toHaveTextContent("nope is not a key name.");
    expect(field("Chord")).toHaveAttribute("aria-invalid", "true");
    expect(field("Chord")).toHaveAccessibleDescription(
      "nope is not a key name. Keys joined with +, for example ctrl+shift+g; modifiers ctrl, alt, shift, super.",
    );
    expect(field("Chord")).toHaveValue("ctrl+nope");
    expect(editor.editor.editKey).not.toHaveBeenCalled();
    await type(field("Chord"), "g+ctrl");
    await settled();
    expect(screen.getByRole("alert")).toHaveTextContent("g is not a modifier");
    await type(field("Chord"), "ctrl+shift+v");
    await waitFor(() =>
      expect(editor.editor.editKey).toHaveBeenLastCalledWith(TARGET, {
        ...STEPS.fields,
        action: { type: "chord", keys: "ctrl+shift+v" },
      }),
    );
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(field("Chord")).not.toHaveAttribute("aria-invalid");
  });

  it("offers Launch next to Chord and Exec and writes it from the command field", async () => {
    const editor = await mounted(accepting());
    const kinds = within(action())
      .getAllByRole("option")
      .map((option) => option.textContent);
    expect(kinds).toEqual(["No action", "Chord", "Exec", "Launch"]);
    await fireEvent.change(action(), { target: { value: "launch" } });
    expect(field("Command")).toHaveValue("");
    expect(editor.editor.editKey).not.toHaveBeenCalled();
    await type(field("Command"), "flatpak run org.kde.kcalc");
    await waitFor(() =>
      expect(editor.editor.editKey).toHaveBeenLastCalledWith(TARGET, {
        ...STEPS.fields,
        action: { type: "launch", cmd: "flatpak run org.kde.kcalc" },
      }),
    );
  });

  it("shows a launch's window class with its hint and the wait switch, and no other action does", async () => {
    await mounted();
    expect(screen.queryByRole("textbox", { name: "Window class" })).not.toBeInTheDocument();
    expect(screen.queryByRole("switch")).not.toBeInTheDocument();
    await fireEvent.change(action(), { target: { value: "launch" } });
    expect(field("Window class")).toHaveValue("");
    expect(field("Window class")).toHaveAccessibleDescription(
      "The class the launched window carries; keyvo ctl status prints the focused one.",
    );
    expect(waitSwitch()).toBeChecked();
    await fireEvent.change(action(), { target: { value: "exec" } });
    expect(screen.queryByRole("switch")).not.toBeInTheDocument();
  });

  it("forgets a launch's class and wait when the action type changes", async () => {
    const launch: KeyFields = {
      label: "Calc",
      action: { type: "launch", cmd: "kcalc", class: "org.kde.kcalc", wait_focus: false },
    };
    const editor = accepting(launch);
    editor.editor.key = vi.fn(() => Promise.resolve({ path: PATH, fields: launch, line: STEPS.line }));
    render(Inspector, { props: { editor: editor.editor, target: TARGET } });
    await waitFor(() => expect(field("Window class")).toHaveValue("org.kde.kcalc"));
    expect(waitSwitch()).not.toBeChecked();
    await fireEvent.change(action(), { target: { value: "exec" } });
    await fireEvent.change(action(), { target: { value: "launch" } });
    expect(field("Window class")).toHaveValue("");
    expect(waitSwitch()).toBeChecked();
  });

  it("undoes and redoes the session's writes with Ctrl+Z and Ctrl+Shift+Z", async () => {
    const editor = await mounted();
    const files = [STEPS.file, ...STEPS.steps.map((step) => step.file)];
    await edit(editor);

    for (let at = files.length - 2; at >= 0; at--) {
      await fireEvent.keyDown(window, { key: "z", ctrlKey: true });
      await waitFor(() => expect(editor.file).toBe(editorFile(nth(files, at))));
      if (at === 1) await waitFor(() => expect(field("Chord")).toHaveValue("ctrl+shift+c"));
    }
    await waitFor(() => expect(field("Label")).toHaveValue("Copy"));
    expect(screen.getByRole("button", { name: "Undo" })).toBeDisabled();

    await fireEvent.keyDown(window, { key: "Z", ctrlKey: true, shiftKey: true });
    await waitFor(() => expect(editor.file).toBe(editorFile(nth(files, 1))));
    for (let at = 2; at < files.length; at++) {
      await fireEvent.click(screen.getByRole("button", { name: "Redo" }));
      await waitFor(() => expect(editor.file).toBe(editorFile(nth(files, at))));
    }
    expect(screen.getByRole("button", { name: "Redo" })).toBeDisabled();
    await waitFor(() => expect(waitSwitch()).not.toBeChecked());
    expect(field("Window class")).toHaveValue("wl-paste");
    await fireEvent.click(screen.getByRole("button", { name: "Undo" }));
    await waitFor(() => expect(editor.file).toBe(editorFile(nth(files, files.length - 2))));
    await waitFor(() => expect(waitSwitch()).toBeChecked());
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

  describe("keeps the typed icon search", () => {
    /** Types `hou` into Icon; the list holds its matches only. */
    async function search(): Promise<HTMLInputElement> {
      const icon = screen.getByRole("combobox", { name: "Icon" }) as HTMLInputElement;
      await fireEvent.input(icon, { target: { value: "hou" } });
      expect(icon).toHaveValue("hou");
      return icon;
    }

    function searched(icon: HTMLInputElement) {
      expect(icon).toHaveValue("hou");
      const rows = within(screen.getByRole("listbox", { name: "Icon" }))
        .getAllByRole("option")
        .map((option) => option.textContent.trim());
      expect(rows).toContain("house");
      expect(rows.every((name) => name.includes("hou"))).toBe(true);
    }

    it("through a new revision of the same key", async () => {
      const editor = await mounted();
      const icon = await search();
      await editor.view.rerender({ revision: 1 });
      await waitFor(() => expect(editor.editor.key).toHaveBeenCalledTimes(2));
      await settled();
      searched(icon);
    });

    it("through an acknowledged write of another field", async () => {
      const editor = await mounted(accepting());
      const icon = await search();
      await fireEvent.input(field("Label"), { target: { value: "Pas" } });
      await fireEvent.change(field("Label"));
      await waitFor(() => expect(editor.editor.editKey).toHaveBeenCalledTimes(1));
      await settled();
      searched(icon);
    });

    it("but not through a read of another key", async () => {
      const editor = await mounted();
      const committed = (screen.getByRole("combobox", { name: "Icon" }) as HTMLInputElement).value;
      await search();
      await editor.view.rerender({ target: { ...TARGET, key: 5 } });
      await waitFor(() => expect(editor.editor.key).toHaveBeenLastCalledWith({ ...TARGET, key: 5 }));
      await settled();
      expect(screen.getByRole("combobox", { name: "Icon" })).toHaveValue(committed);
    });
  });
});

describe("Inspector", () => {
  it("shows the key's on_fail read-only under the action and keeps it through a write", async () => {
    const onFail = { type: "exec", cmd: "notify-send 'keyvo' 'key 2 failed'" } as const;
    const editor = fake({
      key: vi.fn(() => Promise.resolve({ path: PATH, fields: STEPS.fields, line: STEPS.line, on_fail: onFail })),
    });
    await mounted(editor);
    const line = "Runs notify-send 'keyvo' 'key 2 failed' when this fails. Edit it in the file.";
    expect(screen.getByText(/^Runs/, { selector: "p" })).toHaveTextContent(line);
    expect(screen.getByText(onFail.cmd, { selector: "code" })).toBeInTheDocument();
    // Nothing in the form edits it, and a write keeps the line.
    expect(screen.queryByRole("textbox", { name: /on_fail/i })).toBeNull();
    await type(field("Label"), "Paste");
    await waitFor(() => expect(editor.file).toBe(editorFile(nth(STEPS.steps, 0).file)));
    expect(editor.editor.editKey).toHaveBeenLastCalledWith(TARGET, nth(STEPS.steps, 0).fields);
    expect(screen.getByText(/^Runs/, { selector: "p" })).toHaveTextContent(line);
  });

  it("has no on_fail line for a key without one", async () => {
    await mounted();
    expect(screen.queryByText(/when this fails/)).toBeNull();
  });

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
    await fireEvent.input(icon, { target: { value: "hamm" } });
    await fireEvent.keyDown(icon, { key: "ArrowDown" });
    await fireEvent.keyDown(icon, { key: "Enter" });
    await waitFor(() =>
      expect(editor.editor.editKey).toHaveBeenLastCalledWith(TARGET, { ...STEPS.fields, icon: "lucide:hammer" }),
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
    expect(screen.getByRole("option", { name: "Launch" })).toHaveValue("launch");
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
      ...UNBOUND_CONTROLS,
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
  const here = dirname(fileURLToPath(import.meta.url));
  const css = readFileSync(join(here, "Inspector.svelte"), "utf8");
  /** The action part, shared with the control editor (#188). */
  const actionCss = readFileSync(join(here, "ActionFields.svelte"), "utf8");

  it("sizes a label inside a field once, at the label size", () => {
    // `.field` and `label` both carry 0.85em; nested, the Command label
    // rendered at 9.6 px (owner, 2026-10-27).
    const nested = /\.field label \{([^}]*)\}/.exec(actionCss)?.[1] ?? "";
    expect(nested).toMatch(/font-size: 1em/);
  });

  it("sets a hint in the label size and muted, like the label above the field", () => {
    for (const source of [css, actionCss]) {
      const hint = /\.hint \{([^}]*)\}/.exec(source)?.[1] ?? "";
      expect(hint).toMatch(/font-size: 1em/);
      expect(hint).toMatch(/color: var\(--kv-fg-muted\)/);
    }
    expect(actionCss).toMatch(/<p id=\{ids\.hint\} class="hint">/);
  });

  it("reserves one line for the result, so the first message shifts nothing", () => {
    expect(css).toMatch(/\.result \{[^}]*min-height: 1\.4em/);
  });
});

describe("the control editor", () => {
  const VSCODE = "/home/u/.config/keyvo/profiles/vscode.toml";
  const GLOBAL_FILE = "/home/u/.config/keyvo/profiles/global.toml";
  const VOLUME: ControlFields = {
    cw: { type: "chord", keys: "KEY_VOLUMEUP" },
    ccw: { type: "chord", keys: "KEY_VOLUMEDOWN" },
    step: 5,
    label: "Volume",
  };
  const MUTE: ControlFields = { label: "Mute", action: { type: "chord", keys: "KEY_MUTE" } };
  const DIAL: ControlTarget = { profile: "vscode", control: "dial" };
  const BUTTON: ControlTarget = { profile: "vscode", control: "button:1" };

  /** An editor of controls held in memory, the global profile's bound,
   * vscode's as `own`; every write succeeds and answers what was there. */
  function controls(own: Partial<Record<Control, ControlFields>> = {}) {
    const files: Record<string, Partial<Record<Control, ControlFields>>> = {
      global: { dial: VOLUME, "button:1": MUTE },
      vscode: { ...own },
    };
    const path = (profile: string) => (profile === "global" ? GLOBAL_FILE : VSCODE);
    const editor = {
      ...fake().editor,
      control: vi.fn(({ profile, control }: ControlTarget) =>
        Promise.resolve({ path: path(profile), fields: files[profile]?.[control] ?? {} }),
      ),
      editControl: vi.fn(({ profile, control }: ControlTarget, fields: ControlFields) => {
        const table = files[profile] ?? {};
        const before = table[control] ?? {};
        table[control] = fields;
        files[profile] = table;
        return Promise.resolve({ path: path(profile), before, fields });
      }),
    };
    return editor satisfies Editor;
  }

  async function opened(editor: Editor, control: ControlTarget, props: Record<string, unknown> = {}) {
    const view = render(Inspector, { props: { editor, control, recipes: RECIPES, ...props } });
    await waitFor(() => expect(editor.control).toHaveBeenCalled());
    await settled();
    return view;
  }

  function select(name: string): HTMLSelectElement {
    return screen.getByRole("combobox", { name }) as HTMLSelectElement;
  }

  it("edits a rotary's two directions, step and label in its profile's [dialpad] table", async () => {
    const editor = controls({ dial: { label: "Indent", cw: { type: "chord", keys: "ctrl+]" } } });
    await opened(editor, DIAL);
    expect(screen.getByRole("heading", { name: "Dial" })).toBeInTheDocument();
    expect(screen.getByText(`${VSCODE}, the [dialpad] table`)).toBeInTheDocument();
    expect(field("Label")).toHaveValue("Indent");
    expect(select("Clockwise")).toHaveValue("chord");
    expect(field("Clockwise: Chord")).toHaveValue("ctrl+]");
    expect(select("Counter-clockwise")).toHaveValue("none");
    // The key editor's parts: no icon, no colour, no tile on a control.
    expect(screen.queryByRole("textbox", { name: "Color" })).toBeNull();
    await fireEvent.change(select("Counter-clockwise"), { target: { value: "chord" } });
    await type(field("Counter-clockwise: Chord"), "ctrl+[");
    await waitFor(() =>
      expect(editor.editControl).toHaveBeenLastCalledWith(DIAL, {
        cw: { type: "chord", keys: "ctrl+]" },
        ccw: { type: "chord", keys: "ctrl+[" },
        label: "Indent",
      }),
    );
    await type(field("Step"), "3");
    await waitFor(() =>
      expect(editor.editControl).toHaveBeenLastCalledWith(DIAL, expect.objectContaining({ step: 3 })),
    );
    expect(result()).toHaveTextContent(`Wrote Dial to ${VSCODE} at`);
  });

  it("says why a step or a direction cannot be written, under its field", async () => {
    const editor = controls({ dial: { label: "Indent" } });
    await opened(editor, DIAL);
    await type(field("Step"), "0");
    expect(await screen.findByRole("alert")).toHaveTextContent("The step 0 is not a whole number from 1 up.");
    expect(field("Step")).toHaveAttribute("aria-invalid", "true");
    await type(field("Step"), "");
    await fireEvent.change(select("Clockwise"), { target: { value: "chord" } });
    await type(field("Clockwise: Chord"), "ctrl+nokey");
    expect(await screen.findByRole("alert")).toHaveTextContent("nokey is not a key name.");
    expect(editor.editControl).not.toHaveBeenCalled();
  });

  it("edits a button's action and label with the key editor's action fields", async () => {
    const editor = controls({ "button:1": { label: "Save", action: { type: "chord", keys: "ctrl+s" } } });
    await opened(editor, BUTTON);
    expect(screen.getByRole("heading", { name: "Button 1: Chord" })).toBeInTheDocument();
    expect(action()).toHaveValue("chord");
    expect(field("Chord")).toHaveValue("ctrl+s");
    expect(screen.queryByRole("textbox", { name: "Step" })).toBeNull();
    await fireEvent.change(action(), { target: { value: "launch" } });
    await type(field("Command"), "kcalc");
    await fireEvent.click(waitSwitch());
    await waitFor(() =>
      expect(editor.editControl).toHaveBeenLastCalledWith(BUTTON, {
        label: "Save",
        action: { type: "launch", cmd: "kcalc", wait_focus: false },
      }),
    );
  });

  it("shows a global control the profile does not bind, and overrides it here", async () => {
    const editor = controls();
    await opened(editor, DIAL);
    expect(screen.getByText("from global")).toBeInTheDocument();
    expect(screen.getByText(`${GLOBAL_FILE}, the [dialpad] table`)).toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "Override here" }));
    await waitFor(() => expect(editor.editControl).toHaveBeenCalledWith(DIAL, VOLUME));
    await waitFor(() => expect(screen.queryByText("from global")).toBeNull());
    expect(field("Label")).toHaveValue("Volume");
  });

  it("undoes and redoes a control write with the fields it answered", async () => {
    const editor = controls({ "button:1": { label: "Save" } });
    await opened(editor, BUTTON);
    await type(field("Label"), "Store");
    await waitFor(() => expect(editor.editControl).toHaveBeenCalledWith(BUTTON, { label: "Store" }));
    await fireEvent.click(screen.getByRole("button", { name: "Undo" }));
    await waitFor(() => expect(editor.editControl).toHaveBeenLastCalledWith(BUTTON, { label: "Save" }));
    expect(field("Label")).toHaveValue("Save");
    expect(result()).toHaveTextContent("Undid the change to Button 1 at");
    await fireEvent.click(screen.getByRole("button", { name: "Redo" }));
    await waitFor(() => expect(editor.editControl).toHaveBeenLastCalledWith(BUTTON, { label: "Store" }));
  });

  it("clears the control, so the global one shows through again", async () => {
    const editor = controls({ "button:1": { label: "Save" } });
    await opened(editor, BUTTON);
    await fireEvent.click(screen.getByRole("button", { name: "Clear" }));
    await waitFor(() => expect(editor.editControl).toHaveBeenCalledWith(BUTTON, {}));
  });

  it("shows a button's on_fail read-only", async () => {
    const onFail = { type: "exec", cmd: "notify-send failed" } as const;
    const editor = controls({ "button:1": { label: "Save" } });
    editor.control.mockImplementation(() =>
      Promise.resolve({ path: VSCODE, fields: { label: "Save" }, on_fail: onFail }),
    );
    await opened(editor, BUTTON);
    expect(screen.getByText(/^Runs/, { selector: "p" })).toHaveTextContent(
      "Runs notify-send failed when this fails. Edit it in the file.",
    );
  });

  describe("an assigned catalogue entry", () => {
    it("sets a button's action and the preset's label", async () => {
      const editor = controls({ "button:1": { label: "Save" } });
      const view = await opened(editor, BUTTON);
      await view.rerender({ assignControl: { entry: "recipe:spectacle", target: BUTTON, seq: 1 } });
      await waitFor(() =>
        expect(editor.editControl).toHaveBeenCalledWith(BUTTON, {
          label: "Capture",
          action: { type: "exec", cmd: "spectacle --fullscreen" },
        }),
      );
    });

    it("sets the clockwise turn of a rotary, or the counter-clockwise one last focused", async () => {
      const ondirection = vi.fn();
      const editor = controls({ dial: { label: "Indent" } });
      const view = await opened(editor, DIAL, { ondirection });
      await view.rerender({ assignControl: { entry: "recipe:vscode", target: DIAL, seq: 1 } });
      await waitFor(() =>
        expect(editor.editControl).toHaveBeenLastCalledWith(DIAL, {
          cw: { type: "chord", keys: "ctrl+shift+p" },
          label: "Indent",
        }),
      );
      await fireEvent.focusIn(select("Counter-clockwise"));
      expect(ondirection).toHaveBeenLastCalledWith("ccw");
      await view.rerender({ assignControl: { entry: "recipe:konsole", target: DIAL, seq: 2 } });
      await waitFor(() =>
        expect(editor.editControl).toHaveBeenLastCalledWith(DIAL, {
          cw: { type: "chord", keys: "ctrl+shift+p" },
          ccw: { type: "chord", keys: "ctrl+shift+t" },
          label: "Indent",
        }),
      );
    });

    it("sets a tool's type with an empty field, written once the field is entered", async () => {
      const editor = controls({ dial: { label: "Indent" } });
      const view = await opened(editor, DIAL);
      await view.rerender({ assignControl: { entry: "tool:exec", target: DIAL, seq: 1 } });
      await waitFor(() => expect(select("Clockwise")).toHaveValue("exec"));
      expect(editor.editControl).not.toHaveBeenCalled();
      await type(field("Clockwise: Command"), "pactl set-sink-volume @DEFAULT_SINK@ +5%");
      await waitFor(() => expect(editor.editControl).toHaveBeenCalled());
    });
  });
});
