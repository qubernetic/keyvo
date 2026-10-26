<script lang="ts">
  // PROTOTYPE (#107) Variant B, "Document": the list column stays; the
  // profile is one scrolling document of at most 72ch: the header form,
  // the match rules, then every page as its own section with a small panel
  // and the slot list beside it. Selecting a slot opens the key editor
  // inline under that page, where the click was. The right column never
  // follows the selection: it is "On the pad", the live page the Keypad
  // shows, with a button to jump the document to it. The mirror follows
  // the pad, the editor follows the user (the P1 of the critique).
  import KeyEditor from "../KeyEditor.svelte";
  import Soon from "../Soon.svelte";
  import type { Tile } from "../lib/daemon";
  import { describe } from "../lib/editor";
  import { t } from "../lib/i18n";
  import { GLOBAL, pageName } from "../lib/profiles";
  import PadPanel from "./PadPanel.svelte";
  import { pageTiles, type ProfilesModel } from "./model";

  let { model }: { model: ProfilesModel } = $props();
  const selected = $derived(model.selected);

  /** The tiles of every page of the selected profile. */
  let perPage: Tile[][] = $state([]);
  $effect(() => {
    const id = selected?.id;
    const count = model.pages.length;
    const geometry = model.geometry;
    void model.reloads;
    void model.tiles;
    if (id === undefined || geometry === undefined) {
      perPage = [];
      return;
    }
    void pageTiles(model.daemon, id, count, geometry).then((got) => {
      if (id === model.selected?.id) perPage = got;
    });
  });

  /** The tiles of the page the pad shows. */
  let live: Tile[] = $state([]);
  $effect(() => {
    const view = model.view;
    const geometry = model.geometry;
    void model.reloads;
    if (view === undefined || geometry === undefined) {
      live = [];
      return;
    }
    void model.daemon
      .tiles(view.profile, view.page, geometry.rects.length, geometry.tile)
      .then((got) => (live = got))
      .catch(() => (live = []));
  });

  const liveProfile = $derived(model.profiles.find((profile) => profile.id === model.view?.profile));

  function pickKey(page: number, key: number) {
    if (page !== model.page) model.tab(page);
    model.choose(key);
  }

  function jump() {
    const view = model.view;
    const profile = liveProfile;
    if (view === undefined || profile === undefined) return;
    model.pick(profile);
    queueMicrotask(() => model.tab(view.page));
  }
</script>

<div class="columns">
  <section class="list" aria-label={t("columns.list")}>
    <div class="head">
      <h2>{t("profiles.list")}</h2>
      <Soon label={t("profiles.new")} text="+" soon="soon.newProfile" />
    </div>
    {#if model.listError !== undefined}
      <p role="alert" class="problem">{t("profiles.listError", { error: model.listError })}</p>
    {/if}
    <ul>
      {#each model.profiles as profile (profile.id)}
        <li>
          <button
            type="button"
            class="row"
            aria-current={profile.id === selected?.id ? "true" : undefined}
            aria-label={model.itemName(profile)}
            onclick={() => model.pick(profile)}
          >
            <span class="name">{profile.name}</span>
            <span class="swatch" aria-hidden="true" style:background-color={profile.accent}></span>
            <span class="muted">{profile.id}, priority {profile.priority}</span>
          </button>
        </li>
      {/each}
    </ul>
  </section>

  <section class="document" aria-label={t("columns.editor")}>
    {#if selected === undefined}
      {#if !model.loading}
        <p class="note">{t("profiles.select")}</p>
      {/if}
    {:else}
      <h1>{selected.name}</h1>
      <p class="muted">
        {selected.id}, priority {selected.priority}, {selected.path}
        {#if !selected.file}
          ({t("profiles.builtin")})
        {/if}
      </p>
      {#if model.problem !== undefined}
        <p role="alert" class="problem">{describe(model.problem)}</p>
      {/if}
      {#if model.header !== undefined}
        <form class="header" onsubmit={(event) => event.preventDefault()}>
          <label>
            {t("profiles.name")}
            <input type="text" bind:value={model.form.name} onchange={model.commit} />
          </label>
          <label>
            {t("profiles.accent")}
            <span class="accent">
              <span class="swatch big" aria-hidden="true" style:background-color={model.form.accent}></span>
              <input
                type="text"
                placeholder={t("editor.colorExample")}
                bind:value={model.form.accent}
                onchange={model.commit}
              />
            </span>
          </label>
          <span class="buttons">
            <button type="button" disabled={!selected.file} onclick={model.open}>
              {t("profiles.openFile")}
            </button>
          </span>
        </form>
        {#if model.invalid !== undefined}
          <p role="alert" class="problem">{model.invalid}</p>
        {/if}
      {/if}
      <p role="status" class="note">{model.note ?? ""}</p>

      <h2>{t("profiles.match")}</h2>
      {#if selected.id === GLOBAL}
        <p class="note">{t("profiles.globalMatch")}</p>
      {:else if selected.match.class.length === 0 && selected.match.title === undefined}
        <p class="note">{t("profiles.noMatch")}</p>
      {:else}
        <ul class="rules">
          {#each selected.match.class as rule (rule)}
            <li>{t("profiles.class")}: {rule}</li>
          {/each}
          {#if selected.match.title !== undefined}
            <li>{t("profiles.title")}: {selected.match.title}</li>
          {/if}
        </ul>
      {/if}
      <Soon label={t("profiles.editMatch")} soon="soon.match" />

      {#each model.pages as number (number)}
        {@const tiles = perPage[number - 1] ?? []}
        <section class="page" class:current={number === model.page}>
          <h2>{t("profiles.pageValue", { page: number, name: pageName(selected, number) })}</h2>
          <div class="spread">
            <div class="thumb">
              {#if model.geometry !== undefined}
                <PadPanel
                  geometry={model.geometry}
                  {tiles}
                  selected={number === model.page ? model.key : undefined}
                  label={t("profiles.mirror", { profile: selected.id, page: number })}
                  onchoose={(key) => pickKey(number, key)}
                />
              {/if}
            </div>
            <ol class="slots">
              {#each tiles as tile, index (index)}
                <li>
                  <button
                    type="button"
                    class="slot"
                    aria-pressed={number === model.page && index + 1 === model.key}
                    onclick={() => pickKey(number, index + 1)}
                  >
                    <span class="muted">{index + 1}</span>
                    <span class="label">{tile.label ?? ""}</span>
                    <span class="muted">{tile.layer === undefined ? "" : t("layer", { layer: tile.layer })}</span>
                  </button>
                </li>
              {/each}
            </ol>
          </div>
          {#if number === model.page && model.key !== undefined}
            <div class="inline">
              <KeyEditor
                editor={model.editor}
                target={model.target}
                active={model.active}
                revision={model.reloads}
                onedited={model.edited}
              />
              <div class="actions">
                <Soon label={t("profiles.move")} soon="soon.move" />
                <Soon label={t("profiles.behavior")} soon="soon.behaviors" />
              </div>
            </div>
          {/if}
        </section>
      {/each}
      {#if model.pages.length === 0}
        <p class="note">{t("profiles.noPages")}</p>
      {/if}
    {/if}
  </section>

  <aside class="pad" aria-label="On the pad">
    <h2>On the pad</h2>
    {#if model.view === undefined || model.geometry === undefined}
      <p class="note">{t("home.noGeometry")}</p>
    {:else}
      <PadPanel
        geometry={model.geometry}
        tiles={live}
        label={`On the pad: ${model.view.profile}, page ${model.view.page}`}
      />
      <p class="note">
        {liveProfile?.name ?? model.view.profile}, page {model.view.page}, {model.view.mode}
      </p>
      {#if model.view.profile !== selected?.id || model.view.page !== model.page}
        <button type="button" onclick={jump}>Edit this page</button>
      {:else}
        <p class="muted">This is the page you are editing.</p>
      {/if}
    {/if}
  </aside>
</div>

<style>
  .columns {
    display: grid;
    grid-template-columns: 220px minmax(0, 72ch) 260px;
    gap: 24px;
    align-items: start;
    height: 100%;
    min-height: 0;
  }

  .list,
  .document {
    min-height: 0;
    max-height: 100%;
    overflow: auto;
    scrollbar-gutter: stable;
  }

  .pad {
    position: sticky;
    top: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  h1 {
    margin: 0;
    font-size: 1.2em;
  }

  h2 {
    margin: 16px 0 8px;
    font-size: 1em;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .head h2 {
    margin: 0;
  }

  ul,
  ol {
    margin: 8px 0 0;
    padding: 0;
    list-style: none;
  }

  .row {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 0 8px;
    width: 100%;
    margin-bottom: 4px;
    text-align: left;
  }

  .row[aria-current="true"] {
    border-color: var(--kv-accent);
    background: var(--kv-bg);
  }

  .row .muted {
    grid-column: 1 / -1;
  }

  .swatch {
    display: inline-block;
    width: 12px;
    height: 12px;
    border-radius: 3px;
  }

  .swatch.big {
    width: 20px;
    height: 20px;
  }

  .muted {
    color: var(--kv-fg-muted);
    font-size: 0.85em;
  }

  .header {
    display: grid;
    grid-template-columns: minmax(120px, 24ch) minmax(120px, 20ch) auto;
    gap: 16px;
    align-items: end;
    margin-top: 8px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    color: var(--kv-fg-muted);
    font-size: 0.85em;
  }

  .accent {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .accent input {
    flex: 1;
    min-width: 0;
  }

  .rules {
    margin: 0 0 8px;
    padding-left: 1.2em;
    list-style: disc;
  }

  .page {
    margin-top: 16px;
    padding-top: 8px;
    border-top: 1px solid var(--kv-bg-alt);
  }

  .page.current h2 {
    color: var(--kv-fg);
  }

  .spread {
    display: grid;
    grid-template-columns: 200px 1fr;
    gap: 16px;
    align-items: start;
  }

  .slots {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 4px;
    margin: 0;
  }

  .slot {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0 6px;
    width: 100%;
    padding: 4px 8px;
    text-align: left;
  }

  .slot .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .slot > .muted:last-child {
    grid-column: 2;
  }

  .slot[aria-pressed="true"] {
    border-color: var(--kv-accent);
    background: var(--kv-bg);
  }

  .inline {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: 48ch;
    margin-top: 12px;
    padding: 12px;
    border: 1px solid var(--kv-fg-muted);
    border-radius: var(--kv-radius);
  }

  .actions {
    display: flex;
    gap: 8px;
  }

  .note {
    margin: 0;
    color: var(--kv-fg-muted);
    overflow-wrap: anywhere;
  }

  .problem {
    margin: 8px 0;
    padding: 8px 12px;
    border: 1px solid var(--kv-state-error);
    border-radius: var(--kv-radius);
    background: var(--kv-bg-alt);
    color: var(--kv-fg);
    overflow-wrap: anywhere;
  }
</style>
