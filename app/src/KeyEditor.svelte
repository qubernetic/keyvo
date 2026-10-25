<script lang="ts">
  // The key editor beside the mirror (docs/design/gui-ux.md §3.2 and §4.1,
  // pre-alpha subset): the label, color, icon and action of one key, each
  // written to the profile file the moment its field changes, after a
  // snapshot; undo and redo for the session; a problem shown with its file
  // and line. The daemon reloads the file and repaints the pad.
  import { untrack } from "svelte";

  import {
    afterRedo,
    afterUndo,
    describe,
    emptyHistory,
    fromForm,
    problemOf,
    recorded,
    sameFields,
    sameTarget,
    toForm,
    type Change,
    type Edited,
    type Editor,
    type Form,
    type History,
    type KeyFields,
    type KeyView,
    type Problem,
    type Target,
  } from "./lib/editor";
  import { t } from "./lib/i18n";

  let {
    editor,
    target,
    revision = 0,
    onedited,
  }: {
    editor: Editor;
    /** The key to edit; none until the daemon reports a view. */
    target?: Target;
    /** Bumped when the files may have changed on disk (a reload). */
    revision?: number;
    /** Called after every write. */
    onedited?: () => void;
  } = $props();

  let view: KeyView | undefined = $state.raw();
  let form: Form = $state(toForm({}));
  /** Why the key cannot be read or the last write failed. */
  let problem: Problem | undefined = $state();
  /** Why the form cannot be written yet. */
  let invalid: string | undefined = $state();
  /** What the last write did. */
  let note: string | undefined = $state();
  let history: History = $state.raw(emptyHistory());
  const actionLabel = $derived(
    form.actionType === "chord" ? t("editor.keys") : t("editor.command"),
  );

  /** Writes run one after the other, each against the last one's result. */
  let queue: Promise<void> = Promise.resolve();
  function enqueue(work: () => Promise<void>) {
    queue = queue.then(work);
  }

  function sameForm(a: Form, b: Form): boolean {
    return (
      a.label === b.label &&
      a.color === b.color &&
      a.icon === b.icon &&
      a.actionType === b.actionType &&
      a.action === b.action
    );
  }

  /** Bumped by every read, so a late answer of an earlier one is dropped. */
  let reads = 0;

  async function read(at: Target, keepEdits: boolean) {
    const mine = ++reads;
    try {
      const got = await editor.key(at);
      if (mine !== reads) return;
      const editing = keepEdits && view !== undefined && !sameForm(form, toForm(view.fields));
      view = got;
      if (!editing) form = toForm(got.fields);
      problem = undefined;
    } catch (error) {
      if (mine !== reads) return;
      view = undefined;
      problem = problemOf(error);
    }
  }

  const identity = $derived(
    target === undefined ? "" : `${target.profile}\n${target.page}\n${target.key}`,
  );
  /** The identity of the key the form shows. */
  let shown = "";

  $effect(() => {
    const id = identity;
    void revision;
    const at = untrack(() => target);
    if (at === undefined) {
      view = undefined;
      return;
    }
    // A new revision of the same key keeps a field the user is editing.
    const again = id === shown;
    shown = id;
    if (!again) {
      invalid = undefined;
      note = undefined;
    }
    void read({ ...at }, again);
  });

  /** Writes `fields` into the key `at`; the form shows them when `show`. */
  async function write(at: Target, fields: KeyFields, show: boolean): Promise<Edited | undefined> {
    try {
      const edited = await editor.editKey(at, fields);
      if (sameTarget(at, target)) {
        view = { path: edited.path, fields: edited.fields };
        if (show) form = toForm(edited.fields);
      }
      problem = undefined;
      invalid = undefined;
      onedited?.();
      return edited;
    } catch (error) {
      problem = problemOf(error);
      return undefined;
    }
  }

  function commit() {
    enqueue(async () => {
      if (target === undefined || view === undefined) return;
      const result = fromForm(form);
      if ("invalid" in result) {
        invalid = result.invalid;
        return;
      }
      invalid = undefined;
      if (sameFields(result.fields, view.fields)) return;
      const at = { ...target };
      const edited = await write(at, result.fields, false);
      if (edited === undefined) return;
      history = recorded(history, { target: at, before: edited.before, after: edited.fields });
      note = t("editor.wrote", { key: at.key, path: edited.path });
    });
  }

  /** A new action type starts with an empty chord or command. */
  function switched() {
    form.action = "";
    commit();
  }

  function replay(change: Change | undefined, fields: (change: Change) => KeyFields, done: () => void) {
    enqueue(async () => {
      if (change === undefined) return;
      if ((await write(change.target, fields(change), true)) !== undefined) done();
    });
  }

  function undo() {
    const change = history.done.at(-1);
    replay(change, (c) => c.before, () => {
      history = afterUndo(history);
      note = t("editor.undid", { key: String(change?.target.key) });
    });
  }

  function redo() {
    const change = history.undone.at(-1);
    replay(change, (c) => c.after, () => {
      history = afterRedo(history);
      note = t("editor.redid", { key: String(change?.target.key) });
    });
  }

  /** Ctrl+Z and Ctrl+Shift+Z for the session, unless a form field has the
   * focus: there they undo the typing. */
  function keydown(event: KeyboardEvent) {
    if (!(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== "z") return;
    const focus = event.target;
    if (
      focus instanceof HTMLInputElement ||
      focus instanceof HTMLSelectElement ||
      focus instanceof HTMLTextAreaElement
    ) {
      return;
    }
    event.preventDefault();
    if (event.shiftKey) redo();
    else undo();
  }
</script>

<svelte:window onkeydown={keydown} />

<section class="editor" aria-labelledby="kv-editor">
  {#if target === undefined}
    <h2 id="kv-editor">{t("editor.title")}</h2>
    <p class="note">{t("editor.select")}</p>
  {:else}
    <h2 id="kv-editor">{t("editor.key", { key: target.key })}</h2>
    {#if problem !== undefined}
      <p role="alert" class="problem">{describe(problem)}</p>
    {/if}
    {#if view !== undefined}
      <p class="note">{t("editor.file")} <code>{view.path}</code></p>
      <form onsubmit={(event) => event.preventDefault()}>
        <label>
          {t("editor.label")}
          <input
            type="text"
            aria-label={t("editor.label")}
            bind:value={form.label}
            onchange={commit}
          />
        </label>
        <label>
          {t("editor.color")}
          <input
            type="text"
            aria-label={t("editor.color")}
            placeholder={t("editor.colorExample")}
            bind:value={form.color}
            onchange={commit}
          />
        </label>
        <label>
          {t("editor.icon")}
          <input
            type="text"
            aria-label={t("editor.icon")}
            placeholder={t("editor.iconExample")}
            bind:value={form.icon}
            onchange={commit}
          />
        </label>
        <label>
          {t("editor.action")}
          <select aria-label={t("editor.action")} bind:value={form.actionType} onchange={switched}>
            <option value="none">{t("editor.none")}</option>
            <option value="chord">{t("editor.chord")}</option>
            <option value="exec">{t("editor.exec")}</option>
          </select>
        </label>
        {#if form.actionType !== "none"}
          <label>
            {actionLabel}
            <input
              type="text"
              aria-label={actionLabel}
              placeholder={form.actionType === "chord"
                ? t("editor.chordExample")
                : t("editor.commandExample")}
              bind:value={form.action}
              onchange={commit}
            />
          </label>
        {/if}
      </form>
      {#if invalid !== undefined}
        <p role="alert" class="problem">{invalid}</p>
      {/if}
    {/if}
  {/if}
  <div class="history">
    <button
      type="button"
      aria-label={t("editor.undo")}
      aria-keyshortcuts="Control+Z"
      disabled={history.done.length === 0}
      onclick={undo}
    >
      {t("editor.undo")}
    </button>
    <button
      type="button"
      aria-label={t("editor.redo")}
      aria-keyshortcuts="Control+Shift+Z"
      disabled={history.undone.length === 0}
      onclick={redo}
    >
      {t("editor.redo")}
    </button>
  </div>
  <p role="status" aria-label={t("editor.result")} class="note">{note ?? ""}</p>
</section>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 260px;
  }

  h2 {
    margin: 0;
    font-size: 1em;
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    color: var(--kv-fg-muted);
    font-size: 0.85em;
  }

  input,
  select {
    min-height: 32px;
    padding: 0 8px;
    border: 1px solid var(--kv-bg-alt);
    border-radius: var(--kv-radius);
    background: var(--kv-bg-alt);
    color: var(--kv-fg);
    font: inherit;
    font-size: var(--kv-font-size);
  }

  input:focus-visible,
  select:focus-visible {
    border-color: var(--kv-accent);
    outline: none;
  }

  .history {
    display: flex;
    gap: 8px;
  }

  .history button {
    min-height: 32px;
  }

  .note {
    margin: 0;
    color: var(--kv-fg-muted);
    overflow-wrap: anywhere;
  }

  .problem {
    margin: 0;
    padding: 8px 12px;
    background: var(--kv-bg-alt);
    border-left: 4px solid var(--kv-state-error);
    border-radius: var(--kv-radius);
    overflow-wrap: anywhere;
  }
</style>
