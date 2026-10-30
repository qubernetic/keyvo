// The Profiles screen's file side (docs/design/gui-ux.md §3.2, §4.1): the
// Tauri shell's `profile_list`, `profile`, `edit_profile`, `edit_page`,
// `restore_profile` and `open_profile` commands over keyvo_client; tests
// pass a fake. And the screen's pure parts: the list order, the page list
// and the names of its tabs, the header form.
import { invoke } from "@tauri-apps/api/core";

import type { Problem } from "./editor";
import { t } from "./i18n";

/** A profile's window match rules as written
 * (keyvo_client::profiles::MatchSummary). */
export interface MatchSummary {
  class: string[];
  title?: string;
}

/** One profile of the list (keyvo_client::profiles::ProfileSummary). */
export interface ProfileSummary {
  id: string;
  name: string;
  /** `#rrggbb`. */
  accent?: string;
  priority: number;
  match: MatchSummary;
  /** The name of each page, in order; null for a page without one. */
  pages: (string | null)[];
  /** The pages switched off with `enabled = false`, counted from 1; absent
   * when none is. */
  disabled?: number[];
  /** False when the profile leaves the global pages out of its page list;
   * absent when it keeps them. */
  global_pages?: boolean;
  /** The profile file. */
  path: string;
  /** False for the built-in global profile until its first edit. */
  file: boolean;
}

/** The header fields the editor writes, as keyvo_core::edit::ProfileFields;
 * an absent accent is removed from the file, an absent `global_pages` is
 * true and leaves no line. */
export interface ProfileFields {
  name: string;
  /** `#rrggbb`. */
  accent?: string;
  /** False writes `global_pages = false`; the global profile ignores it. */
  global_pages?: boolean;
}

/** One change to a profile's `[[pages]]`, as keyvo_core::edit::PageEdit;
 * pages count from 1. */
export type PageEdit =
  /** A new page at the end; without a name the tab shows its number. */
  | { op: "add"; name?: string }
  /** Without a name the page's name is removed. */
  | { op: "rename"; page: number; name?: string }
  /** The page moves to position `to`. */
  | { op: "move"; page: number; to: number }
  | { op: "enable"; page: number; enabled: boolean }
  | { op: "delete"; page: number };

/** What a page edit did (keyvo_client::editor::PageEdited). */
export interface PageEdited {
  path: string;
  edit: PageEdit;
  /** The edited page's number after the edit; absent for a delete. */
  page?: number;
  /** The edit that undoes this one; absent for a delete, which the
   * snapshot undoes. */
  undo?: PageEdit;
  /** The copy of the file taken before the write. */
  snapshot?: string;
}

/** What a restore did (keyvo_client::editor::Restored). */
export interface Restored {
  path: string;
  /** The copy of the file taken before the restore. */
  snapshot?: string;
}

/** A profile header as the editor shows it (keyvo_client::editor::ProfileView). */
export interface ProfileView {
  path: string;
  fields: ProfileFields;
}

/** What a header write did (keyvo_client::editor::ProfileEdited). */
export interface ProfileEdited {
  path: string;
  before: ProfileFields;
  fields: ProfileFields;
  snapshot?: string;
}

export interface ProfileFiles {
  /** Every profile of the daemon's configuration directory, in id order;
   * rejects with the load errors. */
  list(): Promise<ProfileSummary[]>;
  /** The profile's file and header fields; rejects with a Problem. */
  profile(id: string): Promise<ProfileView>;
  /** Writes `fields` into the header after a snapshot of the file; rejects
   * with a Problem and leaves the file as it was. */
  editProfile(id: string, fields: ProfileFields): Promise<ProfileEdited>;
  /** Applies `edit` to the profile's pages after a snapshot of the file;
   * rejects with a Problem, a refused edit (the context page switched off,
   * the only enabled page deleted) too, and leaves the file as it was. */
  editPage(id: string, edit: PageEdit): Promise<PageEdited>;
  /** Writes the text of `snapshot` back as the profile's file after a
   * snapshot of the current one; rejects with a Problem. */
  restore(id: string, snapshot: string): Promise<Restored>;
  /** Opens the profile's file in the desktop's application for it;
   * rejects with a Problem. */
  open(id: string): Promise<void>;
}

/** The profile files through the Tauri shell's commands. */
export const tauriProfiles: ProfileFiles = {
  list: () => invoke<ProfileSummary[]>("profile_list"),
  profile: (profile) => invoke<ProfileView>("profile", { profile }),
  editProfile: (profile, fields) => invoke<ProfileEdited>("edit_profile", { profile, fields }),
  editPage: (profile, edit) => invoke<PageEdited>("edit_page", { profile, edit }),
  restore: (profile, snapshot) => invoke<Restored>("restore_profile", { profile, snapshot }),
  open: (profile) => invoke<undefined>("open_profile", { profile }),
};

/** The id of the profile that is always there (spec §6.1). */
export const GLOBAL = "global";

/** The list column's order (§3.2): the global profile pinned first, then
 * by priority, highest first (the one that wins a match, spec §6.2), equal
 * priorities by id. */
export function ordered(list: readonly ProfileSummary[]): ProfileSummary[] {
  return [...list].sort((a, b) => {
    if (a.id === GLOBAL || b.id === GLOBAL) return a.id === GLOBAL ? -1 : b.id === GLOBAL ? 1 : 0;
    if (a.priority !== b.priority) return b.priority - a.priority;
    return a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
  });
}

/** Whether page `page` (from 1) of `profile` is switched on. */
export function enabled(profile: ProfileSummary, page: number): boolean {
  return !(profile.disabled ?? []).includes(page);
}

/** The page of `profile` named `context`, else the first enabled one,
 * else the first, counted from 1: for the global profile, the page under
 * an app profile's keys (keyvo_core::state::context_page). It is in the
 * page list whether it is enabled or not. */
export function contextPage(profile: ProfileSummary | undefined): number {
  if (profile === undefined) return 1;
  const named = profile.pages.indexOf("context");
  if (named !== -1) return named + 1;
  const first = profile.pages.findIndex((_, at) => enabled(profile, at + 1));
  return first === -1 ? 1 : first + 1;
}

/** The pages of `profile` that are switched on. */
export function enabledCount(profile: ProfileSummary): number {
  return profile.pages.filter((_, at) => enabled(profile, at + 1)).length;
}

/** The tab name of page `page` (from 1): its name, else its number. */
export function pageName(profile: ProfileSummary, page: number): string {
  return profile.pages[page - 1] ?? t("profiles.pageNumber", { page });
}

/** One entry of a profile's page list (spec §6.3). */
export interface PageTab {
  /** The list position: 1 the context page, 0 and below the global pages. */
  index: number;
  /** The profile that owns the page. */
  profile: string;
  /** Counted from 1 inside `profile`. */
  page: number;
  name: string;
}

/** The global profile's page named `general`, from 1; none without one. */
function generalPage(global: ProfileSummary | undefined): number | undefined {
  const index = global?.pages.indexOf("general") ?? -1;
  return index === -1 ? undefined : index + 1;
}

/** Whether page `page` of the global profile sits at index 0 or below of
 * every list: its `general` page or one after it. */
export function leftOfContext(global: ProfileSummary | undefined, page: number): boolean {
  const general = generalPage(global);
  return general !== undefined && page >= general && page <= (global?.pages.length ?? 0);
}

/** The page list the pad walks for `owner` in index order, as
 * keyvo_core::state::PageList builds it: the enabled global pages from the
 * last one down to `general`, `general` nearest index 0, unless the owner
 * leaves them out (`global_pages = false`, which the global profile
 * ignores); then the owner's context page at 1 and its other enabled pages
 * from 2 in file order. A disabled page has no index. */
export function pageList(owner: ProfileSummary, global: ProfileSummary | undefined): PageTab[] {
  const isGlobal = owner.id === GLOBAL;
  // The global profile itself is the owner's summary, the freshest one.
  const of = isGlobal ? owner : global;
  const general = generalPage(of);
  const pages: [ProfileSummary, number][] = [];
  if (of !== undefined && general !== undefined && (isGlobal || owner.global_pages !== false)) {
    const context = contextPage(of);
    for (let page = of.pages.length; page >= general; page--) {
      if (page === context || enabled(of, page)) pages.push([of, page]);
    }
  }
  const lowest = 1 - pages.length;
  // A profile without pages adds no tab of its own.
  if (owner.pages.length > 0) {
    const context = contextPage(owner);
    pages.push([owner, context]);
    for (let page = 1; page <= owner.pages.length; page++) {
      if (page === context || !enabled(owner, page)) continue;
      // The global profile's pages from `general` on are left of its
      // context page already.
      if (isGlobal && general !== undefined && page >= general) continue;
      pages.push([owner, page]);
    }
  }
  return pages.map(([profile, page], at) => ({
    index: lowest + at,
    profile: profile.id,
    page,
    name: pageName(profile, page),
  }));
}

/** The header form: every field as the text in its input. */
export interface HeaderForm {
  name: string;
  accent: string;
  /** The "Show the global pages" checkbox; only an app profile offers it. */
  globalPages: boolean;
}

export function toHeaderForm(fields: ProfileFields): HeaderForm {
  return { name: fields.name, accent: fields.accent ?? "", globalPages: fields.global_pages !== false };
}

const COLOR = /^#[0-9a-fA-F]{6}$/;

/** Why a header form cannot be written yet, and the field that says so. */
export interface Invalid {
  field: keyof HeaderForm;
  invalid: string;
}

/** The fields `form` says, or why it cannot be written yet. */
export function fromHeaderForm(form: HeaderForm): { fields: ProfileFields } | Invalid {
  if (form.name.trim() === "") return { field: "name", invalid: t("profiles.noName") };
  const fields: ProfileFields = { name: form.name };
  const accent = form.accent.trim();
  if (accent !== "") {
    if (!COLOR.test(accent)) {
      return { field: "accent", invalid: t("profiles.badAccent", { accent }) };
    }
    fields.accent = accent.toLowerCase();
  }
  if (!form.globalPages) fields.global_pages = false;
  return { fields };
}

export function sameHeader(a: ProfileFields, b: ProfileFields): boolean {
  return (
    a.name === b.name &&
    a.accent?.toLowerCase() === b.accent?.toLowerCase() &&
    (a.global_pages !== false) === (b.global_pages !== false)
  );
}

export type { Problem };
