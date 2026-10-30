// The key editor over the mock store, answering like the Tauri shell's
// key and edit_key, and control and edit_control for the Dialpad.
import type { ControlFields, ControlTarget, Editor, KeyFields, Problem, Target } from "../editor";
import { controlOf, type Control } from "../protocol";
import { attempt } from "./profiles";
import { problem, type MockPage, type MockProfile, type Store } from "./store";

/** The key slots of a profile (keyvo_core::profile::KeySlot). */
const KEYS = 9;

function page(store: Store, { profile, page, key }: Target): MockPage {
  const found = store.page(profile, page);
  if (!Number.isInteger(key) || key < 1 || key > KEYS) {
    throw problem(`key ${key} is not a key of the Keypad (1 to ${KEYS})`);
  }
  return found;
}

/** The key's `line`, as keyvo_client answers it: absent for a key the
 * page does not define. */
function line(store: Store, { profile, page, key }: Target): { line?: number } {
  const found = store.keyLine(profile, page, key);
  return found === undefined ? {} : { line: found };
}

export interface MockEditorOptions {
  /** Every write is refused with a Problem placed on the key's line
   * (`?mock-error=write`), as keyvo_client refuses a write the profile
   * would not load after. */
  refuseWrites?: boolean;
}

function refused(store: Store, target: Target): Problem {
  return {
    ...problem("the mock refuses every write (?mock-error=write)"),
    path: store.path(target.profile),
    line: store.keyLine(target.profile, target.page, target.key) ?? 1,
    column: 1,
  };
}

/** The profile and the control of `target`; throws a Problem for an
 * unknown one, as keyvo_core::dialpad::Control refuses to parse it. */
function dialpad(store: Store, target: ControlTarget): [MockProfile, Control] {
  const profile = store.get(target.profile);
  const control = controlOf(target.control);
  if (control === undefined) throw problem(`${target.control} is not a control of the Dialpad`);
  return [profile, control];
}

/** `fields` as keyvo_core::edit writes them: a step of 1 is absent. */
function normalized(fields: ControlFields): ControlFields {
  const { step, ...rest } = structuredClone(fields);
  return step === undefined || step === 1 ? rest : { ...rest, step };
}

/** A key's `fields` as keyvo_core::edit writes them: each rotary's step of
 * 1 absent; a `modifier` without a `dial` or a `roller` refused, as the
 * profile would not load after. */
function keyFields(fields: KeyFields): KeyFields {
  const out = structuredClone(fields);
  if (out.modifier !== undefined && out.dial === undefined && out.roller === undefined) {
    throw problem("`modifier` needs a `dial` or `roller` table next to it");
  }
  if (out.dial !== undefined) out.dial = normalized(out.dial);
  if (out.roller !== undefined) out.roller = normalized(out.roller);
  return out;
}

export function mockEditor(store: Store, options: MockEditorOptions = {}): Editor {
  return {
    key: (target) =>
      attempt(() => {
        const found = page(store, target);
        const onFail = found.onFail.get(target.key);
        return {
          path: store.path(target.profile),
          fields: structuredClone(found.keys.get(target.key) ?? {}),
          ...line(store, target),
          ...(onFail === undefined ? {} : { on_fail: structuredClone(onFail) }),
        };
      }),
    editKey: (target, fields) =>
      attempt(() => {
        const keys = page(store, target).keys;
        if (options.refuseWrites === true) throw refused(store, target);
        const before: KeyFields = keys.get(target.key) ?? {};
        const after = keyFields(fields);
        keys.set(target.key, after);
        return {
          path: store.path(target.profile),
          before,
          fields: structuredClone(after),
          ...line(store, target),
        };
      }),
    control: (target) =>
      attempt(() => {
        const [profile, control] = dialpad(store, target);
        const onFail = profile.dialpadOnFail.get(control);
        return {
          path: store.path(profile.id),
          fields: structuredClone(profile.dialpad.get(control) ?? {}),
          ...(onFail === undefined ? {} : { on_fail: structuredClone(onFail) }),
        };
      }),
    editControl: (target, fields) =>
      attempt(() => {
        const [profile, control] = dialpad(store, target);
        if (options.refuseWrites === true) {
          throw { ...problem("the mock refuses every write (?mock-error=write)"), path: store.path(profile.id) };
        }
        const before = profile.dialpad.get(control) ?? {};
        const after = normalized(fields);
        // A control without any field leaves the table: the global one shows through.
        if (Object.keys(after).length === 0) profile.dialpad.delete(control);
        else profile.dialpad.set(control, after);
        return { path: store.path(profile.id), before, fields: structuredClone(after) };
      }),
  };
}
