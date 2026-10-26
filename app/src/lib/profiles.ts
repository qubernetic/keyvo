// The Profiles screen's file side (docs/design/gui-ux.md §3.2, §4.1): the
// Tauri shell's `profile_list`, `profile`, `edit_profile` and
// `open_profile` commands over keyvo_client; tests pass a fake. And the
// screen's pure parts: the list order, the page names of the tabs, the
// header form.
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
  /** The profile file. */
  path: string;
  /** False for the built-in global profile until its first edit. */
  file: boolean;
}

/** The header fields the editor writes, as keyvo_core::edit::ProfileFields;
 * an absent accent is removed from the file. */
export interface ProfileFields {
  name: string;
  /** `#rrggbb`. */
  accent?: string;
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
  /** Opens the profile's file in the desktop's application for it;
   * rejects with a Problem. */
  open(id: string): Promise<void>;
}

/** The profile files through the Tauri shell's commands. */
export const tauriProfiles: ProfileFiles = {
  list: () => invoke<ProfileSummary[]>("profile_list"),
  profile: (profile) => invoke<ProfileView>("profile", { profile }),
  editProfile: (profile, fields) => invoke<ProfileEdited>("edit_profile", { profile, fields }),
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

/** The tab name of page `page` (from 1): its name, else its number. */
export function pageName(profile: ProfileSummary, page: number): string {
  return profile.pages[page - 1] ?? t("profiles.pageNumber", { page });
}

/** The header form: every field as the text in its input. */
export interface HeaderForm {
  name: string;
  accent: string;
}

export function toHeaderForm(fields: ProfileFields): HeaderForm {
  return { name: fields.name, accent: fields.accent ?? "" };
}

const COLOR = /^#[0-9a-fA-F]{6}$/;

/** The fields `form` says, or why it cannot be written yet. */
export function fromHeaderForm(form: HeaderForm): { fields: ProfileFields } | { invalid: string } {
  if (form.name.trim() === "") return { invalid: t("profiles.noName") };
  const fields: ProfileFields = { name: form.name };
  const accent = form.accent.trim();
  if (accent !== "") {
    if (!COLOR.test(accent)) return { invalid: t("profiles.badAccent", { accent }) };
    fields.accent = accent.toLowerCase();
  }
  return { fields };
}

export function sameHeader(a: ProfileFields, b: ProfileFields): boolean {
  return a.name === b.name && a.accent?.toLowerCase() === b.accent?.toLowerCase();
}

export type { Problem };
