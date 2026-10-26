<script lang="ts">
  // PROTOTYPE (#107): the floating bar that cycles the variants of the
  // Profiles screen (`?variant=`), the prototype skill's switcher. Its
  // colours are deliberately not the app's tokens: the bar is not part of
  // the design under judgement. Dev builds only; deleted in #108.
  import { VARIANTS, type VariantKey } from "./model";

  let {
    current,
    onchange,
  }: {
    current: VariantKey;
    onchange: (next: VariantKey) => void;
  } = $props();

  const index = $derived(VARIANTS.findIndex((variant) => variant.key === current));
  const name = $derived(VARIANTS[index]?.name ?? "");

  function cycle(step: number) {
    const next = VARIANTS[(index + step + VARIANTS.length) % VARIANTS.length];
    if (next === undefined) return;
    const url = new URL(window.location.href);
    url.searchParams.set("variant", next.key);
    window.history.replaceState(null, "", url);
    onchange(next.key);
  }

  function keydown(event: KeyboardEvent) {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    const focus = event.target;
    if (
      focus instanceof HTMLInputElement ||
      focus instanceof HTMLTextAreaElement ||
      focus instanceof HTMLSelectElement ||
      (focus instanceof HTMLElement && (focus.isContentEditable || focus.closest("[role=grid],[role=tablist],[role=group]") !== null))
    ) {
      return;
    }
    event.preventDefault();
    cycle(event.key === "ArrowLeft" ? -1 : 1);
  }
</script>

<svelte:window onkeydown={keydown} />

{#if import.meta.env.DEV}
  <div class="bar" role="toolbar" aria-label="Prototype variants">
    <button type="button" aria-label="Previous variant" onclick={() => cycle(-1)}>&larr;</button>
    <span class="label"><strong>{current.toUpperCase()}</strong> {name}</span>
    <button type="button" aria-label="Next variant" onclick={() => cycle(1)}>&rarr;</button>
  </div>
{/if}

<style>
  .bar {
    position: fixed;
    bottom: 16px;
    left: 50%;
    z-index: 1000;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 10px;
    border: 2px solid #ff2fa8;
    border-radius: 999px;
    background: #121212;
    color: #ffffff;
    font-family: ui-monospace, monospace;
    font-size: 13px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.5);
    transform: translateX(-50%);
  }

  .bar button {
    min-height: 26px;
    padding: 0 10px;
    border: 1px solid #ff2fa8;
    border-radius: 999px;
    background: #121212;
    color: #ff2fa8;
    font-size: 13px;
  }

  .label strong {
    color: #ff2fa8;
  }
</style>
