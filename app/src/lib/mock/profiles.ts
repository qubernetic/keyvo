// The Profiles screen's file side over the mock store, answering like the
// Tauri shell's profile_list, profile, edit_profile, edit_page,
// restore_profile and open_profile.
import type { PageEdit, PageEdited, ProfileFields, ProfileFiles, ProfileSummary } from "../profiles";
import { contextPage, problem, type MockPage, type MockProfile, type Store } from "./store";

/** Runs `answer` as a promise, a thrown Problem as its rejection. */
export function attempt<T>(answer: () => T): Promise<T> {
  try {
    return Promise.resolve(answer());
  } catch (error) {
    return Promise.reject(error as Error);
  }
}

/** Page `page` (from 1) of `profile`; throws a Problem when there is none. */
function pageOf(profile: MockProfile, page: number): MockPage {
  const found = profile.pages[page - 1];
  if (found === undefined) throw problem(`profile \`${profile.id}\` has no page ${page}`);
  return found;
}

/** `edit` applied to `profile`'s pages, as keyvo_core::edit::PageEdit
 * writes it: the edited page's number after it and the edit that undoes
 * it, none for a delete. Throws a Problem for an edit the editor refuses:
 * the context page switched off, the only enabled page deleted. */
function applied(profile: MockProfile, edit: PageEdit): Pick<PageEdited, "page" | "undo"> {
  const pages = profile.pages;
  switch (edit.op) {
    case "add": {
      pages.push({ name: edit.name ?? null, enabled: true, keys: new Map(), onFail: new Map() });
      return { page: pages.length, undo: { op: "delete", page: pages.length } };
    }
    case "rename": {
      const found = pageOf(profile, edit.page);
      const before = found.name;
      found.name = edit.name ?? null;
      return {
        page: edit.page,
        undo: { op: "rename", page: edit.page, ...(before === null ? {} : { name: before }) },
      };
    }
    case "move": {
      const found = pageOf(profile, edit.page);
      if (!Number.isInteger(edit.to) || edit.to < 1 || edit.to > pages.length) {
        throw problem(`profile \`${profile.id}\` has no page ${edit.to} to move to`);
      }
      pages.splice(edit.page - 1, 1);
      pages.splice(edit.to - 1, 0, found);
      return { page: edit.to, undo: { op: "move", page: edit.to, to: edit.page } };
    }
    case "enable": {
      const found = pageOf(profile, edit.page);
      if (!edit.enabled && edit.page === contextPage(profile)) {
        throw problem(`page ${edit.page} is the context page of \`${profile.id}\`; it stays enabled`);
      }
      const before = found.enabled;
      found.enabled = edit.enabled;
      return { page: edit.page, undo: { op: "enable", page: edit.page, enabled: before } };
    }
    case "delete": {
      const found = pageOf(profile, edit.page);
      if (found.enabled && pages.filter((page) => page.enabled).length === 1) {
        throw problem(`page ${edit.page} is the only enabled page of \`${profile.id}\`; it stays`);
      }
      pages.splice(edit.page - 1, 1);
      return {};
    }
  }
}

export function mockProfiles(
  store: Store,
  log: (line: string) => void = (line) => console.info(line),
): ProfileFiles {
  return {
    list: () =>
      attempt(() =>
        store.all().map((profile): ProfileSummary => {
          const disabled = profile.pages.flatMap((page, at) => (page.enabled ? [] : [at + 1]));
          return {
            id: profile.id,
            name: profile.fields.name,
            ...(profile.fields.accent === undefined ? {} : { accent: profile.fields.accent }),
            priority: profile.priority,
            match: structuredClone(profile.match),
            pages: profile.pages.map((page) => page.name),
            ...(disabled.length === 0 ? {} : { disabled }),
            ...(profile.fields.global_pages === false ? { global_pages: false } : {}),
            path: store.path(profile.id),
            file: true,
          };
        }),
      ),
    profile: (id) =>
      attempt(() => ({ path: store.path(id), fields: structuredClone(store.get(id).fields) })),
    editProfile: (id, fields) =>
      attempt(() => {
        const profile = store.get(id);
        const before = profile.fields;
        const after: ProfileFields = { name: fields.name };
        if (fields.accent !== undefined) after.accent = fields.accent;
        if (fields.global_pages === false) after.global_pages = false;
        profile.fields = after;
        return { path: store.path(id), before, fields: structuredClone(after) };
      }),
    editPage: (id, edit) =>
      attempt(() => {
        const profile = store.get(id);
        // A refused edit leaves the profile as it was.
        const done = applied(structuredClone(profile), edit);
        const snapshot = store.snapshot(id);
        applied(profile, edit);
        store.repaged(id);
        return { path: store.path(id), edit: structuredClone(edit), ...done, snapshot };
      }),
    restore: (id, snapshot) =>
      attempt(() => {
        const before = store.snapshot(id);
        store.restore(id, snapshot);
        return { path: store.path(id), snapshot: before };
      }),
    open: (id) =>
      attempt(() => {
        store.get(id);
        log(`mock: open ${store.path(id)}`);
      }),
  };
}
