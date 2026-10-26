<script lang="ts">
  // The page tabs under the drawing (pad screen brief §5.2, decision Q5):
  // the pinned profile's pages as a tablist by name, arrows wrapping, Home
  // and End; "+" is not built yet (M4a-4); the profile's accent as a 12 px
  // swatch with its label and `#rrggbb` at the right. Choosing a tab pins
  // the page; the hardware page buttons page the pad only.
  import { tick } from "svelte";

  import Soon from "./Soon.svelte";
  import { t } from "./lib/i18n";
  import { pageName, type ProfileSummary } from "./lib/profiles";

  let {
    profile,
    page,
    onpage,
  }: {
    /** The pinned profile. */
    profile: ProfileSummary;
    /** The pinned page, from 1. */
    page: number;
    /** Pins page `page`, from 1. */
    onpage: (page: number) => void;
  } = $props();

  const pages = $derived(profile.pages.map((_, index) => index + 1));
  const tabs: HTMLButtonElement[] = $state([]);

  async function choose(next: number) {
    onpage(next);
    await tick();
    tabs[next - 1]?.focus();
  }

  function keydown(event: KeyboardEvent) {
    const last = pages.length;
    if (last === 0) return;
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
    if (next === undefined) return;
    event.preventDefault();
    void choose(next);
  }
</script>

<div class="pages">
  <div role="tablist" aria-label={t("pad.pages")} class="tabs" tabindex="-1" onkeydown={keydown}>
    {#each pages as number (number)}
      <button
        bind:this={tabs[number - 1]}
        type="button"
        role="tab"
        aria-selected={number === page}
        tabindex={number === page ? 0 : -1}
        onclick={() => onpage(number)}
      >
        {pageName(profile, number)}
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
