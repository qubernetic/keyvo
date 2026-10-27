<script lang="ts">
  // The navigation rail (docs/design/gui-ux.md §2, d8; §6, d15; pad screen
  // brief §5.1): Pad, Profiles, Behaviors in order, Ctrl+1 to Ctrl+3,
  // icons with labels or icons only in
  // a narrow window. A screen not built yet stays in the tab order with
  // aria-disabled; hovering says "Coming soon", focus adds the sentence
  // naming its roadmap item, and neither a click nor its shortcut
  // navigates: the shortcut focuses it, so it is announced.
  import Icon from "./Icon.svelte";
  import { t } from "./lib/i18n";
  import { SCREENS, shortcut, type ScreenId } from "./lib/shell";

  let {
    active,
    labels,
    onnavigate,
  }: {
    active: ScreenId;
    /** Labels beside the icons; false in a narrow window. */
    labels: boolean;
    onnavigate: (id: ScreenId) => void;
  } = $props();

  const buttons: HTMLButtonElement[] = $state([]);
  let focused: ScreenId | undefined = $state();
  let hovered: ScreenId | undefined = $state();

  function keydown(event: KeyboardEvent) {
    const index = shortcut(event);
    const screen = index === undefined ? undefined : SCREENS[index];
    if (index === undefined || screen === undefined) return;
    event.preventDefault();
    if (screen.soon === undefined) onnavigate(screen.id);
    else buttons[index]?.focus();
  }
</script>

<svelte:window onkeydown={keydown} />

<nav class="rail" aria-label={t("rail.label")}>
  <ul>
    {#each SCREENS as screen, index (screen.id)}
      <li>
        <button
          bind:this={buttons[index]}
          type="button"
          class="item"
          class:labels
          aria-label={t(screen.label)}
          aria-current={screen.id === active ? "page" : undefined}
          aria-disabled={screen.soon === undefined ? undefined : "true"}
          aria-describedby={screen.soon === undefined ? undefined : `kv-soon-${screen.id}`}
          aria-keyshortcuts={`Control+${index + 1}`}
          onclick={() => {
            if (screen.soon === undefined) onnavigate(screen.id);
          }}
          onfocus={() => (focused = screen.id)}
          onblur={() => (focused = undefined)}
          onmouseenter={() => (hovered = screen.id)}
          onmouseleave={() => (hovered = undefined)}
        >
          <Icon name={screen.icon} />
          {#if labels}
            <span>{t(screen.label)}</span>
          {/if}
        </button>
        {#if screen.soon !== undefined}
          <div
            role="tooltip"
            id={`kv-soon-${screen.id}`}
            hidden={focused !== screen.id && hovered !== screen.id}
          >
            {t("soon")}
            {#if focused === screen.id}
              <p>{t(screen.soon)}</p>
            {/if}
          </div>
        {/if}
      </li>
    {/each}
  </ul>
</nav>

<style>
  .rail {
    height: 100%;
    padding: 8px;
    box-sizing: border-box;
    background: var(--kv-bg-alt);
    color: var(--kv-fg);
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
    position: relative;
  }

  /* §6: the rail's targets are 40 px. */
  .item {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
    min-width: 40px;
    min-height: 40px;
    border-color: transparent;
    background: var(--kv-bg-alt);
    color: var(--kv-fg);
  }

  .item.labels {
    justify-content: flex-start;
    width: 100%;
  }

  .item[aria-current="page"] {
    border-color: var(--kv-accent);
    background: var(--kv-bg);
    color: var(--kv-fg);
  }

  .item[aria-disabled="true"] {
    background: var(--kv-bg-alt);
    color: var(--kv-fg-muted);
    cursor: not-allowed;
  }

  [role="tooltip"] {
    position: absolute;
    top: 0;
    inset-inline-start: calc(100% + 8px);
    z-index: 1;
    width: max-content;
    max-width: 280px;
  }

  [role="tooltip"][hidden] {
    display: none;
  }

  [role="tooltip"] p {
    margin: 4px 0 0;
  }
</style>
