// The mock's configuration directory: profiles parsed from their TOML
// text, held in memory. Edits change the memory only; a reload of the page
// starts from the files again.
import { parse } from "smol-toml";

import type { KeyFields, Problem } from "../editor";
import type { MatchSummary, ProfileFields } from "../profiles";
import type { Action } from "../protocol";
import { CONFIG_DIR } from "./data";

export interface MockPage {
  name: string | null;
  /** The keys the page defines, by key number from 1. */
  keys: Map<number, KeyFields>;
  /** The `on_fail` actions of the keys, by key number; no edit writes them. */
  onFail: Map<number, Action>;
}

export interface MockProfile {
  id: string;
  fields: ProfileFields;
  priority: number;
  match: MatchSummary;
  pages: MockPage[];
}

type Table = Record<string, unknown>;

function table(value: unknown): Table {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Table)
    : {};
}

function text(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

function action(value: unknown): Action | undefined {
  const { type, keys, cmd, class: windowClass, wait_focus } = table(value);
  if (type === "chord" && typeof keys === "string") return { type, keys };
  if (type === "exec" && typeof cmd === "string") return { type, cmd };
  if (type === "launch" && typeof cmd === "string") {
    const launch: Action = { type, cmd };
    if (typeof windowClass === "string") launch.class = windowClass;
    if (typeof wait_focus === "boolean") launch.wait_focus = wait_focus;
    return launch;
  }
  return undefined;
}

function key(value: unknown): KeyFields {
  const fields = table(value);
  const out: KeyFields = {};
  const icon = text(fields.icon);
  const label = text(fields.label);
  const color = text(fields.color);
  const act = action(fields.action);
  if (icon !== undefined) out.icon = icon;
  if (label !== undefined) out.label = label;
  if (color !== undefined) out.color = color;
  if (act !== undefined) out.action = act;
  return out;
}

/** The profile a profile file describes. */
export function parseProfile(toml: string): MockProfile {
  const doc = parse(toml) as Table;
  const header = table(doc.profile);
  const match = table(header.match);
  const id = text(header.id) ?? "";
  const fields: ProfileFields = { name: text(header.name) ?? id };
  const accent = text(header.accent);
  if (accent !== undefined) fields.accent = accent;
  const pages = Array.isArray(doc.pages) ? doc.pages : [];
  return {
    id,
    fields,
    priority: typeof header.priority === "number" ? header.priority : 0,
    match: {
      class: Array.isArray(match.class) ? match.class.filter((c) => typeof c === "string") : [],
      ...(typeof match.title === "string" ? { title: match.title } : {}),
    },
    pages: pages.map((page: unknown) => {
      const { name, keys } = table(page);
      const entries = Object.entries(table(keys));
      return {
        name: text(name) ?? null,
        keys: new Map(entries.map(([n, fields]) => [Number(n), key(fields)])),
        onFail: new Map(
          entries.flatMap(([n, fields]) => {
            const onFail = action(table(fields).on_fail);
            return onFail === undefined ? [] : [[Number(n), onFail] as const];
          }),
        ),
      };
    }),
  };
}

/** A rejection as the editor's Problem. */
export function problem(message: string): Problem {
  return { message };
}

export class Store {
  readonly configDir: string;
  readonly #profiles = new Map<string, MockProfile>();
  /** The text each profile was parsed from; edits leave it as it was. */
  readonly #texts = new Map<string, string>();

  /** The profiles of `files`, each the text of one profile file. */
  constructor(files: readonly string[], configDir = CONFIG_DIR) {
    this.configDir = configDir;
    for (const file of files) {
      const profile = parseProfile(file);
      this.#profiles.set(profile.id, profile);
      this.#texts.set(profile.id, file);
    }
  }

  /** The line, from 1, of the `[pages.keys.N]` header of key `key` on page
   * `page` (from 1) in the text of the profile `id`; none when that page of
   * the text has no such header (an inline key, or one only an edit added). */
  keyLine(id: string, page: number, key: number): number | undefined {
    const lines = (this.#texts.get(id) ?? "").split("\n");
    const header = new RegExp(`^\\s*\\[\\s*pages\\s*\\.\\s*keys\\s*\\.\\s*"?${key}"?\\s*\\]`);
    let at = 0;
    for (const [index, line] of lines.entries()) {
      if (/^\s*\[\[\s*pages\s*\]\]/.test(line)) at += 1;
      else if (at === page && header.test(line)) return index + 1;
    }
    return undefined;
  }

  /** Every profile, in id order. */
  all(): MockProfile[] {
    return [...this.#profiles.values()].sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
  }

  find(id: string): MockProfile | undefined {
    return this.#profiles.get(id);
  }

  /** The profile `id`; throws a Problem when there is none. */
  get(id: string): MockProfile {
    const profile = this.#profiles.get(id);
    if (profile === undefined) throw problem(`no profile \`${id}\``);
    return profile;
  }

  /** Page `page` (from 1) of the profile `id`; throws a Problem when there
   * is none. */
  page(id: string, page: number): MockPage {
    const found = this.get(id).pages[page - 1];
    if (found === undefined) throw problem(`profile \`${id}\` has no page ${page}`);
    return found;
  }

  /** The file of the profile `id`. */
  path(id: string): string {
    return `${this.configDir}/profiles/${id}.toml`;
  }
}
