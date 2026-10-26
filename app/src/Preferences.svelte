<script lang="ts">
  // Preferences (pad screen brief §5.1; #108 open point 5): the gear's
  // drawer, non-modal in the same window (gui-ux.md §2 keeps modals for
  // the one confirmation). The tabs General, Devices, Integrations and
  // Projects are not built yet, so each panel is the sentence of its
  // roadmap item. Arrows, Home and End move between the tabs.
  import Drawer from "./Drawer.svelte";
  import { t } from "./lib/i18n";
  import { PREFERENCES, type PreferenceId } from "./lib/shell";

  let { onclose }: { onclose: () => void } = $props();

  const id = $props.id();
  let selected: PreferenceId = $state("general");
  const tabs: HTMLButtonElement[] = $state([]);
  const panel = $derived(PREFERENCES.find((tab) => tab.id === selected) ?? PREFERENCES[0]);

  function keydown(event: KeyboardEvent, index: number) {
    const last = PREFERENCES.length - 1;
    const next =
      event.key === "ArrowRight"
        ? (index + 1) % PREFERENCES.length
        : event.key === "ArrowLeft"
          ? (index + last) % PREFERENCES.length
          : event.key === "Home"
            ? 0
            : event.key === "End"
              ? last
              : undefined;
    const tab = next === undefined ? undefined : PREFERENCES[next];
    if (next === undefined || tab === undefined) return;
    event.preventDefault();
    selected = tab.id;
    tabs[next]?.focus();
  }
</script>

<Drawer title={t("preferences.title")} close={t("preferences.close")} {onclose}>
  <div class="tabs" role="tablist" aria-label={t("preferences.title")}>
    {#each PREFERENCES as tab, index (tab.id)}
      <button
        bind:this={tabs[index]}
        type="button"
        role="tab"
        id={`kv-pref-${id}-${tab.id}`}
        aria-label={t(tab.label)}
        aria-selected={tab.id === selected}
        aria-controls={`kv-pref-${id}-panel`}
        tabindex={tab.id === selected ? 0 : -1}
        onclick={() => (selected = tab.id)}
        onkeydown={(event) => keydown(event, index)}
      >
        {t(tab.label)}
      </button>
    {/each}
  </div>
  {#if panel !== undefined}
    <div
      role="tabpanel"
      id={`kv-pref-${id}-panel`}
      aria-labelledby={`kv-pref-${id}-${panel.id}`}
      tabindex="0"
    >
      <p>{t(panel.soon)}</p>
    </div>
  {/if}
</Drawer>

<style>
  .tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin: 16px 0;
  }

  [role="tab"][aria-selected="true"] {
    border-color: var(--kv-accent);
    background: var(--kv-bg);
    color: var(--kv-fg);
  }

  p {
    margin: 0;
  }
</style>
