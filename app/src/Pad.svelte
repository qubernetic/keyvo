<script lang="ts">
  // The Pad screen (pad screen brief §5.2, §6; screen id `home`): the
  // device drawing with the live mirror (Mirror.svelte), the page list
  // tabs (#151), the "pad moved" line, the held caption or the session
  // lock line (#158, the mirror then shows the lock page), and the pinned
  // inspector, which keeps editing the pinned page under the lock. Selecting a key pins the inspector to the pad's
  // profile and page (lib/pin.ts), so it never changes under the user; the
  // drawing keeps following the pad, and outlines the selected key only
  // while it shows the pinned page (#143). The inspector gets the pinned page's
  // tile, the global key under a key that falls through, and Press, a dry
  // run of the selected key whose result shows under the drawing. The
  // unreachable, no-device and configuration notices go through Notice.
  // The trace strip sits under the inspector; the actions catalogue is the
  // right column from 1200 px and collapses into the inspector's Action
  // select below it; its presets are the daemon's recipe packs. An entry
  // dropped on a tile, or clicked in the catalogue with a tile selected,
  // selects that tile and hands it to the inspector: a tool sets the action
  // type, a preset writes its default variant.
  import { untrack } from "svelte";

  import Catalogue from "./Catalogue.svelte";
  import Inspector from "./Inspector.svelte";
  import Mirror from "./Mirror.svelte";
  import Notice from "./Notice.svelte";
  import PageTabs from "./PageTabs.svelte";
  import TraceStrip from "./TraceStrip.svelte";
  import { catalogueOf } from "./lib/catalogue";
  import type { Daemon, Tile } from "./lib/daemon";
  import { t } from "./lib/i18n";
  import {
    apply,
    caption,
    fromStatus,
    initial,
    PAD_TRACE,
    schemaError,
    summarize,
    type PadState,
  } from "./lib/pad";
  import type { Editor, Target } from "./lib/editor";
  import { difference, follow, pinOnSelect, pinPage, target, type Pinned } from "./lib/pin";
  import { contextPage, GLOBAL, leftOfContext, pageList, type ProfileSummary } from "./lib/profiles";
  import type { DeviceStatus, PressKind, Recipes, TraceRecord, View } from "./lib/protocol";
  import { layout, RIGHT_FROM, type Columns } from "./lib/shell";

  let {
    daemon,
    editor,
    active = true,
    columns = layout(RIGHT_FROM),
    profiles = [],
    pinned,
    onpin,
    ondevice,
    onview,
    ontrace,
  }: {
    daemon: Daemon;
    editor: Editor;
    /** False while the shell shows another screen: the Pad keeps following
     * the daemon but leaves the keyboard alone. */
    active?: boolean;
    /** What the window is wide enough for: the catalogue column with `right`. */
    columns?: Columns;
    /** Every profile, for the page tabs; empty until the shell has read them. */
    profiles?: ProfileSummary[];
    /** The inspector's profile and page; undefined follows the pad. */
    pinned?: Pinned;
    /** Sets the pin, which the shell holds. */
    onpin?: (pinned: Pinned) => void;
    /** The device the daemon reports, none while it cannot be reached. */
    ondevice?: (device: DeviceStatus | undefined) => void;
    /** What the pad shows, none until the daemon reports it, and the count
     * of configuration reloads, for the Profiles screen. */
    onview?: (view: View | undefined, reloads: number) => void;
    /** Opens a trace entry in the drawer, which the shell shows. */
    ontrace?: (entry: TraceRecord) => void;
  } = $props();

  /** The skeleton tiles of the drawing while the Pad connects. */
  const SKELETON = [1, 2, 3, 4, 5, 6, 7, 8, 9];

  let pad: PadState = $state(initial());
  let connecting = $state(true);
  /** Why the daemon cannot be reached; the rest is the last frame. */
  let unreachable: string | undefined = $state();
  let tiles: Tile[] = $state([]);
  let tilesError: string | undefined = $state();
  /** The key selected on the mirror; none until the user selects one. */
  let selected: number | undefined = $state();
  let result: string | undefined = $state();
  /** The entry of the last dry-run press, for the result line's drawer. */
  let pressed: TraceRecord | undefined = $state();

  let stop: (() => void) | undefined;
  /** Bumped by every connect, so a late answer of an earlier one is dropped. */
  let attempt = 0;

  function reason(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  async function connect() {
    const mine = ++attempt;
    stop?.();
    stop = undefined;
    connecting = true;
    try {
      const status = await daemon.status();
      if (mine !== attempt) return;
      pad = { ...fromStatus(status), trace: pad.trace, reloads: pad.reloads };
      unreachable = undefined;
      const unlisten = await daemon.subscribe(PAD_TRACE, (update) => {
        if (mine !== attempt) return;
        if (update.kind === "subscribed") {
          // The replay that follows is the newest entries again.
          pad = { ...pad, trace: [] };
        }
        pad = apply(pad, update);
        unreachable = pad.closed;
      });
      if (mine !== attempt) {
        unlisten();
        return;
      }
      stop = unlisten;
    } catch (error) {
      if (mine === attempt) unreachable = reason(error);
    } finally {
      if (mine === attempt) connecting = false;
    }
  }

  // Connects once per mount; a change of another prop does not reconnect.
  $effect(() => {
    void untrack(connect);
    return () => {
      attempt++;
      stop?.();
      stop = undefined;
    };
  });

  $effect(() => {
    ondevice?.(unreachable === undefined ? pad.device : undefined);
  });

  $effect(() => {
    onview?.(pad.view, pad.reloads);
  });

  /** Writes of the key editor, so the tiles follow them without a daemon. */
  let edits = $state(0);

  // The tiles of the page the pad shows, again after every reload and edit.
  let fetched = 0;
  $effect(() => {
    const view = pad.view;
    const geometry = pad.geometry;
    void pad.reloads;
    void edits;
    if (view === undefined || geometry === undefined) return;
    const mine = ++fetched;
    // Under the session lock the pad shows the lock page.
    const answer = untrack(() =>
      view.locked === true
        ? daemon.tiles(view.profile, view.page, geometry.rects.length, geometry.tile, true)
        : daemon.tiles(view.profile, view.page, geometry.rects.length, geometry.tile),
    );
    answer.then(
      (got) => {
        if (mine !== fetched) return;
        tiles = got;
        tilesError = undefined;
      },
      (error: unknown) => {
        if (mine !== fetched) return;
        tiles = [];
        tilesError = reason(error);
      },
    );
  });

  /** The daemon's recipe packs, the user packs it left out, and why they
   * could not be read; read again after every reload and reconnect. */
  let recipes: Recipes = $state.raw({ recipes: [] });
  let recipesError: string | undefined = $state();
  const reachable = $derived(unreachable === undefined);
  /** The daemon answered: there is a pad to select a key on. */
  const connected = $derived(reachable && !connecting);
  const reloads = $derived(pad.reloads);
  let fetchedRecipes = 0;
  $effect(() => {
    void reloads;
    if (!reachable) return;
    const mine = ++fetchedRecipes;
    untrack(() => daemon.recipes()).then(
      (got) => {
        if (mine !== fetchedRecipes) return;
        recipes = got;
        recipesError = undefined;
      },
      (error: unknown) => {
        if (mine === fetchedRecipes) recipesError = reason(error);
      },
    );
  });
  const catalogue = $derived(catalogueOf(recipes.recipes));

  /** The profile and page the inspector and the tabs show: the pin, else
   * the pad's. */
  const shown = $derived(
    pinned ?? (pad.view === undefined ? undefined : { profile: pad.view.profile, page: pad.view.page }),
  );
  const moved = $derived(difference(pinned, pad.view));
  const globalProfile = $derived(profiles.find((profile) => profile.id === GLOBAL));
  /** The profile whose page list the tabs show (spec §6.3, #151): the
   * pinned one, unless the pin is a global page every list holds at index 0
   * and below; else the one the pad walks. */
  const owner = $derived(
    pinned !== undefined && !(pinned.profile === GLOBAL && leftOfContext(globalProfile, pinned.page))
      ? pinned.profile
      : (pad.owner ?? pad.view?.profile),
  );
  const ownerProfile = $derived(profiles.find((profile) => profile.id === owner));
  const pageTabs = $derived(ownerProfile === undefined ? [] : pageList(ownerProfile, globalProfile));
  /** The selected tab: the pinned page's, else the index the daemon reports. */
  const selectedTab = $derived(
    pinned === undefined
      ? pad.view?.index
      : pageTabs.find((tab) => tab.profile === pinned.profile && tab.page === pinned.page)?.index,
  );

  /** The tiles of the pinned page while the pad shows another one. */
  let pinnedTiles: Tile[] = $state([]);
  let fetchedPinned = 0;
  $effect(() => {
    const pin = moved === undefined ? undefined : shown;
    const geometry = pad.geometry;
    void pad.reloads;
    void edits;
    const mine = ++fetchedPinned;
    if (pin === undefined || geometry === undefined) {
      pinnedTiles = [];
      return;
    }
    untrack(() => daemon.tiles(pin.profile, pin.page, geometry.rects.length, geometry.tile)).then(
      (got) => {
        if (mine === fetchedPinned) pinnedTiles = got;
      },
      () => {
        if (mine === fetchedPinned) pinnedTiles = [];
      },
    );
  });
  /** The context line: the session lock, else the held caption. */
  const held = $derived(pad.view?.locked === true ? t("pad.locked") : caption(pad));
  /** A profile's name, as the header names it; its id while the list does
   * not know it. */
  const nameOf = (id: string) => profiles.find((profile) => profile.id === id)?.name ?? id;
  /** The "pad moved" line: what the inspector edits, then what the pad
   * shows, both by name. */
  const movedLine = $derived.by(() => {
    if (moved === undefined || shown === undefined) return undefined;
    const pinned = nameOf(shown.profile);
    const profile = nameOf(moved.profile);
    return selected === undefined
      ? t("pad.movedPage", { pinned, pinnedPage: shown.page, profile, page: moved.page })
      : t("pad.moved", { pinned, key: selected, profile, page: moved.page });
  });

  function select(key: number) {
    selected = key;
    const next = pinOnSelect(pinned, pad.view);
    if (next !== pinned) onpin?.(next);
  }

  /** The catalogue entry the inspector applies to its key. */
  let assign: { entry: string; target: Target; seq: number } | undefined = $state();

  /** Selects `key` and hands the inspector the catalogue entry with key
   * `entry` for it. */
  function assignTo(key: number, entry: string) {
    const at = target(pinOnSelect(pinned, pad.view), pad.view, key);
    if (at === undefined) return;
    select(key);
    hand(entry, at);
  }

  function hand(entry: string, at: Target) {
    assign = { entry, target: at, seq: (assign?.seq ?? 0) + 1 };
  }

  function choosePage(profile: string, page: number) {
    onpin?.(pinPage(profile, page));
  }

  async function press(key: number, kind: PressKind) {
    pressed = undefined;
    try {
      const entry: TraceRecord = await daemon.press(key, kind);
      result = summarize(entry);
      pressed = entry;
    } catch (error) {
      result = t("home.pressFailed", { reason: reason(error) });
    }
  }

  const configMessage = $derived.by(() => {
    const error = pad.configError;
    if (error === undefined) return undefined;
    const place = schemaError(error);
    if (place === undefined) return t("home.configError", { error });
    return place.column === undefined
      ? t("home.schemaErrorLine", { file: place.file, line: place.line, message: place.message })
      : t("home.schemaError", {
          file: place.file,
          line: place.line,
          column: place.column,
          message: place.message,
        });
  });

  const editing = $derived(selected === undefined ? undefined : target(pinned, pad.view, selected));
  /** The inspector's key as its page renders it. */
  const editingTile = $derived(
    selected === undefined ? undefined : (moved === undefined ? tiles : pinnedTiles)[selected - 1],
  );
  /** The global profile's key under the inspector's key when it falls
   * through (spec §6.1). */
  const fallThrough = $derived(
    editing !== undefined && editing.profile !== GLOBAL && editingTile?.layer === "global"
      ? {
          profile: GLOBAL,
          page: contextPage(profiles.find((profile) => profile.id === GLOBAL)),
          key: editing.key,
        }
      : undefined,
  );
  const gridName = $derived(
    pad.view === undefined
      ? t("home.mirror")
      : t("home.mirrorView", { profile: pad.view.profile, page: pad.view.page }),
  );
</script>

<section class="pad">
  {#if unreachable !== undefined}
    <Notice kind="error">
      {t("home.unreachable", { reason: unreachable })}
      <span class="hint">{t("pad.startWith")} <code>{t("pad.daemonCommand")}</code>.</span>
      <button type="button" aria-label={t("home.retry")} onclick={() => void connect()}>
        {t("home.retry")}
      </button>
    </Notice>
  {/if}
  {#if configMessage !== undefined}
    <Notice kind="error">{configMessage}</Notice>
  {/if}
  {#if pad.device?.connected === false}
    <Notice kind="note">
      {pad.seen === undefined
        ? t("home.missingNever")
        : t("home.missing", { time: new Date(pad.seen).toLocaleTimeString() })}
    </Notice>
  {/if}

  <div class="workspace">
    <div class="drawing">
      {#if pad.geometry !== undefined}
        <Mirror
          geometry={pad.geometry}
          {tiles}
          profile={pad.view?.profile}
          held={pad.held}
          selected={moved === undefined ? selected : undefined}
          name={gridName}
          onselect={select}
          onpress={(key, kind) => void press(key, kind)}
          ondrop={assignTo}
        />
      {:else if connecting}
        <!-- §5.2: skeleton tiles, no spinner. -->
        <ul class="skeleton" aria-busy="true" aria-label={t("home.connecting")}>
          {#each SKELETON as tile (tile)}
            <li><span class="bar"></span></li>
          {/each}
        </ul>
      {:else if pad.view !== undefined}
        <p class="note">{t("home.noGeometry")}</p>
      {/if}
      {#if tilesError !== undefined}
        <p class="note">{t("home.tilesError", { error: tilesError })}</p>
      {/if}
      {#if held !== undefined}
        <p role="status" aria-label={t("pad.context")} class="note">{held}</p>
      {/if}
      {#if ownerProfile !== undefined}
        <PageTabs
          profile={ownerProfile}
          global={globalProfile}
          tabs={pageTabs}
          selected={selectedTab}
          onpage={choosePage}
        />
      {/if}
      {#if moved !== undefined}
        <p class="moved">
          {movedLine}
          <button type="button" aria-label={t("pad.follow")} onclick={() => onpin?.(follow())}>
            {t("pad.follow")}
          </button>
        </p>
      {/if}
      <p role="status" aria-label={t("home.result")} class="result">
        {result ?? (connected ? t("pad.resultHint") : "")}
        {#if pressed !== undefined}
          {@const entry = pressed}
          <button
            type="button"
            class="entry"
            aria-label={t("home.showEntry")}
            onclick={() => ontrace?.(entry)}
          >
            {t("home.showEntry")}
          </button>
        {/if}
      </p>
    </div>
    <div class="inspector">
      <Inspector
        {editor}
        target={editing}
        global={fallThrough}
        tile={editingTile}
        {active}
        {connected}
        revision={pad.reloads}
        onpress={selected === undefined ? undefined : () => void press(selected ?? 0, "short")}
        onedited={() => edits++}
        {assign}
        entries={columns.right ? undefined : catalogue}
        recipes={recipes.recipes}
      />
      <TraceStrip entries={pad.trace} {connecting} onopen={(entry) => ontrace?.(entry)} />
    </div>
    {#if columns.right}
      <div class="catalogue">
        <Catalogue
          entries={catalogue}
          problems={recipes.problems}
          unread={recipesError}
          {selected}
          {active}
          {connected}
          onassign={(entry) => {
            if (editing !== undefined) hand(entry, editing);
          }}
        />
      </div>
    {/if}
  </div>
</section>

<style>
  .pad {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .hint {
    display: block;
    margin: 4px 0;
  }

  .workspace {
    display: flex;
    flex-wrap: wrap;
    gap: 24px;
    align-items: flex-start;
  }

  /* The drawing takes the column's width up to the panel's pixel width. */
  .drawing {
    display: flex;
    flex: 1 1 300px;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }

  .inspector {
    display: flex;
    flex: 1 1 300px;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }

  .catalogue {
    flex: none;
  }

  .skeleton {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
    max-width: 512px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .skeleton .bar {
    display: block;
    aspect-ratio: 1;
    border-radius: var(--kv-radius);
    background: var(--kv-bg-alt);
    color: var(--kv-fg-muted);
  }

  .moved {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin: 0;
  }

  .note {
    margin: 0;
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
  }

  .result {
    margin: 0;
    background: var(--kv-bg);
    color: var(--kv-fg);
  }

  .entry {
    border-color: transparent;
    background: var(--kv-bg);
    color: var(--kv-fg);
    text-align: start;
  }

</style>
