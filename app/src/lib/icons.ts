// The lucide icons the shell draws, bundled under src/assets/lucide (see
// the README there for origin and licence).

export const ICONS = [
  "house",
  "layers",
  "workflow",
  "folder-git-2",
  "plug",
  "keyboard",
  "settings",
  "ellipsis-vertical",
  "panel-left",
  "panel-right",
] as const;

export type IconName = (typeof ICONS)[number];

const files = import.meta.glob<string>("../assets/lucide/*.svg", {
  query: "?raw",
  import: "default",
  eager: true,
});

/** The SVG markup of `name`. */
export function icon(name: IconName): string {
  const svg = files[`../assets/lucide/${name}.svg`];
  if (svg === undefined) throw new Error(`no icon ${name} in src/assets/lucide`);
  return svg;
}
