<script lang="ts">
  // PROTOTYPE (#107) Variant A, "Instrument": no list column. The profiles
  // are a row of segmented buttons at the top, the pages a row of tabs
  // under them, and the panel (the preview, scaled, at most 1:1) sits
  // beside the key editor: the gauge and the control in one line of sight.
  // The slot grid is gone, the panel's tiles are the grid. The profile
  // header and the match rules fold into one details block under the
  // panel, so the key work is the screen and the profile work is a drawer.
  import KeyEditor from "../KeyEditor.svelte";
  import Soon from "../Soon.svelte";
  import { describe } from "../lib/editor";
  import { t } from "../lib/i18n";
  import { GLOBAL, pageName } from "../lib/profiles";
  import PadPanel from "./PadPanel.svelte";
  import type { ProfilesModel } from "./model";

  let { model }: { model: ProfilesModel } = $props();
  const selected = $derived(model.selected);
</script>

<div class="screen">
  <div class="profiles" role="tablist" aria-label={t("profiles.list")}>
    {#each model.profiles as profile (profile.id)}
      <button
        type="button"
        role="tab"
        aria-selected={profile.id === selected?.id}
        aria-label={model.itemName(profile)}
        class="chip"
        onclick={() => model.pick(profile)}
      >
        <span class="swatch" aria-hidden="true" style:background-color={profile.accent}></span>
        {profile.name}
        {#if profile.id === model.view?.profile}
          <span class="live">on the pad</span>
        {/if}
      </button>
    {/each}
    <Soon label={t("profiles.new")} text="+" soon="soon.newProfile" />
  </div>

  {#if selected !== undefined}
    <div class="pages" role="tablist" aria-label={t("profiles.pages")}>
      {#each model.pages as number (number)}
        <button
          type="button"
          role="tab"
          aria-selected={number === model.page}
          onclick={() => model.tab(number)}
        >
          {pageName(selected, number)}
        </button>
      {/each}
      {#if model.pages.length === 0}
        <p class="note">{t("profiles.noPages")}</p>
      {/if}
    </div>

    <div class="bench">
      <div class="panel">
        {#if model.geometry === undefined}
          <p class="note">{t("home.noGeometry")}</p>
        {:else}
          <PadPanel
            geometry={model.geometry}
            tiles={model.tiles}
            selected={model.key}
            label={t("profiles.mirror", { profile: selected.id, page: model.page })}
            keyName={model.keyName}
            onchoose={model.choose}
          />
        {/if}
        <p class="note">Click a tile to edit its key. Arrow keys move between the tiles.</p>
      </div>

      <div class="editor">
        {#if model.key === undefined}
          <h2>{t("editor.title")}</h2>
          <p class="note">{t("profiles.selectKey")}</p>
        {:else}
          <p class="facts">
            {t("profiles.pageValue", { page: model.page, name: pageName(selected, model.page) })}
            <span class="dot" aria-hidden="true"></span>
            {model.layerOf(model.key)}
          </p>
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
        {/if}
      </div>
    </div>

    <details class="profile">
      <summary>
        <strong>{selected.name}</strong>
        <span class="muted">{selected.id}, priority {selected.priority}</span>
        <span class="muted">
          {#if selected.id === GLOBAL}
            under every window
          {:else if selected.match.class.length === 0 && selected.match.title === undefined}
            no match rules
          {:else}
            matches {selected.match.class.join(", ")}{selected.match.title !== undefined
              ? `, title ${selected.match.title}`
              : ""}
          {/if}
        </span>
      </summary>
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
        </form>
        {#if model.invalid !== undefined}
          <p role="alert" class="problem">{model.invalid}</p>
        {/if}
      {/if}
      <p class="note">
        {selected.path}
        {#if !selected.file}
          ({t("profiles.builtin")})
        {/if}
      </p>
      <div class="actions">
        <button type="button" disabled={!selected.file} onclick={model.open}>
          {t("profiles.openFile")}
        </button>
        <Soon label={t("profiles.editMatch")} soon="soon.match" />
      </div>
      <p role="status" class="note">{model.note ?? ""}</p>
    </details>
  {/if}
</div>

<style>
  .screen {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 1100px;
  }

  h2 {
    margin: 0;
    font-size: 1em;
  }

  .profiles,
  .pages,
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }

  [role="tab"][aria-selected="true"] {
    border-color: var(--kv-accent);
    background: var(--kv-bg);
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

  .live,
  .muted {
    color: var(--kv-fg-muted);
    font-size: 0.85em;
  }

  .bench {
    display: grid;
    grid-template-columns: minmax(240px, 480px) minmax(280px, 48ch);
    gap: 24px;
    align-items: start;
  }

  .panel {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .editor {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .facts {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    color: var(--kv-fg-muted);
  }

  .dot {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: var(--kv-fg-muted);
  }

  details {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px 12px;
    border: 1px solid var(--kv-bg-alt);
    border-radius: var(--kv-radius);
  }

  summary {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    cursor: pointer;
  }

  details[open] summary {
    margin-bottom: 12px;
  }

  .header {
    display: grid;
    grid-template-columns: repeat(2, minmax(160px, 24ch));
    gap: 16px;
    margin-bottom: 8px;
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
  }

  .note {
    margin: 0;
    color: var(--kv-fg-muted);
    overflow-wrap: anywhere;
  }

  .problem {
    margin: 0;
    padding: 8px 12px;
    border: 1px solid var(--kv-state-error);
    border-radius: var(--kv-radius);
    background: var(--kv-bg-alt);
    color: var(--kv-fg);
    overflow-wrap: anywhere;
  }
</style>
