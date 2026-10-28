<script lang="ts">
  // The trace strip under the inspector (pad screen brief §5.2, decision 6
  // of #108): one line with the newest entries first and their 24 h times,
  // each a button that opens the drawer on itself. The chevron expands the
  // strip in place to every entry the Pad keeps (PAD_TRACE) and collapses
  // it again. A press and its on_fail run are one line (presses).
  import { t } from "./lib/i18n";
  import { clock, presses, STRIP_LINE, summarize } from "./lib/pad";
  import type { TraceRecord } from "./lib/protocol";

  let {
    entries,
    connecting = false,
    onopen,
  }: {
    /** The entries the Pad keeps, newest first. */
    entries: TraceRecord[];
    /** True while the Pad connects: no empty-state sentence yet. */
    connecting?: boolean;
    /** Opens the drawer on a press's entry and its on_fail run. */
    onopen: (entry: TraceRecord, onFail?: TraceRecord) => void;
  } = $props();

  const uid = $props.id();
  let expanded = $state(false);
  const rows = $derived(presses(entries));
  const shown = $derived(expanded ? rows : rows.slice(0, STRIP_LINE));
</script>

<section class="strip" class:expanded aria-labelledby={`kv-strip-${uid}`}>
  <div class="line">
    <h2 id={`kv-strip-${uid}`}>{t("trace.strip")}</h2>
    {#if entries.length === 0}
      {#if !connecting}
        <p class="note">{t("home.noEvents")}</p>
      {/if}
    {:else}
      <ol aria-label={t("trace.strip")}>
        {#each shown as row, index (index)}
          {@const line = summarize(row.entry, row.onFail)}
          {@const time = clock(row.entry.timestamp_ms)}
          <li class:newest={index === 0}>
            <button
              type="button"
              class="entry"
              aria-label={t("trace.open", { time, line })}
              onclick={() => onopen(row.entry, row.onFail)}
            >
              <time>{time}</time>
              {line}
            </button>
          </li>
        {/each}
      </ol>
    {/if}
    {#if rows.length > STRIP_LINE}
      <button
        type="button"
        class="chevron"
        aria-label={t(expanded ? "trace.collapse" : "trace.expand")}
        aria-expanded={expanded}
        onclick={() => (expanded = !expanded)}
      >
        <span class="mark" class:open={expanded} aria-hidden="true"></span>
      </button>
    {/if}
  </div>
</section>

<style>
  .line {
    display: flex;
    gap: 8px;
    align-items: flex-start;
  }

  h2 {
    flex: none;
    margin: 0;
    padding: 6px 0;
    font-size: 1em;
  }

  ol {
    display: flex;
    flex: 1;
    gap: 4px;
    min-width: 0;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  /* Collapsed, the entries share one line: the newest is read in full
     (an ellipsis only when it alone is longer than the line), the older
     ones take what is left and give way to nothing before it loses a
     character. */
  .strip:not(.expanded) li.newest {
    flex: 0 1 auto;
    max-width: 100%;
  }

  .strip:not(.expanded) li:not(.newest) {
    flex: 1 1 0;
    min-width: 0;
    overflow: hidden;
  }

  .strip:not(.expanded) .entry {
    width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .expanded ol {
    flex-direction: column;
  }

  .entry {
    border-color: transparent;
    background: var(--kv-bg);
    color: var(--kv-fg);
    text-align: start;
  }

  time {
    margin-inline-end: 8px;
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
  }

  .note {
    flex: 1;
    margin: 0;
    padding: 6px 0;
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
  }

  .chevron {
    flex: none;
  }

  /* A filled triangle in the body colour: down while collapsed, up open. */
  .mark {
    display: block;
    width: 8px;
    height: 8px;
    background: var(--kv-fg);
    color: var(--kv-bg);
    clip-path: polygon(0 0, 100% 0, 50% 100%);
  }

  .mark.open {
    transform: rotate(180deg);
  }
</style>
