<script lang="ts">
  // The one component that draws a notice (docs/DESIGN.md "Notices",
  // critique P1): the panel surface with a 1 px full border, the state
  // colour on the border only, a leading word, the body in --kv-fg. An
  // error is announced as an alert. No other component colours a border
  // with a state token, and none draws a side stripe.
  import type { Snippet } from "svelte";

  import { t, type MessageKey } from "./lib/i18n";

  let { kind, children }: { kind: "error" | "note"; children: Snippet } = $props();

  const WORDS: Record<"error" | "note", MessageKey> = {
    error: "notice.error",
    note: "notice.note",
  };
</script>

<div class={["notice", kind]} role={kind === "error" ? "alert" : undefined}>
  <strong>{t(WORDS[kind])}</strong>
  {@render children()}
</div>

<style>
  .notice {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin: 0;
    padding: 8px 12px;
    border: 1px solid var(--kv-fg-muted);
    border-radius: var(--kv-radius);
    background: var(--kv-bg-alt);
    color: var(--kv-fg);
    overflow-wrap: anywhere;
  }

  .error {
    border-color: var(--kv-state-error);
  }
</style>
