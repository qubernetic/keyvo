<script lang="ts">
  // The actions catalogue column of the Pad (pad screen brief §5.2, §6,
  // decisions Q3 and Q8): the search field (`/` focuses it), then the
  // groups General, System and Apps, each collapsible on its heading. An
  // entry, a preset of the daemon's recipe packs or a raw tool (Chord,
  // Exec), is dragged onto a tile of the mirror or clicked to assign it to
  // the selected tile. The search covers group, name and variant names.
  // Under the entries: why the presets could not be read, and each user
  // pack the daemon left out.
  import { dragData, DRAG_TYPE, GROUPS, search, type CatalogueEntry, type GroupId } from "./lib/catalogue";
  import { t } from "./lib/i18n";
  import { tileIcon } from "./lib/tileIcons";

  let {
    entries,
    selected,
    active = true,
    connected = true,
    problems = [],
    unread,
    onassign,
  }: {
    entries: readonly CatalogueEntry[];
    /** The selected tile a click assigns to; none until the user selects one. */
    selected?: number;
    /** False while another screen shows: `/` is not ours. */
    active?: boolean;
    /** False while the daemon is unreachable or not yet reached: nothing
     * to assign to, and the hint says so. */
    connected?: boolean;
    /** One `file:line:column: message` per user pack the daemon left out. */
    problems?: readonly string[];
    /** Why the recipe packs could not be read. */
    unread?: string;
    /** Assigns the entry with key `entry` to the selected tile. */
    onassign: (entry: string) => void;
  } = $props();

  const uid = $props.id();
  let text = $state("");
  let field: HTMLInputElement | undefined = $state();
  let collapsed: GroupId[] = $state([]);

  const found = $derived(search(entries, text));
  const searching = $derived(text.trim() !== "");
  const hint = $derived(
    !connected
      ? t("catalogue.noPad")
      : selected === undefined
        ? t("catalogue.selectFirst")
        : t("catalogue.assign", { key: selected }),
  );
  const ids = { heading: `kv-catalogue-${uid}`, hint: `kv-catalogue-${uid}-hint` };

  function toggle(group: GroupId) {
    collapsed = collapsed.includes(group)
      ? collapsed.filter((id) => id !== group)
      : [...collapsed, group];
  }

  function dragstart(event: DragEvent, entry: CatalogueEntry) {
    if (event.dataTransfer === null) return;
    event.dataTransfer.setData(DRAG_TYPE, dragData(entry));
    event.dataTransfer.effectAllowed = "copy";
  }

  function assign(entry: CatalogueEntry) {
    if (selected !== undefined) onassign(entry.key);
  }

  /** `/` to the search, unless a field has the focus: there it is typed. */
  function keydown(event: KeyboardEvent) {
    if (!active || event.key !== "/" || event.ctrlKey || event.altKey || event.metaKey) return;
    const focus = event.target;
    if (
      focus instanceof HTMLInputElement ||
      focus instanceof HTMLSelectElement ||
      focus instanceof HTMLTextAreaElement
    ) {
      return;
    }
    event.preventDefault();
    field?.focus();
  }
</script>

<svelte:window onkeydown={keydown} />

<section class="catalogue" aria-labelledby={ids.heading}>
  <h2 id={ids.heading}>{t("catalogue.title")}</h2>
  <input
    bind:this={field}
    type="search"
    aria-label={t("catalogue.search")}
    aria-keyshortcuts="/"
    placeholder={t("catalogue.search")}
    bind:value={text}
  />
  <p id={ids.hint} class="hint">{hint}</p>
  {#if found.length === 0}
    <p class="note">{t("catalogue.noMatch")}</p>
  {/if}
  {#each GROUPS as group (group.id)}
    {@const shown = found.filter((entry) => entry.group === group.id)}
    {@const open = searching || !collapsed.includes(group.id)}
    {#if shown.length > 0}
      <h3>
        <button
          type="button"
          class="group"
          aria-label={t(group.label)}
          aria-expanded={open}
          onclick={() => toggle(group.id)}
        >
          <span class="chevron" class:open aria-hidden="true"></span>
          {t(group.label)}
        </button>
      </h3>
      {#if open}
        <ul aria-label={t(group.label)}>
          {#each shown as entry (entry.key)}
            {@const svg = tileIcon(entry.icon)}
            <li>
              <span class="icon" aria-hidden="true">
                {#if svg !== undefined}{@html svg}{/if}
              </span>
              <button
                type="button"
                class="entry"
                draggable="true"
                aria-label={entry.name}
                aria-describedby={ids.hint}
                aria-disabled={selected === undefined ? "true" : undefined}
                ondragstart={(event) => dragstart(event, entry)}
                onclick={() => assign(entry)}
              >
                {entry.name}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    {/if}
  {/each}
  {#if unread !== undefined}
    <p class="note">{t("catalogue.unread", { error: unread })}</p>
  {/if}
  {#if problems.length > 0}
    <ul class="problems" aria-label={t("catalogue.problems")}>
      {#each problems as problem, index (index)}
        <li class="note">{problem}</li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .catalogue {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 240px;
  }

  h2,
  h3 {
    margin: 0;
    font-size: 1em;
  }

  .group {
    display: flex;
    gap: 8px;
    align-items: center;
    width: 100%;
    border-color: transparent;
    background: var(--kv-bg);
    color: var(--kv-fg);
    font-weight: 600;
    text-align: left;
  }

  /* A filled triangle in the body colour: right while collapsed, down open. */
  .chevron {
    flex: none;
    width: 8px;
    height: 8px;
    background: var(--kv-fg);
    color: var(--kv-bg);
    clip-path: polygon(0 0, 100% 50%, 0 100%);
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  ul {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .icon {
    display: inline-flex;
    flex: none;
    width: 20px;
    height: 20px;
  }

  .icon :global(svg) {
    width: 100%;
    height: 100%;
  }

  .entry {
    cursor: grab;
  }

  .problems .note {
    overflow-wrap: anywhere;
  }

  .hint,
  .note {
    margin: 0;
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
    font-size: 0.85em;
  }
</style>
