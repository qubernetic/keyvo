<script lang="ts">
  // The app shell (docs/design/gui-ux.md §2, d8; pad screen brief §5.1):
  // the header across the top, the navigation rail on the left, the
  // selected screen beside it, and at most one drawer (a trace entry,
  // Preferences or the keyboard map) at the right. The shell holds the
  // inspector's pin and the profile list, read again on every reload, for
  // the header's dropdown, the Pad's page tabs and the Profiles selection
  // (#164). The Pad stays mounted while
  // another screen shows, so the header's device selector follows the
  // daemon and the mirror keeps its last frame.
  import Header from "./Header.svelte";
  import Pad from "./Pad.svelte";
  import Preferences from "./Preferences.svelte";
  import Profiles from "./Profiles.svelte";
  import Rail from "./Rail.svelte";
  import Shortcuts from "./Shortcuts.svelte";
  import TraceDrawer from "./TraceDrawer.svelte";
  import type { Daemon } from "./lib/daemon";
  import type { Editor } from "./lib/editor";
  import { kept, type Pinned } from "./lib/pin";
  import type { ProfileFiles, ProfileSummary } from "./lib/profiles";
  import type { DeviceStatus, TraceRecord, View } from "./lib/protocol";
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
  /** What the pad shows and the daemon's reload count, as the Pad follows them. */
  let view: View | undefined = $state();
  let reloads = $state(0);

  /** The inspector's profile and page; undefined follows the pad. */
  let pinned: Pinned = $state();
  /** Every profile, read again on each reload; empty until the first read. */
  let profiles: ProfileSummary[] = $state([]);

  /** Bumped by every read, so a late answer of an earlier one is dropped. */
  let reads = 0;

  $effect(() => {
    void reloads;
    const mine = ++reads;
    void files.list().then(
      (list) => {
        if (mine !== reads) return;
        profiles = list;
        pinned = kept(pinned, list);
      },
      // Profiles shows the load errors; the dropdown keeps the last list.
      () => {},
    );
  });

  /** The one drawer that is open, and the trace entry it shows. */
  let open: "trace" | "preferences" | "shortcuts" | undefined = $state();
  let entry: TraceRecord | undefined = $state();
</script>

<svelte:window bind:innerWidth={width} />

<div class="app">
  <div class="top">
    <Header
      {device}
      {chosen}
      onchoose={(choice) => (chosen = choice)}
      {profiles}
      pad={view}
      {pinned}
      onpin={(next) => (pinned = next)}
      onpreferences={() => (open = "preferences")}
      onshortcuts={() => (open = "shortcuts")}
    />
  </div>
  <Rail {active} labels={columns.railLabels} onnavigate={(id) => (active = id)} />
  <main>
    <div class="screen" hidden={active !== "home"}>
      <Pad
        {daemon}
        {editor}
        active={active === "home"}
        {columns}
        ondevice={(next) => (device = next)}
        {profiles}
        {pinned}
        onpin={(next) => (pinned = next)}
        onview={(next, count) => {
          view = next;
          reloads = count;
        }}
        ontrace={(next) => {
          entry = next;
          open = "trace";
        }}
      />
    </div>
    {#if active === "profiles"}
      <div class="screen">
        <Profiles
          {daemon}
          {files}
          {columns}
          {view}
          {reloads}
          {pinned}
          onpin={(next) => (pinned = next)}
          geometry={device?.geometry}
          onedit={(pin) => {
            pinned = pin;
            active = "home";
          }}
        />
      </div>
    {/if}
  </main>
</div>

{#if open === "trace" && entry !== undefined && active === "home"}
  <TraceDrawer {entry} onclose={() => (open = undefined)} />
{:else if open === "preferences"}
  <Preferences onclose={() => (open = undefined)} />
{:else if open === "shortcuts"}
  <Shortcuts onclose={() => (open = undefined)} />
{/if}

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

  /* Each screen owns its height and its columns scroll on their own
     (DESIGN.md "Columns"), so main never shows a second bar (#166). */
  main {
    min-width: 0;
    min-height: 0;
    overflow: hidden;
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
