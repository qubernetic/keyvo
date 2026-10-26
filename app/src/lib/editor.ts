// The key editor's file side as the frontend sees it: the Tauri shell's
// `key` and `edit_key` commands over keyvo_client::Editor; tests pass a
// fake. And the editor's pure parts: the form, the undo history of the
// session, and the line a problem is shown as.
import { invoke } from "@tauri-apps/api/core";

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

/** The key as the editor shows it after `edited`, its line included. */
export function viewOf(edited: Edited): KeyView {
  const { path, fields, line } = edited;
  return { path, fields, ...(line !== undefined ? { line } : {}) };
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

export type ActionType = "none" | "chord" | "exec";

/** The editor's form: every field as the text in its input. */
export interface Form {
  label: string;
  color: string;
  icon: string;
  actionType: ActionType;
  /** The chord's keys or the command, by `actionType`. */
  action: string;
}

export function toForm(fields: KeyFields): Form {
  const action = fields.action;
  return {
    label: fields.label ?? "",
    color: fields.color ?? "",
    icon: fields.icon ?? "",
    actionType: action?.type ?? "none",
    action: action === undefined ? "" : action.type === "chord" ? action.keys : action.cmd,
  };
}

const COLOR = /^#[0-9a-fA-F]{6}$/;

/** The fields `form` says, or why it cannot be written yet: an empty input
 * is an absent field. */
export function fromForm(form: Form): { fields: KeyFields } | { invalid: string } {
  const fields: KeyFields = {};
  if (form.icon.trim() !== "") fields.icon = form.icon.trim();
  if (form.label !== "") fields.label = form.label;
  const color = form.color.trim();
  if (color !== "") {
    if (!COLOR.test(color)) return { invalid: t("editor.badColor", { color }) };
    fields.color = color.toLowerCase();
  }
  const action = form.action.trim();
  switch (form.actionType) {
    case "none":
      break;
    case "chord":
      if (action === "") return { invalid: t("editor.noKeys") };
      fields.action = { type: "chord", keys: action };
      break;
    case "exec":
      if (action === "") return { invalid: t("editor.noCommand") };
      fields.action = { type: "exec", cmd: action };
      break;
  }
  return { fields };
}

export function sameFields(a: KeyFields, b: KeyFields): boolean {
  const action = (fields: KeyFields) =>
    fields.action === undefined
      ? ""
      : `${fields.action.type}:${fields.action.type === "chord" ? fields.action.keys : fields.action.cmd}`;
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
