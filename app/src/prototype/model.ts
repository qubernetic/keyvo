// PROTOTYPE (#107, slice 4 of the GUI design pass): the state of the
// Profiles screen as the three variants read and drive it. Profiles.svelte
// builds one object with getters over its own $state, so a variant's
// markup stays reactive without owning any data. Deleted with the losing
// variants in #108.
import type { Daemon, Tile } from "../lib/daemon";
import type { Editor, Problem, Target } from "../lib/editor";
import type { HeaderForm, ProfileSummary, ProfileView } from "../lib/profiles";
import type { Geometry, View } from "../lib/protocol";
import type { Columns } from "../lib/shell";

export interface ProfilesModel {
  readonly daemon: Daemon;
  readonly editor: Editor;
  readonly columns: Columns;
  readonly reloads: number;
  readonly active: boolean;

  readonly profiles: ProfileSummary[];
  readonly loading: boolean;
  readonly listError: string | undefined;
  readonly selected: ProfileSummary | undefined;
  readonly view: View | undefined;
  readonly geometry: Geometry | undefined;

  readonly pages: number[];
  readonly page: number;
  readonly key: number | undefined;
  readonly tiles: Tile[];
  readonly tilesError: string | undefined;
  readonly rows: number[][];
  readonly target: Target | undefined;

  readonly header: ProfileView | undefined;
  readonly form: HeaderForm;
  readonly problem: Problem | undefined;
  readonly invalid: string | undefined;
  readonly note: string | undefined;

  pick(profile: ProfileSummary): void;
  tab(page: number): void;
  choose(key: number): void;
  commit(): void;
  open(): void;
  keyName(key: number): string;
  layerOf(key: number): string;
  itemName(profile: ProfileSummary): string;
  edited(): void;
}

export type VariantKey = "a" | "b" | "c";

export const VARIANTS: readonly { key: VariantKey; name: string }[] = [
  { key: "a", name: "Instrument: the panel and the editor side by side" },
  { key: "b", name: "Document: one column, the pad live on the right" },
  { key: "c", name: "Three panes: profile, page, key" },
];

/** The `?variant=` of the page, if it names a variant. */
export function variantOf(search: string): VariantKey | undefined {
  const value = new URLSearchParams(search).get("variant")?.toLowerCase();
  return VARIANTS.some((variant) => variant.key === value) ? (value as VariantKey) : undefined;
}

/** The tiles of every page of a profile, page 1 first. */
export async function pageTiles(
  daemon: Daemon,
  profile: string,
  pages: number,
  geometry: Geometry,
): Promise<Tile[][]> {
  return Promise.all(
    Array.from({ length: pages }, (_, index) =>
      daemon.tiles(profile, index + 1, geometry.rects.length, geometry.tile).catch(() => []),
    ),
  );
}
