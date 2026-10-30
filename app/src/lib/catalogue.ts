// The actions catalogue (pad screen brief §5.2, decisions Q3 and Q8): the
// entries in the groups General, System and Apps, the search over them,
// what an entry does to the inspector's form, and the data a drag carries
// to a tile. The presets are the recipe packs of the daemon's `recipes`
// reply (spec §7.5); the raw tools Chord, Exec and Launch close General. A key
// never names its pack, so the recipe a key carries is found from its icon
// and action.
import { isRoller } from "./dialpad";
import { actionForm, type ActionPart, type ControlForm, type Form, type KeyFields } from "./editor";
import { t, type MessageKey } from "./i18n";
import type { Action, Control, Recipe, RecipeGroup } from "./protocol";
import { tileIconValue } from "./tileIcons";

export type GroupId = RecipeGroup;

/** The groups, top to bottom. */
export const GROUPS: readonly { id: GroupId; label: MessageKey }[] = [
  { id: "general", label: "catalogue.general" },
  { id: "system", label: "catalogue.system" },
  { id: "apps", label: "catalogue.apps" },
];

export interface CatalogueEntry {
  /** `<kind>:<id>`, unique in the catalogue: what a drag carries and a
   * click assigns. */
  key: string;
  id: string;
  group: GroupId;
  /** The name the catalogue shows. */
  name: string;
  /** A tile icon name (lib/tileIcons.ts). */
  icon: string;
  /** A raw tool sets the action type; a recipe is a preset pack. */
  kind: "tool" | "recipe";
  /** The pack of a recipe entry. */
  recipe?: Recipe;
}

/** The action types a raw tool sets. */
type ToolId = Exclude<Form["actionType"], "none">;

function tool(id: ToolId, label: MessageKey, icon: string): CatalogueEntry {
  return { key: `tool:${id}`, id, group: "general", name: t(label), icon, kind: "tool" };
}

/** The raw tools, at the end of General. */
export const TOOLS: readonly CatalogueEntry[] = [
  tool("chord", "catalogue.chord", "square"),
  tool("exec", "catalogue.exec", "terminal"),
  tool("launch", "catalogue.launch", "app-window"),
];

/** The entries of `recipes` (in the reply's order) in group order, the raw
 * tools at the end of General. */
export function catalogueOf(recipes: readonly Recipe[]): CatalogueEntry[] {
  const presets = recipes.map(
    (recipe): CatalogueEntry => ({
      key: `recipe:${recipe.id}`,
      id: recipe.id,
      group: recipe.group,
      name: recipe.name,
      icon: tileIconValue.read(recipe.icon) ?? "",
      kind: "recipe",
      recipe,
    }),
  );
  return GROUPS.flatMap((group) => [
    ...presets.filter((entry) => entry.group === group.id),
    ...TOOLS.filter((entry) => entry.group === group.id),
  ]);
}

function groupLabel(group: GroupId): string {
  const found = GROUPS.find((candidate) => candidate.id === group);
  return found === undefined ? "" : t(found.label);
}

/** The entries whose name, group name or a variant's name contains
 * `text`, case-insensitive; every entry for a blank text. */
export function search(entries: readonly CatalogueEntry[], text: string): CatalogueEntry[] {
  const wanted = text.trim().toLowerCase();
  if (wanted === "") return [...entries];
  return entries.filter((entry) =>
    [entry.name, groupLabel(entry.group), ...(entry.recipe?.variants.map((v) => v.name) ?? [])].some(
      (name) => name.toLowerCase().includes(wanted),
    ),
  );
}

/** `form` with variant `variant` of `recipe`: its action, the pack's label
 * and icon, the colour kept (keyvo_client::Editor::apply_recipe); `form`
 * itself for a variant the pack does not have. */
export function withVariant(recipe: Recipe, variant: string, form: Form): Form {
  const found = recipe.variants.find((candidate) => candidate.id === variant);
  if (found === undefined) return form;
  return { ...form, label: recipe.label, icon: recipe.icon, ...actionForm(found.action) };
}

/** `form` with `entry` applied: a tool sets its action type with an empty
 * field, so nothing is written until the field is entered (the editor's
 * rule); a preset writes its default variant. */
export function applied(entry: CatalogueEntry, form: Form): Form {
  if (entry.recipe !== undefined) {
    const first = entry.recipe.variants[0];
    return first === undefined ? form : withVariant(entry.recipe, first.id, form);
  }
  const tool = TOOLS.find((candidate) => candidate.id === entry.id);
  if (tool === undefined) return form;
  return { ...form, ...actionForm(undefined), actionType: tool.id as ToolId };
}

/** `part` with `entry`'s action: a tool's type with an empty field, a
 * preset's default variant; `part` itself for an unknown tool. */
export function appliedAction(entry: CatalogueEntry, part: ActionPart): ActionPart {
  if (entry.recipe !== undefined) {
    const first = entry.recipe.variants[0];
    return first === undefined ? part : actionForm(first.action);
  }
  const tool = TOOLS.find((candidate) => candidate.id === entry.id);
  return tool === undefined ? part : { ...actionForm(undefined), actionType: tool.id as ToolId };
}

/** `form` with `entry` assigned to the Dialpad `control` (#188): a
 * button's action, and a preset's label with it as a recipe writes a
 * key's; the `direction` of a rotary, the field last edited, its label
 * kept. */
export function appliedToControl(
  entry: CatalogueEntry,
  control: Control,
  direction: "cw" | "ccw",
  form: ControlForm,
): ControlForm {
  if (isRoller(control)) return { ...form, [direction]: appliedAction(entry, form[direction]) };
  return {
    ...form,
    ...(entry.recipe === undefined ? {} : { label: entry.recipe.label }),
    action: appliedAction(entry, form.action),
  };
}

/** The `DataTransfer` type a dragged entry is carried under. */
export const DRAG_TYPE = "application/x-keyvo-action";

/** What a drag of `entry` carries: its key, `<kind>:<id>`. */
export function dragData(entry: CatalogueEntry): string {
  return entry.key;
}

const RECIPE_KEY = /^recipe:[A-Za-z0-9._-]+$/;

/** The entry key a drop carries: a raw tool's or a recipe's; undefined for
 * an unknown tool or data from elsewhere. */
export function dropped(data: string): string | undefined {
  if (TOOLS.some((entry) => entry.key === data)) return data;
  return RECIPE_KEY.test(data) ? data : undefined;
}

function sameAction(a: Action, b: Action): boolean {
  return a.type === "chord"
    ? b.type === "chord" && a.keys === b.keys
    : b.type === a.type && a.cmd === b.cmd;
}

/** The recipe a key carries and the variant it runs: a pack with the key's
 * icon and a variant with its action, the one with the key's label first,
 * else the first in catalogue order; undefined for any other key. */
export function recipeOf(
  recipes: readonly Recipe[],
  fields: KeyFields,
): { recipe: Recipe; variant: string } | undefined {
  const action = fields.action;
  if (action === undefined) return undefined;
  const found = recipes.flatMap((recipe) => {
    if (recipe.icon !== fields.icon) return [];
    const variant = recipe.variants.find((candidate) => sameAction(candidate.action, action));
    return variant === undefined ? [] : [{ recipe, variant: variant.id }];
  });
  return found.find((candidate) => candidate.recipe.label === fields.label) ?? found[0];
}
