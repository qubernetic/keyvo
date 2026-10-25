// The key editor against a fake editor that holds the profile file of the
// shared steps in tests/protocol-fixtures/editor/: the file a step's fields
// give is the file keyvo_client::Editor writes for them
// (crates/keyvo-client/tests/editor.rs), so each test asserts the bytes on
// disk after the GUI's edit.
import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { isDeepStrictEqual } from "node:util";
import { describe, expect, it, vi } from "vitest";

import KeyEditor from "./KeyEditor.svelte";
import type { Edited, Editor, KeyFields, KeyView, Target } from "./lib/editor";
import { editorFile, editorSteps, nth } from "./lib/fixtures";

const STEPS = editorSteps();
const TARGET: Target = { profile: STEPS.profile, page: STEPS.page, key: STEPS.key };
const PATH = `/home/u/.config/keyvo/${STEPS.path}`;

/** An editor on the fixture's file: a write of the fields one step away
 * moves the file to that step; anything else is refused. */
function fake(overrides: Partial<Editor> = {}) {
  const states = [STEPS.fields, ...STEPS.steps.map((step) => step.fields)];
  const files = [STEPS.file, ...STEPS.steps.map((step) => step.file)];
  let at = 0;
  const editor = {
    key: vi.fn(
      (_target: Target): Promise<KeyView> => Promise.resolve({ path: PATH, fields: nth(states, at) }),
    ),
    editKey: vi.fn((_target: Target, fields: KeyFields): Promise<Edited> => {
      const next = states.findIndex((state) => isDeepStrictEqual(state, fields));
      if (next === -1 || Math.abs(next - at) !== 1) {
        return Promise.reject({ message: `no step writes ${JSON.stringify(fields)}` });
      }
      const before = nth(states, at);
      at = next;
      return Promise.resolve({ path: PATH, before, fields, snapshot: "/s/1-terminal.toml" });
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

function field(name: string): HTMLInputElement {
  return screen.getByRole("textbox", { name }) as HTMLInputElement;
}

async function type(input: HTMLElement, value: string) {
  await fireEvent.input(input, { target: { value } });
  await fireEvent.change(input);
}

/** The editor mounted on `target`, the fixture's key unless `null`. */
async function mounted(editor = fake(), target: Target | null = TARGET) {
  const onedited = vi.fn();
  const view = render(KeyEditor, {
    props: { editor: editor.editor, target: target ?? undefined, onedited },
  });
  if (target !== null) {
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
      const action = step.fields.action;
      if (action?.type !== "exec") throw new Error("the fixture's action step is an exec");
      await fireEvent.change(screen.getByRole("combobox", { name: "Action" }), {
        target: { value: "exec" },
      });
      await type(field("Command"), action.cmd);
    }
  }
}

describe("KeyEditor", () => {
  it("shows the key's fields and its file", async () => {
    const editor = await mounted();
    expect(editor.editor.key).toHaveBeenCalledWith(TARGET);
    expect(screen.getByRole("heading", { name: "Key 2" })).toBeInTheDocument();
    expect(screen.getByText(PATH)).toBeInTheDocument();
    expect(field("Color")).toHaveValue("");
    expect(field("Icon")).toHaveValue("");
    expect(screen.getByRole("combobox", { name: "Action" })).toHaveValue("chord");
    expect(field("Chord")).toHaveValue("ctrl+shift+c");
  });

  it("writes each change at once and the file is the fixture's", async () => {
    const editor = await mounted();
    await type(field("Label"), "Paste");
    await waitFor(() => expect(editor.file).toBe(editorFile(nth(STEPS.steps, 0).file)));
    expect(editor.editor.editKey).toHaveBeenLastCalledWith(TARGET, nth(STEPS.steps, 0).fields);
    expect(editor.onedited).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("status", { name: "Editor result" })).toHaveTextContent(
      `Wrote key 2 to ${PATH}`,
    );
  });

  it("changes the action type without writing until the command is entered", async () => {
    const editor = await mounted();
    await type(field("Label"), "Paste");
    await waitFor(() => expect(editor.editor.editKey).toHaveBeenCalledTimes(1));
    await fireEvent.change(screen.getByRole("combobox", { name: "Action" }), {
      target: { value: "exec" },
    });
    expect(field("Command")).toHaveValue("");
    expect(screen.getByText("Enter the command to write it.")).toBeInTheDocument();
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
    expect(screen.getByRole("status", { name: "Editor result" })).toHaveTextContent(
      "Undid the change to key 2",
    );
  });

  it("leaves Ctrl+Z to a text field that has the focus", async () => {
    const editor = await mounted();
    await type(field("Label"), "Paste");
    await waitFor(() => expect(editor.editor.editKey).toHaveBeenCalledTimes(1));
    await fireEvent.keyDown(field("Icon"), { key: "z", ctrlKey: true });
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

  it("refuses a color that is not #rrggbb without writing", async () => {
    const editor = await mounted();
    await type(field("Color"), "teal");
    expect(screen.getByRole("alert")).toHaveTextContent("The color teal is not written #rrggbb.");
    expect(editor.editor.editKey).not.toHaveBeenCalled();
  });

  it("removes the action when it is set to none", async () => {
    const editKey = vi.fn((_target: Target, fields: KeyFields) =>
      Promise.resolve({ path: PATH, before: STEPS.fields, fields }),
    );
    await mounted(fake({ editKey }));
    await fireEvent.change(screen.getByRole("combobox", { name: "Action" }), {
      target: { value: "none" },
    });
    await waitFor(() => expect(editKey).toHaveBeenCalledWith(TARGET, { label: "Copy" }));
    expect(screen.queryByRole("textbox", { name: "Chord" })).not.toBeInTheDocument();
  });

  it("shows a schema error of the file with its file, line and column", async () => {
    const problem = { message: "invalid type: integer `7`, expected a string", path: PATH, line: 4, column: 8 };
    render(KeyEditor, {
      props: { editor: fake({ key: () => Promise.reject(problem) }).editor, target: TARGET },
    });
    expect(await screen.findByRole("alert")).toHaveTextContent(
      `${PATH}:4:8: invalid type: integer \`7\`, expected a string`,
    );
    expect(screen.queryByRole("textbox", { name: "Label" })).not.toBeInTheDocument();
  });

  it("shows a refused write and keeps the history as it was", async () => {
    const problem = { message: "invalid key fields: unknown key name `hyperr`" };
    const editor = await mounted(fake({ editKey: () => Promise.reject(problem) }));
    await type(field("Label"), "Paste");
    expect(await screen.findByRole("alert")).toHaveTextContent(problem.message);
    expect(screen.getByRole("button", { name: "Undo" })).toBeDisabled();
    expect(editor.onedited).not.toHaveBeenCalled();
  });

  it("asks for a key until there is one", async () => {
    await mounted(fake(), null);
    expect(screen.getByText("Select a key on the mirror to edit it.")).toBeInTheDocument();
    expect(screen.queryByRole("textbox", { name: "Label" })).not.toBeInTheDocument();
  });

  it("loads another key when the target changes", async () => {
    const editor = await mounted();
    await editor.view.rerender({ target: { ...TARGET, key: 5 } });
    await waitFor(() => expect(editor.editor.key).toHaveBeenLastCalledWith({ ...TARGET, key: 5 }));
    expect(screen.getByRole("heading", { name: "Key 5" })).toBeInTheDocument();
  });

  it("reads the key again on a new revision but keeps a field being edited", async () => {
    const editor = await mounted();
    await fireEvent.input(field("Icon"), { target: { value: "lucide:clipboard" } });
    await editor.view.rerender({ revision: 1 });
    await waitFor(() => expect(editor.editor.key).toHaveBeenCalledTimes(2));
    expect(field("Icon")).toHaveValue("lucide:clipboard");
  });
});
