<script lang="ts">
  // Home (docs/design/gui-ux.md §3.1): the 3x3 mirror of the page the pad
  // shows, the status strip, the recent trace entries, and the virtual pad:
  // a click is a short dry-run press, a right click a long one.
  import type { Daemon, Tile } from "./lib/daemon";
  import { apply, fromStatus, HOME_TRACE, initial, summarize, type HomeState } from "./lib/home";
  import type { PressKind, TraceRecord } from "./lib/protocol";

  let { daemon }: { daemon: Daemon } = $props();

  /** The Keypad's grid until the daemon reports the device geometry. */
  const COLUMNS = 3;
  const KEYS = 9;

  let home: HomeState = $state(initial());
  let connecting = $state(true);
  /** Why the daemon cannot be reached; the rest is the last frame. */
  let unreachable: string | undefined = $state();
  let tiles: Tile[] = $state([]);
  let tilesError: string | undefined = $state();
  let selected = $state(1);
  let result: string | undefined = $state();
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

  // The tiles of the page the pad shows, again after every reload.
  let fetched = 0;
  $effect(() => {
    const view = home.view;
    void home.reloads;
    if (view === undefined) return;
    const mine = ++fetched;
    daemon.tiles(view.profile, view.page).then(
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

  function action(tile: Tile | undefined): string {
    switch (tile?.action?.type) {
      case undefined:
        return "unbound";
      case "chord":
        return `chord ${tile.action.keys}`;
      case "exec":
        return `exec ${tile.action.cmd}`;
    }
  }

  function name(key: number): string {
    const tile = tiles[key - 1];
    const label = tile?.label === undefined ? "" : `${tile.label}, `;
    return `Key ${key}: ${label}${action(tile)}`;
  }

  async function press(key: number, kind: PressKind) {
    selected = key;
    try {
      const entry: TraceRecord = await daemon.press(key, kind);
      result = summarize(entry);
    } catch (error) {
      result = `Press failed: ${reason(error)}`;
    }
  }

  function select(key: number) {
    selected = key;
    buttons[key - 1]?.focus();
  }

  /** The key the arrow `arrow` moves to from `key`, inside the grid. */
  function moved(key: number, arrow: string): number {
    const row = Math.floor((key - 1) / COLUMNS);
    const column = (key - 1) % COLUMNS;
    const rows = KEYS / COLUMNS;
    const [r, c] =
      arrow === "ArrowUp"
        ? [Math.max(row - 1, 0), column]
        : arrow === "ArrowDown"
          ? [Math.min(row + 1, rows - 1), column]
          : arrow === "ArrowLeft"
            ? [row, Math.max(column - 1, 0)]
            : [row, Math.min(column + 1, COLUMNS - 1)];
    return r * COLUMNS + c + 1;
  }

  function keydown(event: KeyboardEvent) {
    if (/^[1-9]$/.test(event.key)) {
      event.preventDefault();
      select(Number(event.key));
    } else if (event.key === " ") {
      event.preventDefault();
      void press(selected, event.shiftKey ? "long" : "short");
    } else if (["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(event.key)) {
      event.preventDefault();
      select(moved(selected, event.key));
    }
  }

  function contextmenu(event: MouseEvent, key: number) {
    event.preventDefault();
    void press(key, "long");
  }

  function device(): string {
    const device = home.device;
    if (device === undefined) return "unknown";
    if (device.connected) {
      return device.keys === undefined ? "Keypad connected" : `Keypad connected, ${device.keys} keys`;
    }
    return device.error === undefined ? "No Keypad" : `No Keypad: ${device.error}`;
  }

  function time(entry: TraceRecord): string {
    return new Date(entry.timestamp_ms).toLocaleTimeString();
  }

  const rows = Array.from({ length: KEYS / COLUMNS }, (_, row) =>
    Array.from({ length: COLUMNS }, (_, column) => row * COLUMNS + column + 1),
  );
  const gridName = $derived(
    home.view === undefined
      ? "Keypad mirror"
      : `Keypad mirror: profile ${home.view.profile}, page ${home.view.page}`,
  );
</script>

<section class="home">
  {#if unreachable !== undefined}
    <div role="alert" class="banner error">
      Daemon unreachable: {unreachable}
      <button type="button" onclick={() => void connect()}>Retry</button>
    </div>
  {:else if connecting && home.view === undefined}
    <p class="banner">Connecting to the daemon</p>
  {/if}
  {#if home.configError !== undefined}
    <div role="alert" class="banner error">Configuration not loaded: {home.configError}</div>
  {/if}

  <section class="strip" aria-label="Status">
    <dl>
      <div><dt id="kv-device">Device</dt><dd aria-labelledby="kv-device">{device()}</dd></div>
      <div>
        <dt id="kv-profile">Profile</dt>
        <dd aria-labelledby="kv-profile">{home.view?.profile ?? "unknown"}</dd>
      </div>
      <div>
        <dt id="kv-page">Page</dt>
        <dd aria-labelledby="kv-page">{home.view?.page ?? "unknown"}</dd>
      </div>
      <div>
        <dt id="kv-mode">Mode</dt>
        <dd aria-labelledby="kv-mode">{home.view?.mode ?? "unknown"}</dd>
      </div>
      <div>
        <dt id="kv-context">Context</dt>
        <dd aria-labelledby="kv-context">
          {home.context === undefined
            ? "no window reported"
            : `${home.context.class}: ${home.context.title}`}
        </dd>
      </div>
    </dl>
  </section>

  <div role="grid" tabindex="-1" aria-label={gridName} class="grid" onkeydown={keydown}>
    {#each rows as row, index (index)}
      <div role="row" class="row">
        {#each row as key (key)}
          <div role="gridcell" aria-selected={key === selected}>
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
              {:else}
                <span aria-hidden="true">{key}</span>
              {/if}
            </button>
          </div>
        {/each}
      </div>
    {/each}
  </div>
  {#if tilesError !== undefined}
    <p class="note">Tiles not rendered: {tilesError}</p>
  {/if}

  <p role="status" aria-label="Press result" class="result">
    {result ?? "Click a key for a dry-run press, right click for a long one."}
  </p>

  <section class="events">
    <h2 id="kv-events">Recent events</h2>
    {#if home.trace.length === 0}
      <p class="note">No presses yet.</p>
    {:else}
      <ol aria-label="Recent events">
        {#each home.trace as entry, index (index)}
          <li><time>{time(entry)}</time> {summarize(entry)}</li>
        {/each}
      </ol>
    {/if}
  </section>
</section>

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
    color: var(--kv-fg-muted);
    font-size: 0.85em;
  }

  .strip dd {
    margin: 0;
  }

  .grid {
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-self: flex-start;
  }

  .row {
    display: flex;
    gap: 8px;
  }

  .key {
    width: 118px;
    height: 118px;
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

  .key img {
    display: block;
    width: 100%;
    height: 100%;
  }

  .note,
  .result {
    margin: 0;
    color: var(--kv-fg-muted);
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

  .events time {
    color: var(--kv-fg-muted);
    margin-right: 8px;
  }
</style>
