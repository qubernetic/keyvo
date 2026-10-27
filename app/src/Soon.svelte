<script lang="ts">
  // A control that is not built yet (docs/design/gui-ux.md §5.2; UI rule
  // of the pre-alpha plan: shown disabled, never faked): it stays in the
  // tab order with aria-disabled, hovering says "Coming soon", focus adds
  // the sentence naming its roadmap item, and a click does nothing. The
  // rail's screens follow the same rule.
  import { t, type MessageKey } from "./lib/i18n";

  let {
    label,
    soon,
    text,
    toggle = false,
    checked = false,
  }: {
    /** The control's name, shown and announced. */
    label: string;
    /** The sentence naming the roadmap item. */
    soon: MessageKey;
    /** What the button shows instead of the label; the label stays its name. */
    text?: string;
    /** A switch instead of a button. */
    toggle?: boolean;
    /** The state the switch shows, as it is today; off by default. */
    checked?: boolean;
  } = $props();

  const id = $props.id();
  let focused = $state(false);
  let hovered = $state(false);
</script>

<span class="soon">
  <button
    type="button"
    role={toggle ? "switch" : undefined}
    aria-checked={toggle ? checked : undefined}
    aria-label={label}
    aria-disabled="true"
    aria-describedby={`kv-soon-${id}`}
    onfocus={() => (focused = true)}
    onblur={() => (focused = false)}
    onmouseenter={() => (hovered = true)}
    onmouseleave={() => (hovered = false)}
  >
    {text ?? label}
  </button>
  <span role="tooltip" id={`kv-soon-${id}`} hidden={!focused && !hovered}>
    {t("soon")}
    {#if focused}
      <span class="sentence">{t(soon)}</span>
    {/if}
  </span>
</span>

<style>
  .soon {
    position: relative;
    display: inline-block;
  }

  button {
    background: var(--kv-bg-alt);
    color: var(--kv-fg-muted);
    cursor: not-allowed;
  }

  [role="tooltip"] {
    position: absolute;
    top: calc(100% + 4px);
    inset-inline-start: 0;
    z-index: 1;
    width: max-content;
    max-width: 280px;
  }

  [role="tooltip"][hidden] {
    display: none;
  }

  .sentence {
    display: block;
    margin-top: 4px;
  }
</style>
