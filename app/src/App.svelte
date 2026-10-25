<script lang="ts">
  // The app shell (docs/design/gui-ux.md §2, d8): the header across the
  // top, the navigation rail on the left, the selected screen beside it.
  // Home stays mounted while another screen shows, so the header's device
  // selector follows the daemon and the mirror keeps its last frame.
  import Columns from "./Columns.svelte";
  import Header from "./Header.svelte";
  import Home from "./Home.svelte";
  import Rail from "./Rail.svelte";
  import type { Daemon } from "./lib/daemon";
  import type { Editor } from "./lib/editor";
  import { t } from "./lib/i18n";
  import type { DeviceStatus } from "./lib/protocol";
  import { layout, type DeviceChoice, type ScreenId } from "./lib/shell";

  let { daemon, editor }: { daemon: Daemon; editor: Editor } = $props();

  let width = $state(window.innerWidth);
  const columns = $derived(layout(width));
  let active: ScreenId = $state("home");
  let device: DeviceStatus | undefined = $state();
  let chosen: DeviceChoice | undefined = $state();
</script>

<svelte:window bind:innerWidth={width} />

<div class="app">
  <div class="top">
    <Header {device} {chosen} onchoose={(choice) => (chosen = choice)} />
  </div>
  <Rail {active} labels={columns.railLabels} onnavigate={(id) => (active = id)} />
  <main>
    <div class="screen" hidden={active !== "home"}>
      <Home {daemon} {editor} active={active === "home"} ondevice={(next) => (device = next)} />
    </div>
    {#if active === "profiles"}
      <div class="screen">
        <Columns {columns}>
          {#snippet list()}
            <p class="empty">{t("profiles.list")}</p>
          {/snippet}
          {#snippet editor()}
            <p class="empty">{t("profiles.editor")}</p>
          {/snippet}
          {#snippet right()}
            <p class="empty">{t("profiles.right")}</p>
          {/snippet}
        </Columns>
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

  .empty {
    margin: 0;
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
  }
</style>
