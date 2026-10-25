<script lang="ts">
  // The columns of a screen (docs/design/gui-ux.md §2, d8): the list
  // column, the editor and the right column. The window width decides
  // which show (lib/shell.ts `layout`: the right column collapses first,
  // then the list column); the toggle at a column's edge shows or hides it
  // until the width crosses that column's limit again.
  import type { Snippet } from "svelte";

  import Icon from "./Icon.svelte";
  import { t } from "./lib/i18n";
  import type { Columns } from "./lib/shell";

  let {
    columns,
    list,
    editor,
    right,
  }: {
    columns: Columns;
    /** The screen's list column, if it has one. */
    list?: Snippet;
    editor: Snippet;
    /** The screen's right column, if it has one. */
    right?: Snippet;
  } = $props();

  /** The user's toggle, kept while the width-driven state it overrode holds. */
  interface Override {
    over: boolean;
    shown: boolean;
  }
  let listOverride: Override | undefined = $state();
  let rightOverride: Override | undefined = $state();

  function visible(byWidth: boolean, override: Override | undefined): boolean {
    return override?.over === byWidth ? override.shown : byWidth;
  }

  const listShown = $derived(visible(columns.list, listOverride));
  const rightShown = $derived(visible(columns.right, rightOverride));
</script>

<div class="columns">
  {#if list !== undefined}
    <div class="side">
      <button
        type="button"
        class="toggle"
        aria-controls="kv-list"
        aria-expanded={listShown}
        aria-label={listShown ? t("columns.hideList") : t("columns.showList")}
        onclick={() => (listOverride = { over: columns.list, shown: !listShown })}
      >
        <Icon name="panel-left" />
      </button>
      {#if listShown}
        <section id="kv-list" class="list" aria-label={t("columns.list")}>
          {@render list()}
        </section>
      {/if}
    </div>
  {/if}
  <section class="editor" aria-label={t("columns.editor")}>
    {@render editor()}
  </section>
  {#if right !== undefined}
    <div class="side">
      {#if rightShown}
        <section id="kv-right" class="right" aria-label={t("columns.right")}>
          {@render right()}
        </section>
      {/if}
      <button
        type="button"
        class="toggle"
        aria-controls="kv-right"
        aria-expanded={rightShown}
        aria-label={rightShown ? t("columns.hideRight") : t("columns.showRight")}
        onclick={() => (rightOverride = { over: columns.right, shown: !rightShown })}
      >
        <Icon name="panel-right" />
      </button>
    </div>
  {/if}
</div>

<style>
  .columns {
    display: flex;
    gap: 16px;
    height: 100%;
    min-height: 0;
  }

  .side {
    display: flex;
    flex: none;
    align-items: flex-start;
    gap: 8px;
  }

  .toggle {
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 32px;
    padding: 0;
  }

  .list {
    width: 240px;
    overflow: auto;
  }

  .editor {
    flex: 1;
    min-width: 0;
    overflow: auto;
  }

  .right {
    width: 320px;
    overflow: auto;
  }
</style>
