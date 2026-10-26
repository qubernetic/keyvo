// The key editor over the mock store, answering like the Tauri shell's
// key and edit_key.
import type { Editor, KeyFields, Target } from "../editor";
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

export function mockEditor(store: Store): Editor {
  return {
    key: (target) =>
      attempt(() => ({
        path: store.path(target.profile),
        fields: structuredClone(page(store, target).keys.get(target.key) ?? {}),
      })),
    editKey: (target, fields) =>
      attempt(() => {
        const keys = page(store, target).keys;
        const before: KeyFields = keys.get(target.key) ?? {};
        keys.set(target.key, structuredClone(fields));
        return { path: store.path(target.profile), before, fields: structuredClone(fields) };
      }),
  };
}
