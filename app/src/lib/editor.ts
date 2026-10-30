// The key editor's file side as the frontend sees it: the Tauri shell's
// `key` and `edit_key` commands, and `control` and `edit_control` for the
// Dialpad controls, over keyvo_client::Editor; tests pass a fake. And the
// editor's pure parts: the key's and the control's form, the undo history
// of the session, and the line a problem is shown as.
import { invoke } from "@tauri-apps/api/core";

import { checkChord, type ChordError } from "./chord";
import { isRoller } from "./dialpad";
import { t } from "./i18n";
import type { Action, Control } from "./protocol";

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

/** The fields of a Dialpad control the editor writes, as
 * keyvo_client::editor::ControlFields: `cw`, `ccw`, `step` and `label`
 * for the dial and the roller (keyvo_core::edit::RotaryFields), `label`
 * and `action` for a button (ButtonFields). An absent field is removed
 * from the file, a step of 1 is written as absent, and a control without
 * any field is removed, so the global profile's binding shows through. */
export interface ControlFields {
  /** What a clockwise turn runs. */
  cw?: Action;
  /** What a counter-clockwise turn runs. */
  ccw?: Action;
  /** Detents per run; absent for the default 1. */
  step?: number;
  /** What the Dialpad panel shows; no LCD shows it. */
  label?: string;
  /** What a button's press runs. */
  action?: Action;
}

/** A Dialpad control as the editor shows it
 * (keyvo_client::editor::ControlView): the profile's own binding, never
 * the global one under it. */
export interface ControlView {
  /** The profile file. */
  path: string;
  /** All absent when the profile's `[dialpad]` table does not bind it. */
  fields: ControlFields;
  /** A button's `on_fail`, shown read-only like a key's. */
  on_fail?: Action;
}

/** What a write of a Dialpad control did
 * (keyvo_client::editor::ControlEdited). */
export interface ControlEdited {
  path: string;
  /** The fields before the write, for undo. */
  before: ControlFields;
  fields: ControlFields;
  /** The copy of the file taken before the write. */
  snapshot?: string;
}

/** Which Dialpad control the editor edits: one per profile, not per page. */
export interface ControlTarget {
  profile: string;
  control: Control;
}

export interface Editor {
  /** The key's file and fields; rejects with a Problem. */
  key(target: Target): Promise<KeyView>;
  /** Writes `fields` into the key after a snapshot of the file; rejects
   * with a Problem and leaves the file as it was. */
  editKey(target: Target, fields: KeyFields): Promise<Edited>;
  /** The Dialpad control's file and fields; rejects with a Problem. */
  control(target: ControlTarget): Promise<ControlView>;
  /** Writes `fields` into the Dialpad control after a snapshot of the
   * file; rejects with a Problem and leaves the file as it was. */
  editControl(target: ControlTarget, fields: ControlFields): Promise<ControlEdited>;
}

/** The editor through the Tauri shell's commands. */
export const tauriEditor: Editor = {
  key: ({ profile, page, key }) => invoke<KeyView>("key", { profile, page, key }),
  editKey: ({ profile, page, key }, fields) =>
    invoke<Edited>("edit_key", { profile, page, key, fields }),
  control: ({ profile, control }) => invoke<ControlView>("control", { profile, control }),
  editControl: ({ profile, control }, fields) =>
    invoke<ControlEdited>("edit_control", { profile, control, fields }),
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

/** The action part of a form: the key's action, a direction of a rotary
 * or a Dialpad button's action, edited by ActionFields.svelte. */
export type ActionPart = Pick<Form, "actionType" | "action" | "windowClass" | "waitFocus">;

/** The action fields of the form for `action`. */
export function actionForm(action: Action | undefined): ActionPart {
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
  const action = actionOf(form);
  if ("invalid" in action) return { invalid: action.invalid, field: "action" };
  if (action.action !== undefined) fields.action = action.action;
  return { fields };
}

/** The action `part` says, none for no action, or why it cannot be
 * written yet: an empty field is never written. */
export function actionOf(part: ActionPart): { action?: Action } | { invalid: string } {
  const action = part.action.trim();
  switch (part.actionType) {
    case "none":
      return {};
    case "chord": {
      if (action === "") return { invalid: t("editor.noKeys") };
      const wrong = checkChord(action);
      if (wrong !== undefined) return { invalid: chordMessage(wrong) };
      return { action: { type: "chord", keys: action } };
    }
    case "exec":
      if (action === "") return { invalid: t("editor.noCommand") };
      return { action: { type: "exec", cmd: action } };
    case "launch": {
      if (action === "") return { invalid: t("editor.noCommand") };
      const launch: Action = { type: "launch", cmd: action };
      const windowClass = part.windowClass.trim();
      if (windowClass !== "") launch.class = windowClass;
      if (!part.waitFocus) launch.wait_focus = false;
      return { action: launch };
    }
  }
}

/** `action` as one comparable string, the launch's defaults filled in. */
function actionKey(action: Action | undefined): string {
  if (action === undefined) return "";
  if (action.type === "chord") return `chord:${action.keys}`;
  if (action.type === "exec") return `exec:${action.cmd}`;
  return `launch:${action.cmd}:${action.class ?? ""}:${action.wait_focus ?? true}`;
}

export function sameFields(a: KeyFields, b: KeyFields): boolean {
  return (
    a.icon === b.icon &&
    a.label === b.label &&
    a.color?.toLowerCase() === b.color?.toLowerCase() &&
    actionKey(a.action) === actionKey(b.action)
  );
}

export function sameTarget(a: Target | undefined, b: Target | undefined): boolean {
  return a?.profile === b?.profile && a?.page === b?.page && a?.key === b?.key;
}

/** The control editor's form: the label and the step as typed, both
 * directions of a rotary and a button's action as the key editor's action
 * fields. A control uses the parts of its kind. */
export interface ControlForm {
  label: string;
  step: string;
  cw: ActionPart;
  ccw: ActionPart;
  action: ActionPart;
}

export function toControlForm(fields: ControlFields): ControlForm {
  return {
    label: fields.label ?? "",
    step: fields.step === undefined ? "" : String(fields.step),
    cw: actionForm(fields.cw),
    ccw: actionForm(fields.ccw),
    action: actionForm(fields.action),
  };
}

/** The control form field a message of `fromControlForm` belongs under. */
export type ControlField = "cw" | "ccw" | "step" | "action";

const STEP = /^\d+$/;

/** The fields of `control`'s kind `form` says, or why it cannot be
 * written yet and in which field. An empty step or a step of 1 is the
 * default, written as absent. */
export function fromControlForm(
  control: Control,
  form: ControlForm,
): { fields: ControlFields } | { invalid: string; field: ControlField } {
  const fields: ControlFields = {};
  const parts: ("cw" | "ccw" | "action")[] = isRoller(control) ? ["cw", "ccw"] : ["action"];
  for (const part of parts) {
    const action = actionOf(form[part]);
    if ("invalid" in action) return { invalid: action.invalid, field: part };
    if (action.action !== undefined) fields[part] = action.action;
  }
  if (isRoller(control)) {
    const step = form.step.trim();
    if (step !== "") {
      if (!STEP.test(step) || Number(step) < 1) {
        return { invalid: t("editor.badStep", { step }), field: "step" };
      }
      if (Number(step) !== 1) fields.step = Number(step);
    }
  }
  if (form.label !== "") fields.label = form.label;
  return { fields };
}

export function sameControlFields(a: ControlFields, b: ControlFields): boolean {
  return (
    a.label === b.label &&
    a.step === b.step &&
    actionKey(a.cw) === actionKey(b.cw) &&
    actionKey(a.ccw) === actionKey(b.ccw) &&
    actionKey(a.action) === actionKey(b.action)
  );
}

export function sameControl(a: ControlTarget | undefined, b: ControlTarget | undefined): boolean {
  return a !== undefined && b !== undefined && a.profile === b.profile && a.control === b.control;
}

/** One write of the session, undone by writing `before` again. */
export interface Change {
  target: Target;
  before: KeyFields;
  after: KeyFields;
}

/** One write of a Dialpad control, undone the same way. */
export interface ControlChange {
  target: ControlTarget;
  before: ControlFields;
  after: ControlFields;
}

/** The session's writes: `done` oldest first, `undone` the ones undo took
 * back, the next to redo last. */
export interface History<C = Change> {
  done: C[];
  undone: C[];
}

export function emptyHistory<C = Change>(): History<C> {
  return { done: [], undone: [] };
}

/** `history` after a new write: nothing is left to redo. */
export function recorded<C>(history: History<C>, change: C): History<C> {
  return { done: [...history.done, change], undone: [] };
}

/** `history` after the last write was undone. */
export function afterUndo<C>(history: History<C>): History<C> {
  const change = history.done.at(-1);
  if (change === undefined) return history;
  return { done: history.done.slice(0, -1), undone: [...history.undone, change] };
}

/** `history` after the last undone write was written again. */
export function afterRedo<C>(history: History<C>): History<C> {
  const change = history.undone.at(-1);
  if (change === undefined) return history;
  return { done: [...history.done, change], undone: history.undone.slice(0, -1) };
}
