// The actions catalogue (pad screen brief §5.2, decisions Q3 and Q8): the
// v1 entries in the groups General, System and Apps, the search over them,
// what a tool does to the inspector's form, and the data a drag carries to
// a tile. The raw tools Chord and Exec work today; the presets are the
// recipe packs of #119 and stay "Coming soon" until the catalogue reads
// them from the daemon.
import type { Form } from "./editor";
import { t, type MessageKey } from "./i18n";

export type GroupId = "general" | "system" | "apps";

/** The groups, top to bottom. */
export const GROUPS: readonly { id: GroupId; label: MessageKey }[] = [
  { id: "general", label: "catalogue.general" },
  { id: "system", label: "catalogue.system" },
  { id: "apps", label: "catalogue.apps" },
];

export interface CatalogueEntry {
  id: string;
  group: GroupId;
  label: MessageKey;
  /** A tile icon name (lib/tileIcons.ts). */
  icon: string;
  /** A raw tool sets the action type; a recipe is a preset pack. */
  kind: "tool" | "recipe";
  /** For an entry not built yet: the sentence naming its roadmap item. */
  soon?: MessageKey;
}

function recipe(id: string, group: GroupId, label: MessageKey, icon: string): CatalogueEntry {
  return { id, group, label, icon, kind: "recipe", soon: "soon.presets" };
}

/** The v1 entries in group order. */
export const CATALOGUE: readonly CatalogueEntry[] = [
  recipe("open-folder", "general", "catalogue.openFolder", "folder-open"),
  recipe("open-website", "general", "catalogue.openWebsite", "globe"),
  recipe("new-email", "general", "catalogue.newEmail", "mail"),
  { id: "chord", group: "general", label: "catalogue.chord", icon: "square", kind: "tool" },
  { id: "exec", group: "general", label: "catalogue.exec", icon: "terminal", kind: "tool" },
  recipe("lock", "system", "catalogue.lock", "lock"),
  recipe("volume", "system", "catalogue.volume", "volume-2"),
  recipe("brightness", "system", "catalogue.brightness", "sun"),
  recipe("settings-page", "system", "catalogue.settingsPage", "settings"),
  recipe("spectacle", "apps", "catalogue.spectacle", "camera"),
  recipe("calculator", "apps", "catalogue.calculator", "calculator"),
  recipe("vscode", "apps", "catalogue.vscode", "code"),
  recipe("konsole", "apps", "catalogue.konsole", "terminal"),
];

function groupLabel(group: GroupId): string {
  const found = GROUPS.find((candidate) => candidate.id === group);
  return found === undefined ? "" : t(found.label);
}

/** The entries whose label or group name contains `text`, case-insensitive;
 * every entry for a blank text. */
export function search(entries: readonly CatalogueEntry[], text: string): CatalogueEntry[] {
  const wanted = text.trim().toLowerCase();
  if (wanted === "") return [...entries];
  return entries.filter((entry) =>
    [t(entry.label), groupLabel(entry.group)].some((name) => name.toLowerCase().includes(wanted)),
  );
}

/** `form` with `entry` applied: a tool sets its action type with an empty
 * field, so nothing is written until the field is entered (the editor's
 * rule); a preset leaves the form until #119 fills in its variant. */
export function applied(entry: CatalogueEntry, form: Form): Form {
  if (entry.kind !== "tool" || (entry.id !== "chord" && entry.id !== "exec")) return form;
  return { ...form, actionType: entry.id, action: "" };
}

/** The `DataTransfer` type a dragged entry is carried under. */
export const DRAG_TYPE = "application/x-keyvo-action";

/** What a drag of `entry` carries: `<kind>:<id>`. */
export function dragData(entry: CatalogueEntry): string {
  return `${entry.kind}:${entry.id}`;
}

/** The id of the tool a drop carries; undefined for a preset, an unknown
 * entry or data from elsewhere. */
export function dropped(data: string): string | undefined {
  const id = data.startsWith("tool:") ? data.slice("tool:".length) : undefined;
  return CATALOGUE.some((entry) => entry.kind === "tool" && entry.id === id) ? id : undefined;
}
