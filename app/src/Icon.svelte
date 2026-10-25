<script lang="ts">
  // A bundled lucide icon in the text colour. Decorative: the control
  // around it carries the name.
  import { icon, type IconName } from "./lib/icons";

  let { name }: { name: IconName } = $props();
</script>

<span class="icon" aria-hidden="true">
  {@html icon(name)}
</span>

<style>
  .icon {
    display: inline-flex;
    flex: none;
    width: 20px;
    height: 20px;
  }

  .icon :global(svg) {
    width: 100%;
    height: 100%;
  }
</style>
