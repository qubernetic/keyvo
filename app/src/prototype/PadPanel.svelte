<script lang="ts">
  // PROTOTYPE (#107): the pad's panel in device proportions, scaled to its
  // container and never above 1:1; the tiles sit where the geometry puts
  // them. The answer to the P0 of docs/design/critique-2026-10.md, shared
  // by the three variants; the winner's version is rewritten in #108.
  import type { Tile } from "../lib/daemon";
  import type { Geometry } from "../lib/protocol";

  let {
    geometry,
    tiles,
    selected,
    label,
    onchoose,
    keyName = (key: number) => String(key),
  }: {
    geometry: Geometry;
    tiles: Tile[];
    selected?: number;
    label: string;
    /** Absent: the panel is a picture, not a control. */
    onchoose?: (key: number) => void;
    keyName?: (key: number) => string;
  } = $props();

  const pct = (part: number, whole: number) => `${(part / whole) * 100}%`;
</script>

<div
  role={onchoose === undefined ? "img" : "group"}
  aria-label={label}
  class="panel"
  style:max-width={`${geometry.panel.width}px`}
  style:aspect-ratio={`${geometry.panel.width} / ${geometry.panel.height}`}
>
  {#each geometry.rects as rect, index (index)}
    {@const key = index + 1}
    {@const tile = tiles[index]}
    <button
      type="button"
      class="tile"
      class:selected={key === selected}
      class:still={onchoose === undefined}
      tabindex={onchoose === undefined ? -1 : key === (selected ?? 1) ? 0 : -1}
      aria-label={keyName(key)}
      aria-pressed={onchoose === undefined ? undefined : key === selected}
      style:left={pct(rect.x, geometry.panel.width)}
      style:top={pct(rect.y, geometry.panel.height)}
      style:width={pct(rect.width, geometry.panel.width)}
      style:height={pct(rect.height, geometry.panel.height)}
      onclick={() => onchoose?.(key)}
    >
      {#if tile}
        <img src={tile.image} alt="" />
      {:else}
        <span aria-hidden="true">{key}</span>
      {/if}
    </button>
  {/each}
</div>

<style>
  .panel {
    position: relative;
    box-sizing: border-box;
    width: 100%;
    border: 1px solid var(--kv-fg-muted);
    border-radius: var(--kv-radius);
    background: var(--kv-bg-alt);
    overflow: hidden;
  }

  .tile {
    position: absolute;
    box-sizing: border-box;
    min-height: 0;
    padding: 0;
    border: 2px solid transparent;
    border-radius: var(--kv-radius);
    background: var(--kv-bg);
    overflow: hidden;
  }

  .tile.still {
    cursor: default;
  }

  .tile.selected {
    border-color: var(--kv-accent);
  }

  .tile img {
    display: block;
    width: 100%;
    height: 100%;
  }
</style>
