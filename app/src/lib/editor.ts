// The key editor's file side as the frontend sees it: the Tauri shell's
// `key` and `edit_key` commands over keyvo_client::Editor; tests pass a
// fake. And the editor's pure parts: the form, the undo history of the
// session, and the line a problem is shown as.
import { invoke } from "@tauri-apps/api/core";

import { checkChord, type ChordError } from "./chord";
import { t } from "./i18n";
import type { Action } from "./protocol";

/** The fields of a key the editor writes, as keyvo_core::edit::KeyFields;
 * an absent field is removed from the file. */
export interface KeyFields {
  icon?: string;
  label?: string;
  /** `#rrggbb`. */
  color?: string;
  action?: Action;
}

/** A key as the editor shows it (keyvo_client::editor::KeyView). */
export interface KeyView {
  /** The profile file. */
  path: string;
  fields: KeyFields;
  /** The line, from 1, on which the key's table starts in the file; absent
   * for a key the page does not define. */
  line?: number;
  /** The key's `on_fail` action, shown read-only: no write touches it. */
  on_fail?: Action;
}

/** What a write did (keyvo_client::editor::Edited). */
export interface Edited {
  path: string;
  /** The fields before the write, for undo. */
  before: KeyFields;
  fields: KeyFields;
  /** The line of the key's table in the written file. */
  line?: number;
  /** The copy of the file taken before the write. */
  snapshot?: string;
}

/** The key as the editor shows it after `edited`, its line included, and
 * the `on_fail` of `before`, the view of the same key the write kept it in. */
export function viewOf(edited: Edited, before?: KeyView): KeyView {
  const { path, fields, line } = edited;
  const onFail = before?.on_fail;
  return {
    path,
    fields,
    ...(line !== undefined ? { line } : {}),
    ...(onFail !== undefined ? { on_fail: onFail } : {}),
  };
}

/** What `action` does in a few words, as keyvo_core's Action::summary: a
 * chord's keys, the command line of `exec` and `launch`. */
export function summaryOf(action: Action): string {
  return action.type === "chord" ? action.keys : action.cmd;
}

/** Why a key cannot be read or written (keyvo_client::Problem): the file,
 * line and column when the problem has a place. */
export interface Problem {
  message: string;
  path?: string;
  line?: number;
  column?: number;
}

/** Which key the editor edits. */
export interface Target {
  profile: string;
  /** Counted from 1. */
  page: number;
  key: number;
}

export interface Editor {
  /** The key's file and fields; rejects with a Problem. */
  key(target: Target): Promise<KeyView>;
  /** Writes `fields` into the key after a snapshot of the file; rejects
   * with a Problem and leaves the file as it was. */
  editKey(target: Target, fields: KeyFields): Promise<Edited>;
}

/** The editor through the Tauri shell's commands. */
export const tauriEditor: Editor = {
  key: ({ profile, page, key }) => invoke<KeyView>("key", { profile, page, key }),
  editKey: ({ profile, page, key }, fields) =>
    invoke<Edited>("edit_key", { profile, page, key, fields }),
};

/** A rejection of the editor as a Problem. */
export function problemOf(error: unknown): Problem {
  if (typeof error === "object" && error !== null && "message" in error) {
    const { message, path, line, column } = error as Record<string, unknown>;
    return {
      message: String(message),
      ...(typeof path === "string" ? { path } : {}),
      ...(typeof line === "number" ? { line } : {}),
      ...(typeof column === "number" ? { column } : {}),
    };
  }
  return { message: String(error) };
}

/** One line: `file:line:column: message`, as the daemon and the CLI write
 * a diagnostic. */
export function describe(problem: Problem): string {
  if (problem.path === undefined) return problem.message;
  const place = [problem.path, problem.line, problem.column]
    .filter((part) => part !== undefined)
    .join(":");
  return `${place}: ${problem.message}`;
}

export type ActionType = "none" | "chord" | "exec" | "launch";

/** The editor's form: every field as the text in its input. */
export interface Form {
  label: string;
  color: string;
  icon: string;
  actionType: ActionType;
  /** The chord's keys or the command, by `actionType`. */
  action: string;
  /** A launch's window class; empty for none. */
  windowClass: string;
  /** Whether a launch waits for its window. */
  waitFocus: boolean;
}

/** The action fields of the form for `action`. */
export function actionForm(
  action: Action | undefined,
): Pick<Form, "actionType" | "action" | "windowClass" | "waitFocus"> {
  return {
    actionType: action?.type ?? "none",
    action: action === undefined ? "" : action.type === "chord" ? action.keys : action.cmd,
    windowClass: action?.type === "launch" ? (action.class ?? "") : "",
    waitFocus: action?.type === "launch" ? (action.wait_focus ?? true) : true,
  };
}

export function toForm(fields: KeyFields): Form {
  return {
    label: fields.label ?? "",
    color: fields.color ?? "",
    icon: fields.icon ?? "",
    ...actionForm(fields.action),
  };
}

const COLOR = /^#[0-9a-fA-F]{6}$/;

/** `text` as a `#rrggbb` colour in lower case; undefined when it is not one. */
export function colorOf(text: string): string | undefined {
  const color = text.trim();
  return COLOR.test(color) ? color.toLowerCase() : undefined;
}

/** Why a chord cannot be written, naming the wrong token. */
function chordMessage(wrong: ChordError): string {
  switch (wrong.error) {
    case "empty":
      return t("editor.noKeys");
    case "missing_key":
      return t("editor.chordMissingKey");
    case "unknown_key":
      return t("editor.chordUnknownKey", { token: wrong.token });
    case "not_a_modifier":
      return t("editor.chordNotModifier", { token: wrong.token });
    case "expected_separator":
      return t("editor.chordSeparator", { token: wrong.token });
  }
}

/** The form field a message of `fromForm` belongs under. */
export type FormField = "color" | "action";

/** The fields `form` says, or why it cannot be written yet and in which
 * field: an empty input is an absent field. */
export function fromForm(
  form: Form,
): { fields: KeyFields } | { invalid: string; field: FormField } {
  const fields: KeyFields = {};
  if (form.icon.trim() !== "") fields.icon = form.icon.trim();
  if (form.label !== "") fields.label = form.label;
  const color = form.color.trim();
  if (color !== "") {
    const valid = colorOf(color);
    if (valid === undefined) return { invalid: t("editor.badColor", { color }), field: "color" };
    fields.color = valid;
  }
  const action = form.action.trim();
  switch (form.actionType) {
    case "none":
      break;
    case "chord":
      if (action === "") return { invalid: t("editor.noKeys"), field: "action" };
      {
        const wrong = checkChord(action);
        if (wrong !== undefined) return { invalid: chordMessage(wrong), field: "action" };
      }
      fields.action = { type: "chord", keys: action };
      break;
    case "exec":
      if (action === "") return { invalid: t("editor.noCommand"), field: "action" };
      fields.action = { type: "exec", cmd: action };
      break;
    case "launch": {
      if (action === "") return { invalid: t("editor.noCommand"), field: "action" };
      const launch: Action = { type: "launch", cmd: action };
      const windowClass = form.windowClass.trim();
      if (windowClass !== "") launch.class = windowClass;
      if (!form.waitFocus) launch.wait_focus = false;
      fields.action = launch;
      break;
    }
  }
  return { fields };
}

export function sameFields(a: KeyFields, b: KeyFields): boolean {
  const action = (fields: KeyFields) => {
    const action = fields.action;
    if (action === undefined) return "";
    if (action.type === "chord") return `chord:${action.keys}`;
    if (action.type === "exec") return `exec:${action.cmd}`;
    return `launch:${action.cmd}:${action.class ?? ""}:${action.wait_focus ?? true}`;
  };
  return (
    a.icon === b.icon &&
    a.label === b.label &&
    a.color?.toLowerCase() === b.color?.toLowerCase() &&
    action(a) === action(b)
  );
}

export function sameTarget(a: Target | undefined, b: Target | undefined): boolean {
  return a?.profile === b?.profile && a?.page === b?.page && a?.key === b?.key;
}

/** One write of the session, undone by writing `before` again. */
export interface Change {
  target: Target;
  before: KeyFields;
  after: KeyFields;
}

/** The session's writes: `done` oldest first, `undone` the ones undo took
 * back, the next to redo last. */
export interface History {
  done: Change[];
  undone: Change[];
}

export function emptyHistory(): History {
  return { done: [], undone: [] };
}

/** `history` after a new write: nothing is left to redo. */
export function recorded(history: History, change: Change): History {
  return { done: [...history.done, change], undone: [] };
}

/** `history` after the last write was undone. */
export function afterUndo(history: History): History {
  const change = history.done.at(-1);
  if (change === undefined) return history;
  return { done: history.done.slice(0, -1), undone: [...history.undone, change] };
}

/** `history` after the last undone write was written again. */
export function afterRedo(history: History): History {
  const change = history.undone.at(-1);
  if (change === undefined) return history;
  return { done: [...history.done, change], undone: history.undone.slice(0, -1) };
}
