<script lang="ts">
  // The mirror (pad screen brief §5.2): the tiles the pad shows in the
  // cutouts of the device drawing (Device.svelte), each cell placed by the
  // percent rect Device hands over, so the grid scales with the drawing and
  // never above 1:1. A click selects a key and does not press it; 1 to 9
  // and the arrows select, Space presses the selected key short and
  // Shift+Space long, as dry runs. The selected tile has the accent
  // outline, a key held down on the device its mark, and a key that falls
  // through from the global layer is dimmed with a corner mark. Every cell
  // is a drop target for a tool dragged from the actions catalogue.
  import { tick } from "svelte";

  import Device from "./Device.svelte";
  import { DRAG_TYPE, dropped } from "./lib/catalogue";
  import type { Tile } from "./lib/daemon";
  import { t } from "./lib/i18n";
  import { describeAction, grid, moved, phrase } from "./lib/pad";
  import type { Geometry, PressKind } from "./lib/protocol";
  import { GLOBAL } from "./lib/profiles";

  let {
    geometry,
    tiles,
    profile,
    held,
    selected,
    name,
    onselect,
    onpress,
    ondrop,
  }: {
    geometry: Geometry;
    /** The pad's tiles in key order; a missing one shows its key number. */
    tiles: Tile[];
    /** The profile the pad shows, for the fall-through mark. */
    profile: string | undefined;
    /** The keys held down on the device. */
    held: number[];
    /** The selected key; none until the user selects one. */
    selected?: number;
    /** The grid's accessible name. */
    name: string;
    onselect: (key: number) => void;
    /** A dry-run press of `key`. */
    onpress: (key: number, kind: PressKind) => void;
    /** A catalogue tool, by its entry id, dropped on `key`. */
    ondrop?: (key: number, entry: string) => void;
  } = $props();

  const rows = $derived(grid(geometry));
  const keyCount = $derived(geometry.rects.length);
  const buttons: HTMLButtonElement[] = $state([]);

  function label(key: number): string {
    const tile = tiles[key - 1];
    const parts = phrase([
      tile?.label,
      describeAction(tile?.action) ?? t("home.unbound"),
      tile?.layer === undefined ? undefined : t("layer", { layer: tile.layer }),
    ]);
    return t("home.key", { key, parts });
  }

  /** A global key under an app profile (spec §6.1). */
  function fallen(key: number): boolean {
    return tiles[key - 1]?.layer === "global" && profile !== GLOBAL;
  }

  async function select(key: number) {
    onselect(key);
    await tick();
    buttons[key - 1]?.focus();
  }

  /** The key a drag is over, for its outline. */
  let over: number | undefined = $state();

  function dragover(event: DragEvent, key: number) {
    if (ondrop === undefined) return;
    event.preventDefault();
    if (event.dataTransfer !== null) event.dataTransfer.dropEffect = "copy";
    over = key;
  }

  function drop(event: DragEvent, key: number) {
    over = undefined;
    const entry = dropped(event.dataTransfer?.getData(DRAG_TYPE) ?? "");
    if (entry === undefined) return;
    event.preventDefault();
    ondrop?.(key, entry);
  }

  function keydown(event: KeyboardEvent) {
    if (/^[1-9]$/.test(event.key) && Number(event.key) <= keyCount) {
      event.preventDefault();
      void select(Number(event.key));
    } else if (event.key === " ") {
      event.preventDefault();
      if (selected !== undefined) onpress(selected, event.shiftKey ? "long" : "short");
    } else if (["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(event.key)) {
      event.preventDefault();
      void select(moved(rows, selected ?? 1, event.key));
    }
  }
</script>

<div role="grid" tabindex="-1" aria-label={name} class="grid" onkeydown={keydown}>
  <Device {geometry}>
    {#snippet row(cells)}
      <div role="row" class="row">{@render cells()}</div>
    {/snippet}
    {#snippet tile(key, place)}
      <div
        role="gridcell"
        aria-selected={key === selected}
        tabindex="-1"
        style={place}
        ondragover={(event) => dragover(event, key)}
        ondragleave={() => (over = over === key ? undefined : over)}
        ondrop={(event) => drop(event, key)}
      >
        <button
          bind:this={buttons[key - 1]}
          type="button"
          class="key"
          class:selected={key === selected}
          class:held={held.includes(key)}
          class:fallen={fallen(key)}
          class:over={key === over}
          tabindex={key === (selected ?? 1) ? 0 : -1}
          aria-label={label(key)}
          onclick={() => void select(key)}
        >
          {#if tiles[key - 1]}
            <img src={tiles[key - 1]?.image} alt="" />
            {#if fallen(key)}
              <span class="layer" aria-hidden="true">{t("home.globalMark")}</span>
            {/if}
          {:else}
            <span aria-hidden="true">{key}</span>
          {/if}
        </button>
      </div>
    {/snippet}
  </Device>
</div>

<style>
  .grid {
    width: 100%;
  }

  /* Device places the cells; the rows only group them for the grid role. */
  .row {
    display: contents;
  }

  .key {
    position: relative;
    width: 100%;
    height: 100%;
    padding: 0;
    border: 2px solid transparent;
    border-radius: var(--kv-radius);
    background: var(--kv-bg-alt);
    color: var(--kv-fg-muted);
    font: inherit;
    cursor: pointer;
    overflow: hidden;
  }

  .key:focus-visible {
    outline: 2px solid var(--kv-fg);
    outline-offset: 2px;
  }

  .key.selected {
    border-color: var(--kv-accent);
  }

  .key.over {
    border-style: dashed;
    border-color: var(--kv-fg);
  }

  .key.held {
    outline: 2px dashed var(--kv-fg);
    outline-offset: -6px;
  }

  .key.fallen img {
    opacity: 0.5;
  }

  .key img {
    display: block;
    width: 100%;
    height: 100%;
  }

  .key .layer {
    position: absolute;
    top: 4px;
    left: 4px;
    padding: 0 4px;
    border-radius: var(--kv-radius);
    background: var(--kv-bg-alt);
    color: var(--kv-fg);
    font-size: 0.75em;
  }
</style>
