<script lang="ts">
  // Test harness for Device.test.ts: the cells wrapped in grid rows inside
  // the slot, as the Pad's mirror does.
  import Device from "../Device.svelte";
  import type { Geometry } from "../lib/protocol";

  let { geometry, svg }: { geometry: Geometry; svg?: string } = $props();
</script>

<div role="grid">
  <Device {geometry} {svg}>
    {#snippet row(cells)}
      <div role="row" style="display: contents">{@render cells()}</div>
    {/snippet}
    {#snippet tile(key, style)}
      <div role="gridcell" data-key={key} {style}></div>
    {/snippet}
  </Device>
</div>
