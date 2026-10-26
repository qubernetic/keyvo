<script lang="ts">
  // The Pad screen (pad screen brief §5.2, §6; screen id `home`): the
  // device drawing with the live mirror (Mirror.svelte), the page tabs of
  // the pinned profile, the "pad moved" line, the held caption, and the
  // inspector, today's key editor until the inspector slice. Selecting a
  // key pins the inspector to the pad's profile and page (lib/pin.ts), so
  // it never changes under the user; the drawing keeps following the pad.
  // The unreachable, no-device and configuration notices go through Notice.
  import { untrack } from "svelte";

  import KeyEditor from "./KeyEditor.svelte";
  import Mirror from "./Mirror.svelte";
  import Notice from "./Notice.svelte";
  import PageTabs from "./PageTabs.svelte";
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
  import type { Editor } from "./lib/editor";
  import { difference, follow, pinOnSelect, pinPage, target, type Pinned } from "./lib/pin";
  import type { ProfileSummary } from "./lib/profiles";
  import type { DeviceStatus, PressKind, TraceRecord, View } from "./lib/protocol";

  let {
    daemon,
    editor,
    active = true,
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
    const answer = untrack(() =>
      daemon.tiles(view.profile, view.page, geometry.rects.length, geometry.tile),
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

  /** The profile and page the inspector and the tabs show: the pin, else
   * the pad's. */
  const shown = $derived(
    pinned ?? (pad.view === undefined ? undefined : { profile: pad.view.profile, page: pad.view.page }),
  );
  const shownProfile = $derived(profiles.find((profile) => profile.id === shown?.profile));
  const moved = $derived(difference(pinned, pad.view));
  const held = $derived(caption(pad));

  function select(key: number) {
    selected = key;
    const next = pinOnSelect(pinned, pad.view);
    if (next !== pinned) onpin?.(next);
  }

  function choosePage(page: number) {
    if (shown !== undefined) onpin?.(pinPage(shown, page));
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

  function time(entry: TraceRecord): string {
    return new Date(entry.timestamp_ms).toLocaleTimeString();
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
          {selected}
          name={gridName}
          onselect={select}
          onpress={(key, kind) => void press(key, kind)}
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
      {#if shownProfile !== undefined && shown !== undefined}
        <PageTabs profile={shownProfile} page={shown.page} onpage={choosePage} />
      {/if}
      {#if moved !== undefined}
        <p class="moved">
          {t("pad.moved", { profile: moved.profile, page: moved.page })}
          <button type="button" aria-label={t("pad.follow")} onclick={() => onpin?.(follow())}>
            {t("pad.follow")}
          </button>
        </p>
      {/if}
      <p role="status" aria-label={t("home.result")} class="result">
        {result ?? t("pad.resultHint")}
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
      <KeyEditor {editor} target={editing} {active} revision={pad.reloads} onedited={() => edits++} />
    </div>
  </div>

  <section class="events" aria-labelledby="kv-events">
    <h2 id="kv-events">{t("home.events")}</h2>
    {#if pad.trace.length === 0}
      {#if !connecting}
        <p class="note">{t("home.noEvents")}</p>
      {/if}
    {:else}
      <ol aria-label={t("home.events")}>
        {#each pad.trace as entry, index (index)}
          <li>
            <button
              type="button"
              class="entry"
              aria-label={t("home.openEntry", { line: summarize(entry) })}
              onclick={() => ontrace?.(entry)}
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
    flex: 1 1 300px;
    min-width: 0;
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
    text-align: left;
  }

  .events h2 {
    margin: 0 0 4px;
    font-size: 1em;
  }

  .events ol {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .events .entry {
    width: 100%;
  }

  .events time {
    margin-right: 8px;
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
  }
</style>
