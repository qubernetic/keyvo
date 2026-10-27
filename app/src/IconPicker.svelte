<script lang="ts">
  // The icon picker of the inspector (#108 component breakdown): a
  // combobox over the tile icons keyvo-core bundles (lib/tileIcons.ts),
  // each row with its glyph and name. Typing filters; the first SHOWN
  // matches are rows and a line counts the rest (the whole lucide set
  // inlined would slow the list down). The arrows move over the rows,
  // Enter or a click picks, Escape closes; the pick is written as
  // `lucide:<name>`, and an emptied field removes the icon. A value the
  // picker does not bundle stays as written until the user picks.
  import { t } from "./lib/i18n";
  import { TILE_ICONS, tileIcon, tileIconValue } from "./lib/tileIcons";

  let {
    label,
    value,
    onpick,
  }: {
    /** The field's name, shown by the caller and announced. */
    label: string;
    /** The key's `icon` field; empty for none. */
    value: string;
    /** Writes the new `icon` field, empty to remove it. */
    onpick: (value: string) => void;
  } = $props();

  const uid = $props.id();

  /** What the field shows for `value`: the name of a bundled icon, else
   * the value as written. */
  function shownOf(value: string): string {
    return tileIconValue.read(value) ?? value;
  }

  let query = $state("");
  let open = $state(false);
  /** The index of the active row in `matches`; -1 for none, as after
   * typing, until an arrow moves into the list. */
  let active = $state(-1);

  /** The name the field shows for the value. A derived only notifies when
   * the name changes, so a refresh that hands the same value again (a new
   * revision of the key, an acknowledged write) keeps what the user typed. */
  const shown = $derived(shownOf(value));

  // The field follows the value from outside: a pick, a read, an undo.
  $effect(() => {
    query = shown;
  });

  /** How many matches the list shows as rows. */
  const SHOWN = 60;

  const matches = $derived.by(() => {
    const text = query.trim().toLowerCase();
    // The list opened on the shown name lists everything.
    if (text === "" || text === shown.toLowerCase()) return TILE_ICONS;
    return TILE_ICONS.filter((name) => name.includes(text));
  });
  const rows = $derived(matches.slice(0, SHOWN));
  const activeName = $derived(open ? rows[active] : undefined);

  function optionId(name: string): string {
    return `kv-icon-${uid}-${name}`;
  }

  function pick(name: string) {
    open = false;
    query = name;
    onpick(tileIconValue.write(name));
  }

  function close() {
    open = false;
    query = shown;
  }

  function move(step: number) {
    const count = rows.length;
    if (!open || active < 0) {
      open = true;
      active = step > 0 ? 0 : count - 1;
      return;
    }
    if (count > 0) active = (active + step + count) % count;
  }

  function keydown(event: KeyboardEvent) {
    switch (event.key) {
      case "ArrowDown":
        move(1);
        break;
      case "ArrowUp":
        move(-1);
        break;
      case "Enter":
        if (activeName === undefined) return;
        pick(activeName);
        break;
      case "Escape":
        if (!open) return;
        close();
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  function input() {
    open = true;
    active = -1;
  }

  /** A field left empty removes the icon; other text waits for a pick. */
  function change() {
    if (query.trim() === "" && value !== "") onpick("");
  }
</script>

<div class="picker">
  <input
    type="text"
    role="combobox"
    aria-label={label}
    aria-autocomplete="list"
    aria-expanded={open}
    aria-controls={`kv-icons-${uid}`}
    aria-activedescendant={activeName === undefined ? undefined : optionId(activeName)}
    placeholder={t("picker.search")}
    autocomplete="off"
    bind:value={query}
    oninput={input}
    onchange={change}
    onkeydown={keydown}
    onblur={() => (open = false)}
  />
  {#if open}
    {#if matches.length === 0}
      <p class="none">{t("picker.noMatch")}</p>
    {:else}
      <div class="popup">
        <ul id={`kv-icons-${uid}`} role="listbox" aria-label={label}>
          {#each rows as name, index (name)}
            <!-- The keys reach the rows through the input's
                 aria-activedescendant (the combobox pattern); a row only
                 takes the pointer. -->
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <li
              id={optionId(name)}
              role="option"
              aria-selected={index === active}
              onmousedown={(event) => event.preventDefault()}
              onclick={() => pick(name)}
            >
              <span class="glyph" aria-hidden="true">{@html tileIcon(name)}</span>
              {name}
            </li>
          {/each}
        </ul>
        {#if matches.length > rows.length}
          <p class="more" role="status">{t("picker.more", { shown: rows.length, total: matches.length })}</p>
        {/if}
      </div>
    {/if}
  {/if}
  {#if value !== ""}
    <span class="value">{value}</span>
  {/if}
</div>

<style>
  .picker {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .popup {
    position: absolute;
    top: 34px;
    inset-inline: 0;
    z-index: 1;
    padding: 4px;
    border: 1px solid var(--kv-fg-muted);
    border-radius: var(--kv-radius);
    background: var(--kv-bg-alt);
    color: var(--kv-fg);
  }

  ul {
    max-height: 240px;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    list-style: none;
  }

  .more {
    margin: 4px 8px 0;
    background: var(--kv-bg-alt);
    color: var(--kv-fg-muted);
    font-size: 0.85em;
  }

  li {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 32px;
    padding: 0 8px;
    border-radius: var(--kv-radius);
    background: var(--kv-bg-alt);
    color: var(--kv-fg);
    cursor: pointer;
  }

  li[aria-selected="true"],
  li:hover {
    background: var(--kv-bg);
    color: var(--kv-fg);
  }

  .glyph {
    display: inline-flex;
    flex: none;
    width: 20px;
    height: 20px;
  }

  .glyph :global(svg) {
    width: 100%;
    height: 100%;
  }

  .none,
  .value {
    margin: 0;
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
  }
</style>
