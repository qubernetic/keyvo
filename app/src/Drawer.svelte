<script lang="ts">
  // The drawer frame (docs/design/gui-ux.md §2, §8; pad screen brief
  // §5.1): a panel fixed to the right of the window, a dialog that is not
  // modal, so the screen beside it stays usable. It takes the focus on its
  // Close button, closes on Escape, and gives the focus back to the
  // control that had it when it opened. The trace entry, Preferences and
  // the keyboard map use it; the app shows one drawer at a time.
  import { onMount, type Snippet } from "svelte";

  import { t } from "./lib/i18n";

  let {
    title,
    close,
    onclose,
    actions,
    children,
  }: {
    /** The drawer's heading and its accessible name. */
    title: string;
    /** The Close button's accessible name. */
    close: string;
    onclose: () => void;
    /** Controls in the header between the title and Close. */
    actions?: Snippet;
    children: Snippet;
  } = $props();

  const id = $props.id();
  // Read before the drawer mounts, while the opener still has the focus.
  const opener = document.activeElement instanceof HTMLElement ? document.activeElement : undefined;
  let button: HTMLButtonElement | undefined = $state();

  onMount(() => {
    button?.focus();
    return () => {
      if (opener?.isConnected === true) opener.focus();
    };
  });

  function keydown(event: KeyboardEvent) {
    // A menu or a field that handled its own Escape keeps the drawer open.
    if (event.key !== "Escape" || event.defaultPrevented) return;
    event.preventDefault();
    onclose();
  }
</script>

<svelte:window onkeydown={keydown} />

<div class="drawer" role="dialog" aria-modal="false" aria-labelledby={`kv-drawer-${id}`}>
  <header>
    <h2 id={`kv-drawer-${id}`}>{title}</h2>
    {@render actions?.()}
    <button bind:this={button} type="button" aria-label={close} onclick={() => onclose()}>
      {t("drawer.closeShort")}
    </button>
  </header>
  {@render children()}
</div>

<style>
  .drawer {
    position: fixed;
    top: 0;
    inset-inline-end: 0;
    bottom: 0;
    z-index: 2;
    box-sizing: border-box;
    width: 360px;
    max-width: 100%;
    padding: 16px;
    overflow: auto;
    border-inline-start: 1px solid var(--kv-fg-muted);
    background: var(--kv-bg-alt);
    color: var(--kv-fg);
  }

  header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  h2 {
    margin: 0 auto 0 0;
    font-size: 1.1em;
  }
</style>
