<script lang="ts">
  // PROTOTYPE (#107) Variant C, "Three panes": the hierarchy profile, page,
  // key reads left to right, one pane each. The first pane lists the
  // profiles with their match rules and accent; the second shows the
  // pages of the chosen profile as a strip of thumbnails and the chosen
  // page as the big panel, with the profile settings folded under it; the
  // third holds the key: its facts and the editor, or, with no key chosen,
  // what the pane will show. Every pane scales its panels to its width.
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

  function rules(profile: { id: string; match: { class: string[]; title?: string } }): string {
    if (profile.id === GLOBAL) return "every window";
    const parts = [...profile.match.class];
    if (profile.match.title !== undefined) parts.push(`title ${profile.match.title}`);
    return parts.length === 0 ? "never selected by focus" : parts.join(", ");
  }
</script>

<div class="panes">
  <section class="pane profiles" aria-label={t("profiles.list")}>
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
            <span class="swatch" aria-hidden="true" style:background-color={profile.accent}></span>
            <span class="name">{profile.name}</span>
            {#if profile.id === model.view?.profile}
              <span class="live">on the pad</span>
            {/if}
            <span class="rules">{rules(profile)}</span>
            <span class="muted">priority {profile.priority}</span>
          </button>
        </li>
      {/each}
    </ul>
  </section>

  <section class="pane page" aria-label={t("profiles.pages")}>
    {#if selected === undefined}
      {#if !model.loading}
        <p class="note">{t("profiles.select")}</p>
      {/if}
    {:else}
      <h2>{selected.name}</h2>
      {#if model.pages.length === 0}
        <p class="note">{t("profiles.noPages")}</p>
      {:else}
        <div class="strip" role="tablist" aria-label={t("profiles.pages")}>
          {#each model.pages as number (number)}
            <button
              type="button"
              role="tab"
              class="thumb"
              aria-selected={number === model.page}
              onclick={() => model.tab(number)}
            >
              {#if model.geometry !== undefined}
                <PadPanel
                  geometry={model.geometry}
                  tiles={perPage[number - 1] ?? []}
                  label={pageName(selected, number)}
                />
              {/if}
              <span>{pageName(selected, number)}</span>
            </button>
          {/each}
        </div>
        <div class="big">
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
        </div>
      {/if}

      <details class="settings">
        <summary>Profile settings</summary>
        {#if model.problem !== undefined}
          <p role="alert" class="problem">{describe(model.problem)}</p>
        {/if}
        {#if model.header !== undefined}
          <form onsubmit={(event) => event.preventDefault()}>
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
          {t("profiles.match")}: {rules(selected)}
        </p>
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
  </section>

  <section class="pane key" aria-label={t("profiles.card")}>
    {#if selected !== undefined && model.key !== undefined}
      <p class="muted">
        {t("profiles.pageValue", { page: model.page, name: pageName(selected, model.page) })},
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
    {:else}
      <h2>{t("editor.title")}</h2>
      <p class="note">{t("profiles.selectKey")}</p>
      <p class="note">The key's label, colour, icon and action are edited here; every change is written to the profile file at once and the pad repaints.</p>
    {/if}
  </section>
</div>

<style>
  .panes {
    display: grid;
    grid-template-columns: minmax(220px, 1fr) minmax(320px, 2fr) minmax(280px, 1.2fr);
    gap: 0;
    height: 100%;
    min-height: 0;
  }

  .pane {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
    padding: 0 16px;
    overflow: auto;
    scrollbar-gutter: stable;
  }

  .pane + .pane {
    border-left: 1px solid var(--kv-bg-alt);
  }

  .profiles {
    padding-left: 0;
  }

  .key {
    padding-right: 0;
  }

  h2 {
    margin: 0;
    font-size: 1em;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .row {
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 2px 8px;
    width: 100%;
    margin-bottom: 4px;
    padding: 6px 8px;
    text-align: left;
  }

  .row[aria-current="true"] {
    border-color: var(--kv-accent);
    background: var(--kv-bg);
  }

  .rules,
  .row .muted {
    grid-column: 2 / -1;
    color: var(--kv-fg-muted);
    font-size: 0.85em;
    overflow-wrap: anywhere;
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

  .strip {
    display: flex;
    gap: 8px;
    overflow-x: auto;
  }

  .thumb {
    display: flex;
    flex: none;
    flex-direction: column;
    gap: 4px;
    width: 96px;
    min-height: 0;
    padding: 4px;
  }

  .thumb[aria-selected="true"] {
    border-color: var(--kv-accent);
    background: var(--kv-bg);
  }

  .big {
    max-width: 480px;
  }

  details {
    padding: 8px 12px;
    border: 1px solid var(--kv-bg-alt);
    border-radius: var(--kv-radius);
  }

  summary {
    cursor: pointer;
  }

  details[open] summary {
    margin-bottom: 8px;
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: 32ch;
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
    min-width: 0;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 8px;
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
