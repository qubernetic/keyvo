// The icons a tile can carry: the lucide SVGs keyvo-core bundles for the
// renderer (crates/keyvo-core/assets/lucide, origin and licence there),
// read at build time, so the picker offers exactly what the pad can draw.
// A profile names one as `lucide:<name>`.

const files = import.meta.glob<string>("../../../crates/keyvo-core/assets/lucide/*.svg", {
  query: "?raw",
  import: "default",
  eager: true,
});

/** The bundled markup by icon name: the whole lucide set, its old names
 * left to the renderer (keyvo_core::render::icon, LUCIDE_ALIASES). */
const MARKUP = new Map(
  Object.entries(files).map(([path, svg]) => [path.slice(path.lastIndexOf("/") + 1, -".svg".length), svg]),
);

/** The names of the tile icons, sorted. */
export const TILE_ICONS: readonly string[] = [...MARKUP.keys()].sort();

/** The SVG markup of `name`; undefined for a name keyvo does not bundle. */
export function tileIcon(name: string): string | undefined {
  return MARKUP.get(name);
}

const PREFIX = "lucide:";

/** A tile icon as the profile's `icon` field writes it. */
export const tileIconValue = {
  write: (name: string): string => `${PREFIX}${name}`,
  /** The name of a `lucide:<name>` value; undefined for anything else. */
  read: (value: string): string | undefined =>
    value.startsWith(PREFIX) && value.length > PREFIX.length ? value.slice(PREFIX.length) : undefined,
};
