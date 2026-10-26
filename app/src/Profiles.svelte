<script lang="ts">
  // Profiles (docs/design/gui-ux.md §3.2, §4.1; d1, d2, d14, ADR-0004):
  // the list column with the profiles by priority, the global one pinned
  // first and the active one marked; the editor with the profile header
  // (name and accent written the moment they change, after a snapshot),
  // the match rules, the pages as tabs, the key grid of the selected page
  // and the key editor of the selected key; the right column with the
  // mirror of the selected page and the key card. What is not built yet is
  // shown disabled (Soon.svelte).
  import Columns from "./Columns.svelte";
  import KeyEditor from "./KeyEditor.svelte";
  import Soon from "./Soon.svelte";
  import type { Daemon, Tile } from "./lib/daemon";
  import { describe, problemOf, type Editor, type Problem, type Target } from "./lib/editor";
  import { grid, moved } from "./lib/home";
  import { t } from "./lib/i18n";
  import {
    fromHeaderForm,
    GLOBAL,
    ordered,
    pageName,
    sameHeader,
    toHeaderForm,
    type HeaderForm,
    type ProfileFiles,
    type ProfileSummary,
    type ProfileView,
  } from "./lib/profiles";
  import type { Geometry, View } from "./lib/protocol";
  import type { Columns as ColumnLayout } from "./lib/shell";

  let {
    daemon,
    editor: keys,
    files,
    columns,
    view,
    geometry,
    reloads = 0,
    active = true,
  }: {
    daemon: Daemon;
    editor: Editor;
    files: ProfileFiles;
    columns: ColumnLayout;
    /** What the pad shows; its profile is the active one. */
    view?: View;
    /** The Keypad's geometry; the mirror needs it. */
    geometry?: Geometry;
    /** Bumped by every configuration reload of the daemon. */
    reloads?: number;
    /** False while another screen shows. */
    active?: boolean;
  } = $props();

  /** The key slots of a page without a geometry: keys 1 to 9 of the
   * profile schema (keyvo_core::profile::KeySlot), three to a row. */
  const SLOTS = [
    [1, 2, 3],
    [4, 5, 6],
    [7, 8, 9],
  ];

  const uid = $props.id();

  function reason(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  /** Writes of this screen, so the list, the header and the tiles follow
   * them without waiting for the daemon's reload. */
  let edits = $state(0);

  let profiles: ProfileSummary[] = $state([]);
  let loading = $state(true);
  let listError: string | undefined = $state();
  let listed = 0;
  $effect(() => {
    void reloads;
    void edits;
    const mine = ++listed;
    files.list().then(
      (got) => {
        if (mine !== listed) return;
        profiles = ordered(got);
        listError = undefined;
        loading = false;
      },
      (error: unknown) => {
        if (mine !== listed) return;
        listError = reason(error);
        loading = false;
      },
    );
  });

  /** The profile the user picked in the list. */
  let chosen: string | undefined = $state();
  /** The selected profile: the picked one, else the active one, else the
   * global one, while the list has it. */
  const selected: ProfileSummary | undefined = $derived(
    [chosen, view?.profile, GLOBAL]
      .map((id) => profiles.find((profile) => profile.id === id))
      .find((profile) => profile !== undefined) ?? profiles[0],
  );
  const id = $derived(selected?.id);

  /** The selected page (from 1) and key; a new profile starts on page 1
   * with no key selected. */
  let page = $state(1);
  let key: number | undefined = $state();
  let shownId: string | undefined;
  $effect(() => {
    const now = id;
    if (now === shownId) return;
    shownId = now;
    page = 1;
    key = undefined;
  });

  function pick(profile: ProfileSummary) {
    chosen = profile.id;
  }

  // The header: the file and the fields as they are on disk.
  let header: ProfileView | undefined = $state.raw();
  let form: HeaderForm = $state({ name: "", accent: "" });
  let problem: Problem | undefined = $state();
  let invalid: string | undefined = $state();
  let note: string | undefined = $state();
  let read = 0;
  $effect(() => {
    const at = id;
    void reloads;
    if (at === undefined) return;
    const mine = ++read;
    files.profile(at).then(
      (got) => {
        if (mine !== read) return;
        header = got;
        form = toHeaderForm(got.fields);
        problem = undefined;
      },
      (error: unknown) => {
        if (mine !== read) return;
        header = undefined;
        problem = problemOf(error);
      },
    );
  });
  $effect(() => {
    void id;
    invalid = undefined;
    note = undefined;
  });

  /** Writes run one after the other, each against the last one's result. */
  let queue: Promise<void> = Promise.resolve();

  function commit() {
    queue = queue.then(async () => {
      const at = id;
      if (at === undefined || header === undefined) return;
      const result = fromHeaderForm(form);
      if ("invalid" in result) {
        invalid = result.invalid;
        return;
      }
      invalid = undefined;
      if (sameHeader(result.fields, header.fields)) return;
      try {
        const edited = await files.editProfile(at, result.fields);
        if (at === id) header = { path: edited.path, fields: edited.fields };
        problem = undefined;
        note = t("profiles.wrote", { id: at, path: edited.path });
        edits++;
      } catch (error) {
        problem = problemOf(error);
      }
    });
  }

  async function open() {
    const at = selected;
    if (at === undefined) return;
    try {
      await files.open(at.id);
      problem = undefined;
      note = t("profiles.opened", { path: at.path });
    } catch (error) {
      problem = problemOf(error);
    }
  }

  // The tabs.
  const pages: number[] = $derived(
    Array.from({ length: selected?.pages.length ?? 0 }, (_, index) => index + 1),
  );
  const tabs: HTMLButtonElement[] = $state([]);

  function tab(next: number) {
    page = next;
    key = undefined;
    tabs[next - 1]?.focus();
  }

  function tabKeydown(event: KeyboardEvent) {
    const last = pages.length;
    const next =
      event.key === "ArrowRight"
        ? (page % last) + 1
        : event.key === "ArrowLeft"
          ? ((page + last - 2) % last) + 1
          : event.key === "Home"
            ? 1
            : event.key === "End"
              ? last
              : undefined;
    if (next === undefined || last === 0) return;
    event.preventDefault();
    tab(next);
  }

  // The tiles of the selected page, again after every reload and edit.
  let tiles: Tile[] = $state([]);
  let tilesError: string | undefined = $state();
  let fetched = 0;
  $effect(() => {
    const at = id;
    const on = page;
    const shape = geometry;
    void reloads;
    void edits;
    if (at === undefined || shape === undefined || on > pages.length) {
      tiles = [];
      return;
    }
    const mine = ++fetched;
    daemon.tiles(at, on, shape.rects.length, shape.tile).then(
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

  // The key grid.
  const rows: number[][] = $derived(geometry === undefined ? SLOTS : grid(geometry));
  const slots = $derived(rows.flat().length);
  const keyButtons: HTMLButtonElement[] = $state([]);

  function choose(next: number) {
    key = next;
    keyButtons[next - 1]?.focus();
  }

  function gridKeydown(event: KeyboardEvent) {
    const from = key ?? 1;
    if (/^[1-9]$/.test(event.key) && Number(event.key) <= slots) {
      event.preventDefault();
      choose(Number(event.key));
    } else if (["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(event.key)) {
      event.preventDefault();
      choose(key === undefined ? from : moved(rows, from, event.key));
    }
  }

  function keyName(n: number): string {
    const label = tiles[n - 1]?.label;
    return label === undefined ? t("profiles.key", { key: n }) : t("profiles.keyLabel", { key: n, label });
  }

  const target: Target | undefined = $derived(
    id === undefined || key === undefined ? undefined : { profile: id, page, key },
  );

  function layerOf(n: number): string {
    if (geometry === undefined) return t("profiles.layerUnknown");
    const layer = tiles[n - 1]?.layer;
    return layer === undefined ? t("profiles.noLayer") : t("layer", { layer });
  }

  function itemName(profile: ProfileSummary): string {
    const values = { name: profile.name, id: profile.id, priority: profile.priority };
    return profile.id === view?.profile
      ? t("profiles.itemActive", values)
      : t("profiles.item", values);
  }
</script>

<Columns {columns}>
  {#snippet list()}
    <div class="head">
      <h2 id={`kv-profiles-${uid}`}>{t("profiles.list")}</h2>
      <Soon label={t("profiles.new")} text="+" soon="soon.newProfile" />
    </div>
    {#if loading && profiles.length === 0}
      <p class="note" aria-busy="true">{t("profiles.loading")}</p>
    {/if}
    {#if listError !== undefined}
      <p role="alert" class="problem">{t("profiles.listError", { error: listError })}</p>
    {/if}
    <ul class="profiles" aria-labelledby={`kv-profiles-${uid}`}>
      {#each profiles as profile (profile.id)}
        <li>
          <button
            type="button"
            class="profile"
            aria-label={itemName(profile)}
            aria-current={profile.id === id ? "true" : undefined}
            onclick={() => pick(profile)}
          >
            <span class="swatch" aria-hidden="true" style:background-color={profile.accent}></span>
            <span class="name">{profile.name}</span>
            <span class="meta">{t("profiles.meta", { id: profile.id, priority: profile.priority })}</span>
            {#if profile.id === view?.profile}
              <span class="mark" aria-hidden="true">{t("profiles.active")}</span>
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  {/snippet}

  {#snippet editor()}
    {#if selected === undefined}
      {#if !loading}
        <p class="note">{t("profiles.select")}</p>
      {/if}
    {:else}
      <section class="card" aria-labelledby={`kv-header-${uid}`}>
        <h2 id={`kv-header-${uid}`}>{t("profiles.header")}</h2>
        {#if problem !== undefined}
          <p role="alert" class="problem">{describe(problem)}</p>
        {/if}
        <dl class="facts">
          <div>
            <dt>{t("profiles.id")}</dt>
            <dd>{selected.id}</dd>
          </div>
          <div>
            <dt>{t("profiles.priority")}</dt>
            <dd>{selected.priority}</dd>
          </div>
        </dl>
        {#if header !== undefined}
          <form onsubmit={(event) => event.preventDefault()}>
            <label>
              {t("profiles.name")}
              <input
                type="text"
                aria-label={t("profiles.name")}
                bind:value={form.name}
                onchange={commit}
              />
            </label>
            <label>
              {t("profiles.accent")}
              <input
                type="text"
                aria-label={t("profiles.accent")}
                placeholder={t("editor.colorExample")}
                bind:value={form.accent}
                onchange={commit}
              />
            </label>
          </form>
          {#if invalid !== undefined}
            <p role="alert" class="problem">{invalid}</p>
          {/if}
        {/if}
        <p class="note">{t("profiles.file")} <code>{selected.path}</code></p>
        {#if !selected.file}
          <p class="note">{t("profiles.builtin")}</p>
        {/if}
        <div class="actions">
          <button
            type="button"
            aria-label={t("profiles.openFile")}
            disabled={!selected.file}
            onclick={() => void open()}
          >
            {t("profiles.openFile")}
          </button>
        </div>
        <p role="status" aria-label={t("profiles.result")} class="note">{note ?? ""}</p>
      </section>

      <section class="card" aria-labelledby={`kv-match-${uid}`}>
        <div class="head">
          <h2 id={`kv-match-${uid}`}>{t("profiles.match")}</h2>
          <Soon label={t("profiles.editMatch")} soon="soon.match" />
        </div>
        {#if selected.id === GLOBAL}
          <p class="note">{t("profiles.globalMatch")}</p>
        {:else if selected.match.class.length === 0 && selected.match.title === undefined}
          <p class="note">{t("profiles.noMatch")}</p>
        {:else}
          <dl class="facts">
            {#each selected.match.class as rule (rule)}
              <div>
                <dt>{t("profiles.class")}</dt>
                <dd><code>{rule}</code></dd>
              </div>
            {/each}
            {#if selected.match.title !== undefined}
              <div>
                <dt>{t("profiles.title")}</dt>
                <dd><code>{selected.match.title}</code></dd>
              </div>
            {/if}
          </dl>
        {/if}
      </section>

      <section class="card" aria-label={t("profiles.pages")}>
        {#if pages.length === 0}
          <p class="note">{t("profiles.noPages")}</p>
        {:else}
          <div role="tablist" aria-label={t("profiles.pages")} class="tabs" tabindex="-1" onkeydown={tabKeydown}>
            {#each pages as number (number)}
              <button
                bind:this={tabs[number - 1]}
                type="button"
                role="tab"
                id={`kv-tab-${uid}-${number}`}
                aria-selected={number === page}
                aria-controls={`kv-panel-${uid}`}
                tabindex={number === page ? 0 : -1}
                onclick={() => tab(number)}
              >
                {pageName(selected, number)}
              </button>
            {/each}
          </div>
          <div role="tabpanel" id={`kv-panel-${uid}`} aria-labelledby={`kv-tab-${uid}-${page}`}>
            <div
              role="grid"
              tabindex="-1"
              aria-label={t("profiles.keys", { page: pageName(selected, page) })}
              class="keys"
              onkeydown={gridKeydown}
            >
              {#each rows as row, index (index)}
                <div role="row" class="row">
                  {#each row as n (n)}
                    <div role="gridcell" aria-selected={n === key}>
                      <button
                        bind:this={keyButtons[n - 1]}
                        type="button"
                        class="slot"
                        tabindex={n === (key ?? 1) ? 0 : -1}
                        aria-label={keyName(n)}
                        onclick={() => (key = n)}
                      >
                        <span class="number">{n}</span>
                        <span class="label">{tiles[n - 1]?.label ?? ""}</span>
                      </button>
                    </div>
                  {/each}
                </div>
              {/each}
            </div>
            <div class="actions">
              <Soon label={t("profiles.move")} soon="soon.move" />
              <Soon label={t("profiles.behavior")} soon="soon.behaviors" />
            </div>
          </div>
        {/if}
      </section>

      {#if key !== undefined}
        <div class="card">
          <KeyEditor editor={keys} {target} {active} revision={reloads} onedited={() => edits++} />
        </div>
      {/if}
    {/if}
  {/snippet}

  {#snippet right()}
    {#if selected !== undefined && pages.length > 0}
      <section class="card" aria-labelledby={`kv-preview-${uid}`}>
        <h2 id={`kv-preview-${uid}`}>{t("profiles.preview")}</h2>
        {#if geometry === undefined}
          <p class="note">{t("home.noGeometry")}</p>
        {:else}
          <div
            role="grid"
            aria-label={t("profiles.mirror", { profile: selected.id, page })}
            class="mirror"
          >
            {#each grid(geometry) as row, index (index)}
              <div role="row" class="row">
                {#each row as n (n)}
                  <div role="gridcell" aria-selected={n === key}>
                    <button
                      type="button"
                      class="tile"
                      style:width={`${geometry.tile.width}px`}
                      style:height={`${geometry.tile.height}px`}
                      tabindex="-1"
                      aria-label={keyName(n)}
                      onclick={() => choose(n)}
                    >
                      {#if tiles[n - 1]}
                        <img src={tiles[n - 1]?.image} alt="" />
                      {:else}
                        <span aria-hidden="true">{n}</span>
                      {/if}
                    </button>
                  </div>
                {/each}
              </div>
            {/each}
          </div>
          {#if tilesError !== undefined}
            <p class="note">{t("home.tilesError", { error: tilesError })}</p>
          {/if}
        {/if}
      </section>
      <section class="card" aria-labelledby={`kv-card-${uid}`}>
        <h2 id={`kv-card-${uid}`}>{t("profiles.card")}</h2>
        {#if key === undefined}
          <p class="note">{t("profiles.selectKey")}</p>
        {:else}
          <dl class="facts" aria-labelledby={`kv-card-${uid}`}>
            <div>
              <dt>{t("profiles.position")}</dt>
              <dd>{t("profiles.key", { key })}</dd>
            </div>
            <div>
              <dt>{t("profiles.page")}</dt>
              <dd>{t("profiles.pageValue", { page, name: pageName(selected, page) })}</dd>
            </div>
            <div>
              <dt>{t("profiles.layer")}</dt>
              <dd>{layerOf(key)}</dd>
            </div>
          </dl>
        {/if}
      </section>
    {/if}
  {/snippet}
</Columns>

<style>
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  h2 {
    margin: 0;
    font-size: 1em;
  }

  .profiles {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 8px 0 0;
    padding: 0;
    list-style: none;
  }

  .profile {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 2px 8px;
    width: 100%;
    text-align: left;
  }

  .profile[aria-current="true"] {
    border-color: var(--kv-accent);
    background: var(--kv-bg);
    color: var(--kv-fg);
  }

  .swatch {
    grid-row: span 2;
    width: 6px;
    height: 100%;
    min-height: 24px;
    border-radius: var(--kv-radius);
  }

  .meta,
  .mark {
    background: var(--kv-bg-alt);
    color: var(--kv-fg-muted);
    font-size: 0.85em;
  }

  .mark {
    grid-column: 3;
    grid-row: 1;
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-bottom: 16px;
  }

  .facts {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 16px;
    margin: 0;
  }

  .facts dt {
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
    font-size: 0.85em;
  }

  .facts dd {
    margin: 0;
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
    font-size: 0.85em;
  }

  .actions,
  .tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  [role="tab"][aria-selected="true"] {
    border-color: var(--kv-accent);
    background: var(--kv-bg);
    color: var(--kv-fg);
  }

  [role="tabpanel"] {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 8px;
  }

  .keys,
  .mirror {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .row {
    display: flex;
    gap: 8px;
  }

  .slot {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    width: 96px;
    height: 56px;
    overflow: hidden;
    text-align: left;
  }

  [role="gridcell"][aria-selected="true"] .slot,
  [role="gridcell"][aria-selected="true"] .tile {
    border-color: var(--kv-accent);
  }

  .number {
    background: var(--kv-bg-alt);
    color: var(--kv-fg-muted);
    font-size: 0.85em;
  }

  .label {
    overflow: hidden;
    max-width: 100%;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tile {
    padding: 0;
    overflow: hidden;
  }

  .tile img {
    display: block;
    width: 100%;
    height: 100%;
  }

  .note {
    margin: 0;
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
    overflow-wrap: anywhere;
  }

  .problem {
    margin: 0;
    padding: 8px 12px;
    background: var(--kv-bg-alt);
    color: var(--kv-fg);
    border-left: 4px solid var(--kv-state-error);
    border-radius: var(--kv-radius);
    overflow-wrap: anywhere;
  }
</style>
