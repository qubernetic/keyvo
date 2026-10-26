// The Profiles screen's file side over the mock store, answering like the
// Tauri shell's profile_list, profile, edit_profile and open_profile.
import type { ProfileFields, ProfileFiles, ProfileSummary } from "../profiles";
import type { Store } from "./store";

/** Runs `answer` as a promise, a thrown Problem as its rejection. */
export function attempt<T>(answer: () => T): Promise<T> {
  try {
    return Promise.resolve(answer());
  } catch (error) {
    return Promise.reject(error as Error);
  }
}

export function mockProfiles(
  store: Store,
  log: (line: string) => void = (line) => console.info(line),
): ProfileFiles {
  return {
    list: () =>
      attempt(() =>
        store.all().map(
          (profile): ProfileSummary => ({
            id: profile.id,
            name: profile.fields.name,
            ...(profile.fields.accent === undefined ? {} : { accent: profile.fields.accent }),
            priority: profile.priority,
            match: structuredClone(profile.match),
            pages: profile.pages.map((page) => page.name),
            path: store.path(profile.id),
            file: true,
          }),
        ),
      ),
    profile: (id) =>
      attempt(() => ({ path: store.path(id), fields: structuredClone(store.get(id).fields) })),
    editProfile: (id, fields) =>
      attempt(() => {
        const profile = store.get(id);
        const before = profile.fields;
        const after: ProfileFields = { name: fields.name };
        if (fields.accent !== undefined) after.accent = fields.accent;
        profile.fields = after;
        return { path: store.path(id), before, fields: structuredClone(after) };
      }),
    open: (id) =>
      attempt(() => {
        store.get(id);
        log(`mock: open ${store.path(id)}`);
      }),
  };
}
