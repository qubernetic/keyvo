<script lang="ts">
  // The trace drawer (docs/design/gui-ux.md §8, d9, P4): one trace entry
  // as sections in resolution order. It renders the parts the daemon
  // records today (spec §8): the trigger, the context snapshot (profile
  // and page), the resolved action with the layer that supplied it, and
  // the execution result or the block reason. Conditions, variables and
  // the window context arrive with later slices. The frame is Drawer's;
  // its header carries the Debug switch, off until the daemon sends
  // HID-level events.
  import Drawer from "./Drawer.svelte";
  import Soon from "./Soon.svelte";
  import { t, type MessageKey } from "./lib/i18n";
  import type { TraceRecord } from "./lib/protocol";

  let { entry, onclose }: { entry: TraceRecord; onclose: () => void } = $props();

  interface Row {
    term: MessageKey;
    value: string;
  }

  interface Section {
    title: MessageKey;
    rows?: Row[];
    text?: string;
  }

  function trigger(entry: TraceRecord): Section {
    return {
      title: "drawer.trigger",
      rows: [
        { term: "drawer.time", value: new Date(entry.timestamp_ms).toLocaleString() },
        { term: "drawer.key", value: String(entry.key) },
        { term: "drawer.press", value: entry.trigger.press },
        {
          term: "drawer.source",
          value: t(entry.trigger.simulated ? "drawer.simulated" : "drawer.device"),
        },
      ],
    };
  }

  function context(entry: TraceRecord): Section {
    return {
      title: "drawer.context",
      rows: [
        { term: "drawer.profile", value: entry.profile },
        { term: "drawer.page", value: String(entry.page) },
      ],
    };
  }

  function action(entry: TraceRecord): Section {
    const rows: Row[] = [
      {
        term: "drawer.layer",
        value: entry.layer === undefined ? t("drawer.none") : t("layer", { layer: entry.layer }),
      },
      { term: "drawer.type", value: entry.action?.type ?? t("drawer.none") },
    ];
    if (entry.action?.type === "chord") rows.push({ term: "drawer.keys", value: entry.action.keys });
    if (entry.action?.type === "exec") rows.push({ term: "drawer.command", value: entry.action.cmd });
    return { title: "drawer.action", rows };
  }

  /** One `{code, down}` key event of a sent chord. */
  function keyEvent(event: unknown): string {
    const { code, down } = event as { code: number; down: boolean };
    return t(down ? "drawer.down" : "drawer.up", { code });
  }

  /** The result, or the reason nothing was executed. */
  function outcome(entry: TraceRecord): Section {
    const result = entry.result;
    switch (result.status) {
      case "dry_run":
        return { title: "drawer.block", text: t("drawer.dryRun") };
      case "unbound":
        return { title: "drawer.block", text: t("drawer.unbound") };
      case "pending":
        return { title: "drawer.result", rows: [{ term: "drawer.status", value: t("trace.running") }] };
      case "sent":
        return {
          title: "drawer.result",
          rows: [
            { term: "drawer.status", value: result.status },
            {
              term: "drawer.events",
              value: (Array.isArray(result.events) ? result.events : []).map(keyEvent).join(", "),
            },
          ],
        };
      case "exited":
        return {
          title: "drawer.result",
          rows: [
            { term: "drawer.status", value: result.status },
            {
              term: "drawer.code",
              value: typeof result.code === "number" ? String(result.code) : t("drawer.signal"),
            },
            { term: "drawer.duration", value: t("drawer.ms", { ms: String(result.duration_ms) }) },
          ],
        };
      case "failed":
        return {
          title: "drawer.result",
          rows: [
            { term: "drawer.status", value: result.status },
            { term: "drawer.error", value: String(result.error) },
          ],
        };
      default:
        return { title: "drawer.result", rows: [{ term: "drawer.status", value: result.status }] };
    }
  }

  const sections: Section[] = $derived([trigger(entry), context(entry), action(entry), outcome(entry)]);
</script>

<Drawer title={t("drawer.title")} close={t("drawer.close")} {onclose}>
  {#snippet actions()}
    <!-- §3.1: Debug mode adds HID-level events; the daemon sends none yet. -->
    <Soon label={t("drawer.debug")} soon="soon.debug" toggle />
  {/snippet}
  {#each sections as section, index (index)}
    <section aria-labelledby={`kv-trace-${index}`}>
      <h3 id={`kv-trace-${index}`}>{t(section.title)}</h3>
      {#if section.rows !== undefined}
        <dl>
          {#each section.rows as row (row.term)}
            <dt>{t(row.term)}</dt>
            <dd>{row.value}</dd>
          {/each}
        </dl>
      {/if}
      {#if section.text !== undefined}
        <p>{section.text}</p>
      {/if}
    </section>
  {/each}
</Drawer>

<style>
  h3 {
    margin: 16px 0 4px;
    font-size: 1em;
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 4px 12px;
    margin: 0;
  }

  dt {
    background: var(--kv-bg-alt);
    color: var(--kv-fg-muted);
  }

  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }

  p {
    margin: 0;
  }
</style>
