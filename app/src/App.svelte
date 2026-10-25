<script lang="ts">
  // The app shell: a heading and Home. The navigation rail and the other
  // screens of docs/design/gui-ux.md §2 come after the pre-alpha.
  import Home from "./Home.svelte";
  import type { Daemon } from "./lib/daemon";

  let { daemon }: { daemon: Daemon } = $props();
</script>

<main>
  <h1>keyvo</h1>
  <Home {daemon} />
</main>

<style>
  main {
    padding: 16px;
  }

  h1 {
    margin: 0 0 12px;
    font-size: 1.4em;
  }
</style>
