<script lang="ts">
  // The keyboard map (pad screen brief §7; critique heuristic 6): the
  // overflow menu's "Keyboard shortcuts" as a non-modal drawer, one table
  // row per shortcut of lib/shortcuts.ts; a shortcut not built yet names
  // its roadmap item beside its action.
  import Drawer from "./Drawer.svelte";
  import { t } from "./lib/i18n";
  import { SHORTCUTS } from "./lib/shortcuts";

  let { onclose }: { onclose: () => void } = $props();
</script>

<Drawer title={t("shortcuts.title")} close={t("shortcuts.close")} {onclose}>
  <table aria-label={t("shortcuts.title")}>
    <thead>
      <tr>
        <th scope="col">{t("shortcuts.keysColumn")}</th>
        <th scope="col">{t("shortcuts.actionColumn")}</th>
      </tr>
    </thead>
    <tbody>
      {#each SHORTCUTS as row (row.keys)}
        <tr>
          <td><kbd>{t(row.keys)}</kbd></td>
          <td>
            {t(row.action)}
            {#if row.soon !== undefined}
              <span class="soon">{t(row.soon)}</span>
            {/if}
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</Drawer>

<style>
  table {
    width: 100%;
    margin-top: 16px;
    border-collapse: collapse;
  }

  th {
    text-align: start;
    background: var(--kv-bg-alt);
    color: var(--kv-fg-muted);
    font-weight: normal;
  }

  th,
  td {
    padding: 4px 8px 4px 0;
    vertical-align: top;
  }

  kbd {
    font-family: var(--kv-font);
    white-space: nowrap;
  }

  .soon {
    display: block;
    background: var(--kv-bg-alt);
    color: var(--kv-fg-muted);
  }
</style>
