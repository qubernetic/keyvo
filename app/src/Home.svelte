<script lang="ts">
  // Home (docs/design/gui-ux.md §3.1): the mirror of the page the pad
  // shows, laid out by the geometry the daemon reports in the cutouts of
  // the Keypad drawing (Device.svelte), the
  // status strip, the recent trace entries, and the virtual pad:
  // a click is a short dry-run press, a right click a long one. Beside the
  // mirror, the key editor for the selected key.
  import Device from "./Device.svelte";
  import KeyEditor from "./KeyEditor.svelte";
  import TraceDrawer from "./TraceDrawer.svelte";
  import type { Daemon, Outline, Tile } from "./lib/daemon";
  import type { Editor, Target } from "./lib/editor";
  import {
    apply,
    describeAction,
    fromStatus,
    grid,
    HOME_TRACE,
    initial,
    moved,
    phrase,
    schemaError,
    summarize,
    type HomeState,
  } from "./lib/home";
  import { t, type MessageKey } from "./lib/i18n";
  import type { DeviceStatus, PressKind, TraceRecord, View } from "./lib/protocol";
  import { SCREENS, type ScreenId } from "./lib/shell";

  let {
    daemon,
    editor,
    active = true,
    ondevice,
    onview,
    onnavigate,
  }: {
    daemon: Daemon;
    editor: Editor;
    /** False while the shell shows another screen: Home keeps following the
     * daemon but leaves the keyboard alone. */
    active?: boolean;
    /** The device the daemon reports, none while it cannot be reached. */
    ondevice?: (device: DeviceStatus | undefined) => void;
    /** Opens a screen; a status fact links to the screen that owns it. */
    onnavigate?: (screen: ScreenId) => void;
    /** What the pad shows, none until the daemon reports it, and the count
     * of configuration reloads, for the Profiles screen. */
    onview?: (view: View | undefined, reloads: number) => void;
  } = $props();

  /** The skeleton rows of the event list while Home connects. */
  const SKELETON = [1, 2, 3];

  let home: HomeState = $state(initial());
  /** The key numbers in rows, as the daemon's geometry lays them out. */
  const rows: number[][] = $derived(home.geometry === undefined ? [] : grid(home.geometry));
  const keyCount = $derived(home.geometry?.rects.length ?? 0);
  let connecting = $state(true);
  /** Why the daemon cannot be reached; the rest is the last frame. */
  let unreachable: string | undefined = $state();
  let tiles: Tile[] = $state([]);
  let tilesError: string | undefined = $state();
  let selected = $state(1);
  let result: string | undefined = $state();
  /** The entry of the last dry-run press, for the result line's drawer. */
  let pressed: TraceRecord | undefined = $state();
  const buttons: HTMLButtonElement[] = $state([]);

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
      home = { ...fromStatus(status), trace: home.trace, reloads: home.reloads };
      unreachable = undefined;
      const unlisten = await daemon.subscribe(HOME_TRACE, (update) => {
        if (mine !== attempt) return;
        if (update.kind === "subscribed") {
          // The replay that follows is the newest entries again.
          home = { ...home, trace: [] };
        }
        home = apply(home, update);
        unreachable = home.closed;
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

  $effect(() => {
    void connect();
    return () => {
      attempt++;
      stop?.();
      stop = undefined;
    };
  });

  $effect(() => {
    ondevice?.(unreachable === undefined ? home.device : undefined);
  });

  $effect(() => {
    onview?.(home.view, home.reloads);
  });

  /** Writes of the key editor, so the tiles follow them without a daemon. */
  let edits = $state(0);

  /** The page the user picked in the page selector; the pad's next view
   * drops it. */
  let picked: number | undefined = $state();
  $effect(() => {
    void home.view;
    picked = undefined;
  });
  /** The page the mirror shows: the picked one, else the pad's. */
  const shown = $derived(picked ?? home.view?.page);

  // The page count and accent of the pad's profile, again after a reload.
  let outline: Outline | undefined = $state();
  let outlined = 0;
  $effect(() => {
    const profile = home.view?.profile;
    void home.reloads;
    if (profile === undefined) return;
    const mine = ++outlined;
    daemon.outline(profile).then(
      (got) => {
        if (mine === outlined) outline = got;
      },
      () => {
        if (mine === outlined) outline = undefined;
      },
    );
  });
  /** The pages the selector offers; at least up to the pad's page. */
  const pages = $derived(
    Array.from(
      { length: Math.max(outline?.pages ?? 0, home.view?.page ?? 0) },
      (_, index) => index + 1,
    ),
  );

  // The tiles of the page the mirror shows, again after every reload and edit.
  let fetched = 0;
  $effect(() => {
    const profile = home.view?.profile;
    const page = shown;
    const geometry = home.geometry;
    void home.reloads;
    void edits;
    if (profile === undefined || page === undefined || geometry === undefined) return;
    const mine = ++fetched;
    daemon.tiles(profile, page, geometry.rects.length, geometry.tile).then(
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

  function name(key: number): string {
    const tile = tiles[key - 1];
    const parts = phrase([
      tile?.label,
      describeAction(tile?.action) ?? t("home.unbound"),
      tile?.layer === undefined ? undefined : t("layer", { layer: tile.layer }),
    ]);
    return t("home.key", { key, parts });
  }

  /** A global key under an app profile (spec §6.1), which the mirror marks. */
  function fallen(key: number): boolean {
    return tiles[key - 1]?.layer === "global" && home.view?.profile !== "global";
  }

  async function press(key: number, kind: PressKind) {
    selected = key;
    pressed = undefined;
    const pad = home.view?.page;
    if (pad !== undefined && shown !== pad) {
      // The daemon resolves a press on the page the pad shows.
      result = t("home.otherPage", { page: pad });
      return;
    }
    try {
      const entry: TraceRecord = await daemon.press(key, kind);
      result = summarize(entry);
      pressed = entry;
    } catch (error) {
      result = t("home.pressFailed", { reason: reason(error) });
    }
  }

  /** The entry the trace drawer shows; the drawer gives the focus back. */
  let drawn: TraceRecord | undefined = $state();

  function draw(entry: TraceRecord) {
    drawn = entry;
  }

  function undraw() {
    drawn = undefined;
  }

  function select(key: number) {
    selected = key;
    buttons[key - 1]?.focus();
  }

  function keydown(event: KeyboardEvent) {
    if (/^[1-9]$/.test(event.key) && Number(event.key) <= keyCount) {
      event.preventDefault();
      select(Number(event.key));
    } else if (event.key === " ") {
      event.preventDefault();
      void press(selected, event.shiftKey ? "long" : "short");
    } else if (["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(event.key)) {
      event.preventDefault();
      select(moved(rows, selected, event.key));
    }
  }

  function contextmenu(event: MouseEvent, key: number) {
    event.preventDefault();
    void press(key, "long");
  }

  function device(): string {
    const device = home.device;
    if (device === undefined) return t("home.unknown");
    if (device.connected) {
      return device.keys === undefined
        ? t("home.keypad")
        : t("home.keypadKeys", { keys: device.keys });
    }
    return device.error === undefined
      ? t("home.noKeypad")
      : t("home.noKeypadError", { error: device.error });
  }

  function time(entry: TraceRecord): string {
    return new Date(entry.timestamp_ms).toLocaleTimeString();
  }

  /** One fact of the status strip, a link when its screen is built. */
  interface Fact {
    id: string;
    label: MessageKey;
    value: string;
    link?: ScreenId;
    linkLabel?: string;
  }

  /** `screen` when it is built, so a fact can link to it. */
  function built(screen: ScreenId | undefined): ScreenId | undefined {
    const entry = SCREENS.find((entry) => entry.id === screen);
    return entry !== undefined && entry.soon === undefined ? entry.id : undefined;
  }

  /** A fact; `owner` is the rail screen it links to, none for a fact whose
   * owner lives in Preferences. */
  function fact(id: string, label: MessageKey, value: string, owner?: ScreenId): Fact {
    const link = built(owner);
    const screen = SCREENS.find((entry) => entry.id === owner);
    return {
      id,
      label,
      value,
      ...(link === undefined || screen === undefined
        ? {}
        : {
            link,
            linkLabel: t("home.link", {
              fact: t("home.fact", { label: t(label), value }),
              screen: t(screen.label),
            }),
          }),
    };
  }

  // The focused window; a held one (`[focus].hold_classes`, the keyvo app)
  // names the window whose context stays.
  function focused(): string {
    const context = home.context;
    if (context === undefined) return t("home.noWindow");
    if (context.held !== true) return t("home.window", { class: context.class, title: context.title });
    return home.holding === undefined
      ? t("home.holdingUnknown", { class: context.class })
      : t("home.holding", { class: context.class, holding: home.holding.class });
  }

  // Design detail: the screen that owns each fact (§3.1). Devices,
  // overlays (from plugins and integrations) and the context's project
  // belong to Preferences tabs, so they link nowhere.
  const facts: Fact[] = $derived([
    fact("device", "home.device", device()),
    fact("profile", "home.profile", home.view?.profile ?? t("home.unknown"), "profiles"),
    fact("page", "home.page", String(home.view?.page ?? t("home.unknown")), "profiles"),
    fact("mode", "home.mode", home.view?.mode ?? t("home.unknown"), "profiles"),
    fact("overlay", "home.overlay", t("home.noOverlay")),
    fact("context", "home.context", phrase([t("home.unknownProject"), focused()])),
  ]);

  const configMessage = $derived.by(() => {
    const error = home.configError;
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

  const target: Target | undefined = $derived(
    home.view === undefined || shown === undefined
      ? undefined
      : { profile: home.view.profile, page: shown, key: selected },
  );
  const gridName = $derived(
    home.view === undefined || shown === undefined
      ? t("home.mirror")
      : t("home.mirrorView", { profile: home.view.profile, page: shown }),
  );
</script>

<section class="home">
  {#if unreachable !== undefined}
    <div role="alert" class="banner error">
      {t("home.unreachable", { reason: unreachable })}
      <button type="button" aria-label={t("home.retry")} onclick={() => void connect()}>
        {t("home.retry")}
      </button>
    </div>
  {/if}
  <section class="strip" aria-label={t("home.status")}>
    <dl>
      {#each facts as fact (fact.id)}
        <div>
          <dt id={`kv-${fact.id}`}>{t(fact.label)}</dt>
          <dd aria-labelledby={`kv-${fact.id}`}>
            {#if fact.link !== undefined}
              {@const link = fact.link}
              <button
                type="button"
                class="link"
                aria-label={fact.linkLabel}
                onclick={() => onnavigate?.(link)}
              >
                {fact.value}
              </button>
            {:else}
              {fact.value}
            {/if}
          </dd>
        </div>
      {/each}
    </dl>
    {#if home.configError !== undefined}
      <p role="alert" class="error">
        {configMessage}
        <button
          type="button"
          class="link"
          aria-label={t("home.open", { screen: t("rail.profiles") })}
          onclick={() => onnavigate?.("profiles")}
        >
          {t("home.open", { screen: t("rail.profiles") })}
        </button>
      </p>
    {/if}
  </section>

  {#if home.device?.connected === false}
    <p class="note">
      {home.seen === undefined
        ? t("home.missingNever")
        : t("home.missing", { time: new Date(home.seen).toLocaleTimeString() })}
    </p>
  {/if}

  <div class="workspace">
    {#if home.geometry === undefined}
      {#if home.view !== undefined}
        <p class="note">{t("home.noGeometry")}</p>
      {/if}
    {:else}
    <div class="mirror">
    <div class="pages">
      <label>
        {t("home.pageShown")}
        <select
          aria-label={t("home.pageShown")}
          value={String(shown)}
          onchange={(event) => (picked = Number(event.currentTarget.value))}
        >
          {#each pages as page (page)}
            <option value={String(page)}>{t("home.pageOption", { page })}</option>
          {/each}
        </select>
      </label>
      {#if outline?.accent !== undefined && home.view !== undefined}
        <div
          role="img"
          class="accent"
          aria-label={t("home.accent", { profile: home.view.profile })}
          style:background-color={outline.accent}
        ></div>
      {/if}
    </div>
    <div role="grid" tabindex="-1" aria-label={gridName} class="grid" onkeydown={keydown}>
      <Device geometry={home.geometry}>
        {#snippet row(cells)}
          <div role="row" class="row">{@render cells()}</div>
        {/snippet}
        {#snippet tile(key, place)}
          <div role="gridcell" aria-selected={key === selected} style={place}>
            <button
              bind:this={buttons[key - 1]}
              type="button"
              class="key"
              class:held={home.held.includes(key)}
              tabindex={key === selected ? 0 : -1}
              aria-label={name(key)}
              onclick={() => void press(key, "short")}
              oncontextmenu={(event) => contextmenu(event, key)}
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
    </div>
    {/if}
    <KeyEditor {editor} {target} {active} revision={home.reloads} onedited={() => edits++} />
  </div>
  {#if tilesError !== undefined}
    <p class="note">{t("home.tilesError", { error: tilesError })}</p>
  {/if}

  <p role="status" aria-label={t("home.result")} class="result">
    {result ?? t("home.resultHint")}
    {#if pressed !== undefined}
      {@const entry = pressed}
      <button
        type="button"
        class="link"
        aria-label={t("home.showEntry")}
        onclick={() => draw(entry)}
      >
        {t("home.showEntry")}
      </button>
    {/if}
  </p>

  <section class="events">
    <div class="head">
      <h2 id="kv-events">{t("home.events")}</h2>
    </div>
    {#if connecting && home.trace.length === 0}
      <!-- §5.2: skeleton rows, no spinner; the mirror keeps its last frame. -->
      <ul class="skeleton" aria-busy="true" aria-label={t("home.connecting")}>
        {#each SKELETON as row (row)}
          <li><span class="bar"></span></li>
        {/each}
      </ul>
    {:else if home.trace.length === 0}
      <p class="note">{t("home.noEvents")}</p>
    {:else}
      <ol aria-label={t("home.events")}>
        {#each home.trace as entry, index (index)}
          <li>
            <button
              type="button"
              class="entry"
              aria-label={t("home.openEntry", { line: summarize(entry) })}
              onclick={() => draw(entry)}
            >
              <time>{time(entry)}</time>
              {summarize(entry)}
            </button>
          </li>
        {/each}
      </ol>
    {/if}
  </section>
</section>

{#if drawn !== undefined && active}
  <TraceDrawer entry={drawn} onclose={undraw} />
{/if}

<style>
  .home {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .banner {
    margin: 0;
    padding: 8px 12px;
    background: var(--kv-bg-alt);
    color: var(--kv-fg);
    border-radius: var(--kv-radius);
  }

  .error {
    border-left: 4px solid var(--kv-state-error);
  }

  .strip dl {
    display: flex;
    flex-wrap: wrap;
    gap: 8px 24px;
    margin: 0;
  }

  .strip dt {
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
    font-size: 0.85em;
  }

  .strip dd {
    margin: 0;
  }

  .link {
    padding: 0 4px;
    border-color: transparent;
    background: var(--kv-bg);
    color: var(--kv-accent);
    text-decoration: underline;
  }

  .strip .error {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 8px 0 0;
    padding: 4px 12px;
    border-radius: var(--kv-radius);
    background: var(--kv-bg-alt);
    color: var(--kv-fg);
  }

  .strip .error .link {
    background: var(--kv-bg-alt);
    color: var(--kv-accent);
  }

  .workspace {
    display: flex;
    flex-wrap: wrap;
    gap: 24px;
    align-items: flex-start;
  }

  /* The drawing takes the column's width up to the panel's pixel width. */
  .mirror {
    display: flex;
    flex: 1 1 300px;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }

  .pages {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .pages label {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .accent {
    flex: 1;
    min-width: 48px;
    height: 6px;
    border-radius: var(--kv-radius);
  }

  .grid {
    width: 100%;
  }

  /* Device places the cells; the rows only group them for the grid role. */
  .row {
    display: contents;
  }

  .key {
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
    border-color: var(--kv-accent);
    outline: none;
  }

  .key.held {
    border-color: var(--kv-fg);
  }

  .key {
    position: relative;
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

  .note,
  .result {
    margin: 0;
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
  }

  .events .head {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 0 0 4px;
  }

  .events h2 {
    margin: 0;
    font-size: 1em;
  }

  .entry {
    width: 100%;
    border-color: transparent;
    background: var(--kv-bg);
    color: var(--kv-fg);
    text-align: left;
  }

  .events ol {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .skeleton {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .skeleton .bar {
    display: block;
    width: 60%;
    height: 1em;
    border-radius: var(--kv-radius);
    background: var(--kv-bg-alt);
    color: var(--kv-fg-muted);
  }

  .events time {
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
    margin-right: 8px;
  }
</style>
