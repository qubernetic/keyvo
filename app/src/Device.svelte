<script lang="ts">
  // The Keypad as itself (docs/design/gui-ux.md §3.1): the owner's drawing
  // inline, coloured from --kv-device, with the caller's tiles in its key
  // cutouts, scaled with it up to the panel's pixel width. A geometry the
  // drawing has no cutout for each key of goes on the plain proportional
  // panel instead. Either way `tile` gets one call per key with the key
  // and the percent style of its place (left, top, width, height), and
  // `row`, when given, wraps the calls of each geometry row; the places
  // are percentages of this component's root, so the cells may sit
  // inside wrappers with `display: contents`.
  import type { Snippet } from "svelte";

  import keypad from "./assets/device/keypad.svg?raw";
  import {
    DEVICE_COLOURS,
    drawing,
    fallBack,
    inline,
    panelPlacement,
    placement,
    styles,
  } from "./lib/device";
  import { grid } from "./lib/pad";
  import { t } from "./lib/i18n";
  import type { Geometry } from "./lib/protocol";

  let {
    geometry,
    tile,
    row,
    svg = keypad,
  }: {
    geometry: Geometry;
    /** One call per key: the key, from 1, and its place as inline style. */
    tile: Snippet<[number, string]>;
    /** Wraps the cells of one row of the geometry. */
    row?: Snippet<[Snippet]>;
    /** The drawing, the Keypad's by default. */
    svg?: string;
  } = $props();

  const id = $props.id();
  const device = $derived(drawing(svg));
  const fitted = $derived(placement(device, geometry));
  const places = $derived(styles(fitted ?? panelPlacement(geometry)));
  const rows = $derived(grid(geometry));
  const markup = $derived(fitted === undefined ? "" : inline(svg, id, t("device.keypad")));
  const aspect = $derived(
    fitted === undefined
      ? `${geometry.panel.width} / ${geometry.panel.height}`
      : `${device.width} / ${device.height}`,
  );

  $effect(() => {
    if (fitted === undefined) fallBack(device.cutouts.length, geometry.rects.length);
  });

  /** Sets the derived colours on the root. */
  function colours(node: HTMLElement): void {
    for (const [name, value] of Object.entries(DEVICE_COLOURS)) {
      node.style.setProperty(name, value);
    }
  }
</script>

<div
  class="device"
  class:panel={fitted === undefined}
  style:width="100%"
  style:max-width={`${geometry.panel.width}px`}
  style:aspect-ratio={aspect}
  use:colours
>
  {#if fitted !== undefined}
    <!-- The drawing is the repository's own asset, its ids prefixed per instance. -->
    {@html markup}
  {/if}
  <div class="tiles">
    {#if row === undefined}
      {#each geometry.rects as _, index (index)}
        {@render tile(index + 1, places.get(index + 1) ?? "")}
      {/each}
    {:else}
      {#each rows as keys, index (index)}
        {#snippet cells()}
          {#each keys as key (key)}
            {@render tile(key, places.get(key) ?? "")}
          {/each}
        {/snippet}
        {@render row(cells)}
      {/each}
    {/if}
  </div>
</div>

<style>
  .device {
    position: relative;
  }

  .device :global(svg) {
    display: block;
    width: 100%;
    height: 100%;
  }

  .panel {
    border-radius: var(--kv-radius);
    background: var(--kv-bg-alt);
    color: var(--kv-fg);
  }

  .tiles {
    position: absolute;
    inset: 0;
  }
</style>
