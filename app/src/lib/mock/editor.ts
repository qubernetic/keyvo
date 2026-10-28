// The key editor over the mock store, answering like the Tauri shell's
// key and edit_key.
import type { Editor, KeyFields, Problem, Target } from "../editor";
import { attempt } from "./profiles";
import { problem, type MockPage, type Store } from "./store";

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
        keys.set(target.key, structuredClone(fields));
        return {
          path: store.path(target.profile),
          before,
          fields: structuredClone(fields),
          ...line(store, target),
        };
      }),
  };
}
