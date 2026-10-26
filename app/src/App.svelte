<script lang="ts">
  // The app shell (docs/design/gui-ux.md §2, d8): the header across the
  // top, the navigation rail on the left, the selected screen beside it.
  // Home stays mounted while another screen shows, so the header's device
  // selector follows the daemon and the mirror keeps its last frame.
  import Header from "./Header.svelte";
  import Home from "./Home.svelte";
  import Profiles from "./Profiles.svelte";
  import Rail from "./Rail.svelte";
  import type { Daemon } from "./lib/daemon";
  import type { Editor } from "./lib/editor";
  import type { ProfileFiles } from "./lib/profiles";
  import type { DeviceStatus, View } from "./lib/protocol";
  import { layout, type DeviceChoice, type ScreenId } from "./lib/shell";

  let {
    daemon,
    editor,
    files,
  }: { daemon: Daemon; editor: Editor; files: ProfileFiles } = $props();

  let width = $state(window.innerWidth);
  const columns = $derived(layout(width));
  let active: ScreenId = $state("home");
  let device: DeviceStatus | undefined = $state();
  let chosen: DeviceChoice | undefined = $state();
  /** What the pad shows and the daemon's reload count, as Home follows them. */
  let view: View | undefined = $state();
  let reloads = $state(0);
</script>

<svelte:window bind:innerWidth={width} />

<div class="app">
  <div class="top">
    <Header {device} {chosen} onchoose={(choice) => (chosen = choice)} />
  </div>
  <Rail {active} labels={columns.railLabels} onnavigate={(id) => (active = id)} />
  <main>
    <div class="screen" hidden={active !== "home"}>
      <Home
        {daemon}
        {editor}
        active={active === "home"}
        ondevice={(next) => (device = next)}
        onnavigate={(id) => (active = id)}
        onview={(next, count) => {
          view = next;
          reloads = count;
        }}
      />
    </div>
    {#if active === "profiles"}
      <div class="screen">
        <Profiles
          {daemon}
          {editor}
          {files}
          {columns}
          {view}
          {reloads}
          geometry={device?.geometry}
          active={active === "profiles"}
        />
      </div>
    {/if}
  </main>
</div>

<style>
  .app {
    display: grid;
    grid-template-columns: auto 1fr;
    grid-template-rows: auto 1fr;
    height: 100vh;
  }

  .top {
    grid-column: 1 / -1;
  }

  main {
    min-width: 0;
    min-height: 0;
    overflow: auto;
  }

  .screen {
    box-sizing: border-box;
    height: 100%;
    padding: 16px;
  }

  .screen[hidden] {
    display: none;
  }
</style>
