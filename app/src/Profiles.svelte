<script lang="ts">
  // Profiles, the profile manager (docs/design/gui-ux.md §3.2; pad screen
  // brief §5.3, §6; d1, d2, d14, ADR-0004): the list column with the
  // profiles by priority, the global one pinned first and the pad's one
  // marked, "+" with the templates; the editor with the heading row (name,
  // Open in editor, the file once), the header form (name and accent
  // written the moment they change, after a snapshot), the match rules,
  // the pages as thumbnails keyvo-core renders, and "Edit keys on the Pad".
  // The key grid lives on the Pad only. What is not built yet is shown
  // disabled (Soon.svelte). The selection is the shell's pin (#164).
  import { untrack } from "svelte";

  import Device from "./Device.svelte";
  import Notice from "./Notice.svelte";
  import Soon from "./Soon.svelte";
  import type { Daemon, Tile } from "./lib/daemon";
  import { describe, problemOf, type Problem } from "./lib/editor";
  import { t, type MessageKey } from "./lib/i18n";
  import { clock } from "./lib/pad";
  import { pinPage, pinProfile, type Pin, type Pinned } from "./lib/pin";
  import {
    fromHeaderForm,
    GLOBAL,
    ordered,
    pageName,
    sameHeader,
    toHeaderForm,
    type HeaderForm,
    type Invalid,
    type ProfileFiles,
    type ProfileSummary,
    type ProfileView,
  } from "./lib/profiles";
  import type { Geometry, View } from "./lib/protocol";
  import type { Columns } from "./lib/shell";

  let {
    daemon,
    files,
    columns,
    view,
    geometry,
    reloads = 0,
    pinned,
    onpin,
    onedit,
  }: {
    daemon: Daemon;
    files: ProfileFiles;
    columns: Columns;
    /** What the pad shows; its profile is the one "on the pad". */
    view?: View;
    /** The Keypad's geometry; the thumbnails need it. */
    geometry?: Geometry;
    /** Bumped by every configuration reload of the daemon. */
    reloads?: number;
    /** The shell's pin, the header's profile; undefined follows the pad. */
    pinned?: Pinned;
    /** Sets the pin, which the shell holds. */
    onpin?: (pinned: Pinned) => void;
    /** Opens the Pad pinned to a profile and page. */
    onedit?: (pin: Pin) => void;
  } = $props();

  /** The rows of the skeleton list while the first read runs (§6). */
  const SKELETON = [1, 2, 3, 4];

  /** The templates behind "+", each not built yet. */
  const TEMPLATES: { label: MessageKey; soon: MessageKey }[] = [
    { label: "profiles.templateEmpty", soon: "soon.templateEmpty" },
    { label: "profiles.templateCopy", soon: "soon.templateCopy" },
    { label: "profiles.templateStarter", soon: "soon.templateStarter" },
  ];

  const uid = $props.id();

  function reason(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  /** Writes of this screen, so the list, the header and the thumbnails
   * follow them without waiting for the daemon's reload. */
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

  let templating = $state(false);

  /** The selected profile: the pinned one, else the pad's one, else the
   * global one, while the list has it. The selection is the shell's pin,
   * the one the header names (#164): it follows the pad only while
   * nothing is pinned. */
  const selected: ProfileSummary | undefined = $derived(
    [pinned?.profile, view?.profile, GLOBAL]
      .map((id) => profiles.find((profile) => profile.id === id))
      .find((profile) => profile !== undefined) ?? profiles[0],
  );
  const id = $derived(selected?.id);

  /** A pick in the list pins that profile on page 1, as the header
   * dropdown does; the pinned profile keeps its pin. */
  function choose(profile: string) {
    if (pinned?.profile !== profile) onpin?.(pinProfile(profile));
  }

  /** A field taking the focus is the user at work on the selected profile
   * (critique P1 of #106): it pins it, on the pad's page when it is the
   * pad's profile, so the pad moving on changes nothing under them. */
  function hold() {
    if (id === undefined || pinned !== undefined) return;
    onpin?.(id === view?.profile ? pinPage(id, view.page) : pinProfile(id));
  }

  /** The selected page (from 1): a newly selected profile starts on its
   * pinned page, else on 1. */
  let page = $state(1);
  let shownId: string | undefined;
  $effect(() => {
    const now = id;
    if (now === shownId) return;
    shownId = now;
    page = untrack(() => (pinned !== undefined && pinned.profile === now ? pinned.page : 1));
  });

  // The header: the file and the fields as they are on disk.
  let header: ProfileView | undefined = $state.raw();
  let form: HeaderForm = $state({ name: "", accent: "" });
  let problem: Problem | undefined = $state();
  let invalid: Invalid | undefined = $state();
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
        invalid = result;
        return;
      }
      invalid = undefined;
      if (sameHeader(result.fields, header.fields)) return;
      try {
        const edited = await files.editProfile(at, result.fields);
        if (at === id) header = { path: edited.path, fields: edited.fields };
        problem = undefined;
        note = t("profiles.wrote", { id: at, path: edited.path, time: clock(Date.now()) });
        edits++;
      } catch (error) {
        problem = problemOf(error);
      }
    });
  }

  async function open() {
    const at = selected;
    if (at === undefined || !at.file) return;
    try {
      await files.open(at.id);
      problem = undefined;
      note = t("profiles.opened", { path: at.path });
    } catch (error) {
      problem = problemOf(error);
    }
  }

  /** The swatch beside the accent text: the typed colour once it is one,
   * else the one on disk. */
  const swatch = $derived.by(() => {
    const result = fromHeaderForm({ name: "-", accent: form.accent });
    return "fields" in result ? result.fields.accent : header?.fields.accent;
  });

  /** The match rules by kind, each kind named once. */
  const rules: { kind: MessageKey; patterns: string[] }[] = $derived(
    selected === undefined
      ? []
      : [
          { kind: "profiles.class" as const, patterns: selected.match.class },
          {
            kind: "profiles.title" as const,
            patterns: selected.match.title === undefined ? [] : [selected.match.title],
          },
        ].filter((rule) => rule.patterns.length > 0),
  );

  const pages: number[] = $derived(
    Array.from({ length: selected?.pages.length ?? 0 }, (_, index) => index + 1),
  );

  // The thumbnails: every page's tiles, again after every reload and edit.
  let thumbs: Tile[][] = $state([]);
  let thumbsError: string | undefined = $state();
  let fetched = 0;
  $effect(() => {
    const at = id;
    const count = pages.length;
    const shape = geometry;
    void reloads;
    void edits;
    const mine = ++fetched;
    thumbs = [];
    if (at === undefined || shape === undefined) return;
    Promise.all(
      Array.from({ length: count }, (_, index) =>
        daemon.tiles(at, index + 1, shape.rects.length, shape.tile),
      ),
    ).then(
      (got) => {
        if (mine !== fetched) return;
        thumbs = got;
        thumbsError = undefined;
      },
      (error: unknown) => {
        if (mine !== fetched) return;
        thumbsError = reason(error);
      },
    );
  });

  function itemName(profile: ProfileSummary): string {
    const values = { name: profile.name, id: profile.id, priority: profile.priority };
    return profile.id === view?.profile
      ? t("profiles.itemOnPad", values)
      : t("profiles.item", values);
  }

  const stacked = $derived(!columns.list);
</script>

<div class={["profiles-screen", { stacked, scroll: stacked }]}>
  <section class={["list", { scroll: !stacked }]} aria-labelledby={`kv-profiles-${uid}`}>
    <div class="head">
      <h2 id={`kv-profiles-${uid}`}>{t("profiles.list")}</h2>
      <button
        type="button"
        aria-label={t("profiles.new")}
        aria-expanded={templating}
        aria-controls={`kv-templates-${uid}`}
        onclick={() => (templating = !templating)}
      >
        {t("profiles.add")}
      </button>
    </div>
    {#if templating}
      <ul id={`kv-templates-${uid}`} class="templates" aria-label={t("profiles.templates")}>
        {#each TEMPLATES as template (template.label)}
          <li><Soon label={t(template.label)} soon={template.soon} /></li>
        {/each}
      </ul>
    {/if}
    {#if listError !== undefined}
      <Notice kind="error">{t("profiles.listError", { error: listError })}</Notice>
    {/if}
    {#if loading && profiles.length === 0}
      <!-- §6: skeleton rows, no spinner. -->
      <ul class="skeleton" aria-busy="true" aria-label={t("profiles.loading")}>
        {#each SKELETON as row (row)}
          <li><span class="bar"></span></li>
        {/each}
      </ul>
    {/if}
    <ul class="profiles" aria-labelledby={`kv-profiles-${uid}`}>
      {#each profiles as profile (profile.id)}
        <li>
          <button
            type="button"
            class="profile"
            aria-label={itemName(profile)}
            aria-current={profile.id === id ? "true" : undefined}
            onclick={() => choose(profile.id)}
          >
            <span class="name">{profile.name}</span>
            {#if profile.accent !== undefined}
              <span class="swatch" aria-hidden="true" style:background-color={profile.accent}></span>
            {/if}
            {#if profile.id === view?.profile}
              <span class="mark" aria-hidden="true">{t("profiles.onPad")}</span>
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  </section>

  <section
    class={["editor", { scroll: !stacked }]}
    aria-labelledby={`kv-heading-${uid}`}
    onfocusin={hold}
  >
    {#if selected === undefined}
      {#if !loading}
        <p class="note">{t("profiles.select")}</p>
      {/if}
    {:else}
      <div class="heading row">
        <h2 id={`kv-heading-${uid}`}>{selected.name}</h2>
        <button
          type="button"
          aria-label={t("profiles.openFile")}
          aria-disabled={selected.file ? undefined : "true"}
          onclick={() => void open()}
        >
          {t("profiles.openFile")}
        </button>
      </div>
      <p class="note path">{selected.path}</p>
      {#if !selected.file}
        <p class="note">{t("profiles.builtin")}</p>
      {/if}
      {#if problem !== undefined}
        <Notice kind="error">{describe(problem)}</Notice>
      {/if}

      {#if header !== undefined}
        <form class="fields" onsubmit={(event) => event.preventDefault()}>
          <div class="field">
            <label for={`kv-name-${uid}`}>{t("profiles.name")}</label>
            <input
              id={`kv-name-${uid}`}
              type="text"
              aria-label={t("profiles.name")}
              aria-invalid={invalid?.field === "name" ? "true" : undefined}
              aria-describedby={invalid?.field === "name" ? `kv-invalid-${uid}` : undefined}
              bind:value={form.name}
              onchange={commit}
            />
            {#if invalid?.field === "name"}
              <p id={`kv-invalid-${uid}`} class="invalid">{invalid.invalid}</p>
            {/if}
          </div>

          <div class="field">
            <label for={`kv-accent-${uid}`}>{t("profiles.accent")}</label>
            <span class="accent">
              <span class="swatch" aria-hidden="true" style:background-color={swatch}></span>
              <input
                id={`kv-accent-${uid}`}
                type="text"
                aria-label={t("profiles.accent")}
                aria-invalid={invalid?.field === "accent" ? "true" : undefined}
                aria-describedby={invalid?.field === "accent" ? `kv-invalid-${uid}` : undefined}
                placeholder={t("editor.colorExample")}
                bind:value={form.accent}
                onchange={commit}
              />
            </span>
            {#if invalid?.field === "accent"}
              <p id={`kv-invalid-${uid}`} class="invalid">{invalid.invalid}</p>
            {/if}
          </div>

          <!-- Read values until M4a-4 writes them: the value, then what it
               means or when it can change. -->
          <div class="field">
            <label for={`kv-priority-${uid}`}>{t("profiles.priority")}</label>
            <span class="value">
              <output id={`kv-priority-${uid}`} aria-live="off">{selected.priority}</output>
              <span class="hint">{t("profiles.priorityHint")}</span>
            </span>
          </div>

          <div class="field">
            <label for={`kv-enabled-${uid}`}>{t("profiles.enabled")}</label>
            <span class="value">
              <!-- Every profile is on until the schema can switch one off. -->
              <output id={`kv-enabled-${uid}`} aria-live="off">{t("profiles.on")}</output>
              <span class="hint">{t("soon.enabled")}</span>
            </span>
          </div>
        </form>
      {/if}
      <p role="status" aria-label={t("profiles.result")} class="note">{note ?? ""}</p>

      <section class="part" aria-labelledby={`kv-match-${uid}`}>
        <div class="head row">
          <h3 id={`kv-match-${uid}`}>{t("profiles.match")}</h3>
          {#if selected.id !== GLOBAL}
            <Soon label={t("profiles.addRule")} text={t("profiles.addRuleText")} soon="soon.match" />
          {/if}
        </div>
        {#if selected.id === GLOBAL}
          <p class="note">{t("profiles.globalMatch")}</p>
        {:else if rules.length === 0}
          <p class="note">{t("profiles.noMatch")}</p>
        {:else}
          <dl class="rules">
            {#each rules as rule (rule.kind)}
              <div class="kind">
                <dt>{t(rule.kind)}</dt>
                {#each rule.patterns as pattern (pattern)}
                  <dd>
                    <span class="pattern">{pattern}</span>
                    <Soon
                      label={t("profiles.editRule", { pattern })}
                      text={t("profiles.editRuleText")}
                      soon="soon.match"
                    />
                  </dd>
                {/each}
              </div>
            {/each}
          </dl>
        {/if}
      </section>

      <section class="part" aria-labelledby={`kv-pages-${uid}`}>
        <!-- The screen's one primary action heads its part, at the 48ch
             end, so a 760 px window shows it without scrolling (#166). -->
        <div class="head row">
          <h3 id={`kv-pages-${uid}`}>{t("profiles.pages")}</h3>
          {#if pages.length > 0}
            <button
              type="button"
              aria-label={t("profiles.editOnPad")}
              onclick={() => onedit?.({ profile: selected.id, page })}
            >
              {t("profiles.editOnPad")}
            </button>
          {/if}
        </div>
        {#if pages.length === 0}
          <p class="note">{t("profiles.noPages")}</p>
        {:else}
          {#if geometry === undefined}
            <p class="note">{t("profiles.thumbnailsWait")}</p>
          {/if}
          <ul class="thumbnails" aria-labelledby={`kv-pages-${uid}`}>
            {#each pages as number (number)}
              <li>
                <button
                  type="button"
                  class="thumbnail"
                  aria-label={pageName(selected, number)}
                  aria-pressed={number === page}
                  onclick={() => (page = number)}
                >
                  {#if geometry !== undefined}
                    <span class="thumb">
                      <Device {geometry}>
                        {#snippet tile(key, place)}
                          {@const image = thumbs[number - 1]?.[key - 1]?.image}
                          {#if image !== undefined}
                            <img src={image} alt="" style={place} />
                          {/if}
                        {/snippet}
                      </Device>
                    </span>
                  {/if}
                  <span class="caption">{pageName(selected, number)}</span>
                </button>
              </li>
            {/each}
          </ul>
          {#if thumbsError !== undefined}
            <p class="note">{t("home.tilesError", { error: thumbsError })}</p>
          {/if}
        {/if}
      </section>
    {/if}
  </section>
</div>

<style>
  .profiles-screen {
    display: grid;
    grid-template-columns: 240px minmax(0, 1fr);
    gap: 16px;
    height: 100%;
    min-height: 0;
  }

  .profiles-screen.stacked {
    display: flex;
    flex-direction: column;
  }

  /* DESIGN.md "Columns": every scrolling column reserves its gutter and
     takes the theme's thin scroll bar, so no control sits under it. */
  .scroll {
    min-height: 0;
    padding-inline-end: 8px;
    overflow: auto;
    scrollbar-gutter: stable;
    scrollbar-width: thin;
  }

  .list,
  .editor {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .head,
  .heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  h2,
  h3 {
    margin: 0;
    font-size: 1em;
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .templates,
  .profiles {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .profile {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    text-align: start;
  }

  .profile[aria-current="true"] {
    border-color: var(--kv-accent);
    background: var(--kv-bg);
    color: var(--kv-fg);
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .swatch {
    flex: none;
    box-sizing: border-box;
    width: 12px;
    height: 12px;
    border: 1px solid var(--kv-fg-muted);
    border-radius: 2px;
  }

  .mark {
    margin-inline-start: auto;
    background: var(--kv-bg-alt);
    color: var(--kv-fg-muted);
    font-size: 0.85em;
    white-space: nowrap;
  }

  .skeleton {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .skeleton .bar {
    display: block;
    height: 32px;
    border-radius: var(--kv-radius);
    background: var(--kv-bg-alt);
    color: var(--kv-fg-muted);
  }

  /* The heading rows end where the form ends, so their button sits at the
     end of the content, not at the column's edge. */
  .row {
    max-width: 48ch;
  }

  /* A name without spaces breaks anywhere rather than pushing the button
     out of the row, and the button keeps its one line beside it. */
  .heading h2 {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .heading > button {
    flex: none;
    white-space: nowrap;
  }

  /* "Edit keys on the Pad" keeps its one line beside the Pages heading. */
  .head > button {
    flex: none;
    white-space: nowrap;
  }

  /* DESIGN.md "Inputs / Fields", as the Inspector: each label above its
     field, muted and smaller. */
  .fields {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: 48ch;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .field > label {
    background: var(--kv-bg);
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

  .invalid {
    margin: 0;
    background: var(--kv-bg);
    color: var(--kv-fg);
  }

  .hint {
    margin-inline-start: 8px;
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
  }

  .part {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 8px;
  }

  .rules {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: 48ch;
    margin: 0;
  }

  .rules dt {
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
  }

  .rules dd {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin: 4px 0 0;
  }

  .pattern {
    overflow-wrap: anywhere;
  }

  .thumbnails {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .thumbnail {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    height: auto;
    padding: 4px;
  }

  .thumbnail[aria-pressed="true"] {
    border-color: var(--kv-accent);
    background: var(--kv-bg);
    color: var(--kv-fg);
  }

  .thumb {
    display: block;
    width: 112px;
  }

  .thumb img {
    display: block;
    border-radius: var(--kv-tile-radius, var(--kv-radius));
    object-fit: cover;
  }

  .note {
    margin: 0;
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
    overflow-wrap: anywhere;
  }
</style>
