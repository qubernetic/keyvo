<script lang="ts">
  // The page tabs under the drawing (pad screen brief §5.2, decision Q5;
  // #151): the page list the pad walks (spec §6.3) as a tablist, the
  // global pages at index 0 and below muted and marked as the global
  // profile's, then the shown profile's pages by name; arrows wrapping,
  // Home and End; "+" after them is not built yet (M4a-4); the shown
  // profile's accent as a 12 px swatch with its label and `#rrggbb` at the
  // right. Choosing a tab pins that page of the profile that owns it; the
  // hardware page buttons page the pad only.
  import { tick } from "svelte";

  import Soon from "./Soon.svelte";
  import { t } from "./lib/i18n";
  import type { PageTab, ProfileSummary } from "./lib/profiles";

  let {
    profile,
    global,
    tabs,
    selected,
    onpage,
  }: {
    /** The profile whose list the tabs show. */
    profile: ProfileSummary;
    /** The global profile, which owns the tabs at index 0 and below. */
    global: ProfileSummary | undefined;
    /** The list, in index order (lib/profiles.ts `pageList`). */
    tabs: PageTab[];
    /** The index of the selected tab; none selects no tab. */
    selected: number | undefined;
    /** Pins page `page` (from 1) of profile `profile`. */
    onpage: (profile: string, page: number) => void;
  } = $props();

  const buttons: HTMLButtonElement[] = $state([]);
  const at = $derived(tabs.findIndex((tab) => tab.index === selected));

  /** The label of a global tab names the global profile. */
  function label(tab: PageTab): string | undefined {
    return tab.index > 0 ? undefined : t("pad.globalPage", { page: tab.name, profile: global?.name ?? tab.profile });
  }

  async function choose(next: number) {
    const tab = tabs[next];
    if (tab === undefined) return;
    onpage(tab.profile, tab.page);
    await tick();
    buttons[next]?.focus();
  }

  function keydown(event: KeyboardEvent) {
    const count = tabs.length;
    if (count === 0) return;
    const from = Math.max(at, 0);
    const next =
      event.key === "ArrowRight"
        ? (from + 1) % count
        : event.key === "ArrowLeft"
          ? (from + count - 1) % count
          : event.key === "Home"
            ? 0
            : event.key === "End"
              ? count - 1
              : undefined;
    if (next === undefined) return;
    event.preventDefault();
    void choose(next);
  }
</script>

<div class="pages">
  <div role="tablist" aria-label={t("pad.pages")} class="tabs" tabindex="-1" onkeydown={keydown}>
    {#each tabs as tab, position (tab.index)}
      <button
        bind:this={buttons[position]}
        type="button"
        role="tab"
        class:global={tab.index <= 0}
        aria-selected={tab.index === selected}
        tabindex={position === Math.max(at, 0) ? 0 : -1}
        aria-label={label(tab) ?? tab.name}
        title={label(tab)}
        onclick={() => onpage(tab.profile, tab.page)}
      >
        {tab.name}
      </button>
    {/each}
  </div>
  <Soon label={t("pad.addPage")} text={t("pad.add")} soon="soon.addPage" />
  {#if profile.accent !== undefined}
    <div class="accent">
      <span aria-hidden="true">{t("pad.accent")}</span>
      <span
        role="img"
        class="swatch"
        aria-label={t("pad.accentSwatch", { accent: profile.accent })}
        style:background-color={profile.accent}
      ></span>
      <span aria-hidden="true">{profile.accent}</span>
    </div>
  {/if}
</div>

<style>
  .pages {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }

  .tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  /* A global page: muted text and a dashed rim; selected, it keeps the rim. */
  [role="tab"].global {
    border-style: dashed;
    background: var(--kv-bg-alt);
    color: var(--kv-fg-muted);
  }

  [role="tab"][aria-selected="true"] {
    border-color: var(--kv-accent);
    background: var(--kv-bg);
    color: var(--kv-fg);
  }

  .accent {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-left: auto;
  }

  .swatch {
    display: inline-block;
    width: 12px;
    height: 12px;
    border: 1px solid var(--kv-fg-muted);
    border-radius: var(--kv-radius);
  }
</style>
