<script lang="ts">
  // The Inspector's control editor (#188): the Dialpad control selected on
  // the Pad's Dialpad panel, in its profile's `[dialpad]` table. The dial
  // and the roller have a label, a step (detents per run, empty for 1) and
  // the two directions, each the key editor's action fields
  // (RotaryFields.svelte, ActionFields.svelte); a button has a label and the action, and its
  // `on_fail` read-only as a key's. Every change is written at once after
  // a snapshot, one write after the other, with undo and redo for the
  // session, like the key editor. A control the profile does not bind and
  // the global profile does says "from global" and offers "Override here";
  // Clear removes the profile's control, so the global one shows through
  // again. An entry of the actions catalogue assigned to a button sets its
  // action and a preset's label; assigned to a rotary it sets the
  // direction last focused, clockwise until the counter-clockwise one was.
  import { untrack, type Snippet } from "svelte";

  import ActionFields from "./ActionFields.svelte";
  import Notice from "./Notice.svelte";
  import RotaryFields from "./RotaryFields.svelte";
  import { appliedToControl, catalogueOf, type CatalogueEntry } from "./lib/catalogue";
  import { bound, controlName, isRoller } from "./lib/dialpad";
  import {
    actionForm,
    afterRedo,
    afterUndo,
    describe,
    emptyHistory,
    fromControlForm,
    problemOf,
    recorded,
    sameControl,
    sameControlFields,
    summaryOf,
    toControlForm,
    type ActionPart,
    type ActionType,
    type ControlChange,
    type ControlEdited,
    type ControlField,
    type ControlFields,
    type ControlForm,
    type ControlTarget,
    type ControlView,
    type Editor,
    type History,
    type Problem,
  } from "./lib/editor";
  import { t, type MessageKey } from "./lib/i18n";
  import { clock } from "./lib/pad";
  import { GLOBAL } from "./lib/profiles";
  import type { Recipe } from "./lib/protocol";

  type Direction = "cw" | "ccw";

  let {
    editor,
    target,
    revision = 0,
    active = true,
    assign,
    entries,
    recipes = [],
    browse,
    onedited,
    ondirection,
  }: {
    editor: Editor;
    /** The control to edit, in the profile the Pad shows. */
    target: ControlTarget;
    /** Bumped when the files may have changed on disk (a reload). */
    revision?: number;
    /** False while another screen shows: Ctrl+Z and Ctrl+Shift+Z are not ours. */
    active?: boolean;
    /** A catalogue entry, by its key, assigned to `target`; a new `seq`
     * applies it once the control is read. */
    assign?: { entry: string; target: ControlTarget; seq: number };
    /** The catalogue's entries for the Action selects, while its column is
     * collapsed. */
    entries?: readonly CatalogueEntry[];
    /** The daemon's recipe packs: what an assigned preset writes. */
    recipes?: readonly Recipe[];
    /** Beside the first Action select: the Pad's Browse actions button. */
    browse?: Snippet;
    /** Called after every write. */
    onedited?: () => void;
    /** The direction of a rotary the catalogue assigns to now. */
    ondirection?: (direction: Direction) => void;
  } = $props();

  const uid = $props.id();
  let view: ControlView | undefined = $state.raw();
  /** The global profile's control of the same name, under an app profile's. */
  let below: ControlView | undefined = $state.raw();
  let form: ControlForm = $state(toControlForm({}));
  /** Why the control cannot be read or the last write failed. */
  let problem: Problem | undefined = $state();
  /** Why the form cannot be written yet, and under which field. */
  let invalid: { message: string; field: ControlField } | undefined = $state();
  /** What the last write did. */
  let note: string | undefined = $state();
  let history: History<ControlChange> = $state.raw(emptyHistory<ControlChange>());
  /** The direction the catalogue assigns to: the one last focused. */
  let direction: Direction = $state("cw");

  const KINDS: Record<ActionType, MessageKey> = {
    none: "inspector.kind.none",
    chord: "inspector.kind.chord",
    exec: "inspector.kind.exec",
    launch: "inspector.kind.launch",
  };

  const rotary = $derived(isRoller(target.control));
  const name = $derived(controlName(target.control));
  /** The profile does not bind the control and the global one does. */
  const fallen = $derived(
    view !== undefined && !bound(view.fields) && below !== undefined && bound(below.fields),
  );
  /** The control the header describes: the global one while it falls through. */
  const shown = $derived(fallen ? below : view);
  const heading = $derived(
    rotary
      ? name
      : t("inspector.controlKind", {
          control: name,
          kind: t(KINDS[shown?.fields.action?.type ?? "none"]),
        }),
  );
  const ids = { heading: `kv-control-${uid}` };

  function now(): string {
    return clock(Date.now());
  }

  /** Writes run one after the other, each against the last one's result. */
  let queue: Promise<void> = Promise.resolve();
  function enqueue(work: () => Promise<void>) {
    queue = queue.then(work);
  }

  function sameForm(a: ControlForm, b: ControlForm): boolean {
    return JSON.stringify(a) === JSON.stringify(b);
  }

  /** Bumped by every read, so a late answer of an earlier one is dropped. */
  let reads = 0;
  /** The control `view` was read for. */
  let readAt: ControlTarget | undefined;
  /** An assigned catalogue entry waiting for its control to be read. */
  let pending: { entry: CatalogueEntry; target: ControlTarget } | undefined;
  /** The `seq` of the last assignment taken. */
  let assigned = 0;

  /** Applies the pending entry once its control is the one the form shows. */
  function settle() {
    if (pending === undefined || view === undefined || !sameControl(readAt, pending.target)) return;
    const { entry, target: at } = pending;
    form = appliedToControl(entry, at.control, direction, form);
    invalid = undefined;
    pending = undefined;
    if (entry.kind === "recipe") commit();
  }

  $effect(() => {
    const next = assign;
    if (next === undefined || next.seq === assigned) return;
    assigned = next.seq;
    const entry = untrack(() => catalogueOf(recipes)).find(
      (candidate) => candidate.key === next.entry,
    );
    if (entry === undefined) return;
    pending = { entry, target: { ...next.target } };
    untrack(settle);
  });

  async function read(at: ControlTarget, keepEdits: boolean) {
    const mine = ++reads;
    try {
      const [got, global] = await Promise.all([
        editor.control(at),
        at.profile === GLOBAL
          ? Promise.resolve(undefined)
          : editor.control({ profile: GLOBAL, control: at.control }),
      ]);
      if (mine !== reads) return;
      const editing = keepEdits && view !== undefined && !sameForm(form, toControlForm(view.fields));
      view = got;
      below = global;
      readAt = at;
      if (!editing) form = toControlForm(got.fields);
      problem = undefined;
      settle();
    } catch (error) {
      if (mine !== reads) return;
      view = undefined;
      below = undefined;
      problem = problemOf(error);
    }
  }

  const identity = $derived(`${target.profile}\n${target.control}`);
  /** Derived, so a parent that hands the same revision again reads nothing. */
  const seenRevision = $derived(revision);
  /** The identity of the control the form shows. */
  let shownId = "";

  $effect(() => {
    const id = identity;
    void seenRevision;
    const at = untrack(() => ({ ...target }));
    // An assignment for a control the user left is dropped.
    if (pending !== undefined && !sameControl(pending.target, at)) pending = undefined;
    // A new revision of the same control keeps a field the user is editing.
    const again = id === shownId;
    shownId = id;
    if (!again) {
      invalid = undefined;
      note = undefined;
      untrack(() => focusDirection("cw"));
    }
    // Only the control and the revision read again, never another prop.
    void untrack(() => read(at, again));
  });

  function focusDirection(next: Direction) {
    if (direction === next) return;
    direction = next;
    ondirection?.(next);
  }

  /** Writes `fields` into the control `at`; the form shows them when `show`. */
  async function write(
    at: ControlTarget,
    fields: ControlFields,
    show: boolean,
  ): Promise<ControlEdited | undefined> {
    try {
      const edited = await editor.editControl(at, fields);
      if (sameControl(at, target)) {
        const onFail = view?.on_fail;
        view = {
          path: edited.path,
          fields: edited.fields,
          ...(onFail === undefined ? {} : { on_fail: onFail }),
        };
        if (show) form = toControlForm(edited.fields);
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

  /** Writes `fields` as one change of the session's history. */
  async function change(fields: ControlFields, show: boolean) {
    const at = { ...target };
    const edited = await write(at, fields, show);
    if (edited === undefined) return;
    history = recorded(history, { target: at, before: edited.before, after: edited.fields });
    note = t("inspector.wroteControlAt", {
      control: controlName(at.control),
      path: edited.path,
      time: now(),
    });
  }

  function commit() {
    enqueue(async () => {
      if (view === undefined) return;
      const result = fromControlForm(target.control, form);
      if ("invalid" in result) {
        invalid = { message: result.invalid, field: result.field };
        return;
      }
      invalid = undefined;
      if (sameControlFields(result.fields, view.fields)) return;
      await change(result.fields, false);
    });
  }

  /** The select of `part` chose `value`: a new action type starts empty; a
   * preset of the collapsed catalogue is applied like an assigned one. */
  function switched(part: Direction | "action", value: string) {
    const preset = entries?.find((entry) => entry.kind === "recipe" && entry.key === value);
    if (preset !== undefined) {
      form = appliedToControl(preset, target.control, part === "action" ? "cw" : part, form);
      invalid = undefined;
    } else {
      form = { ...form, [part]: { ...actionForm(undefined), actionType: value as ActionType } };
    }
    commit();
  }

  function edited(part: Direction | "action", next: ActionPart) {
    form = { ...form, [part]: next };
  }

  /** Every field absent: the profile's control leaves the table. */
  function clear() {
    enqueue(async () => {
      if (view === undefined || !bound(view.fields)) return;
      await change({}, true);
    });
  }

  /** The global control's fields written into the profile's control. */
  function override() {
    enqueue(async () => {
      if (below === undefined) return;
      await change(structuredClone(below.fields), true);
    });
  }

  function replay(
    change: ControlChange | undefined,
    fields: (change: ControlChange) => ControlFields,
    done: () => void,
  ) {
    enqueue(async () => {
      if (change === undefined) return;
      if ((await write(change.target, fields(change), true)) !== undefined) done();
    });
  }

  function undo() {
    const change = history.done.at(-1);
    replay(change, (c) => c.before, () => {
      history = afterUndo(history);
      note = t("inspector.undidControlAt", {
        control: change === undefined ? "" : controlName(change.target.control),
        time: now(),
      });
    });
  }

  function redo() {
    const change = history.undone.at(-1);
    replay(change, (c) => c.after, () => {
      history = afterRedo(history);
      note = t("inspector.redidControlAt", {
        control: change === undefined ? "" : controlName(change.target.control),
        time: now(),
      });
    });
  }

  /** Ctrl+Z and Ctrl+Shift+Z for the session, unless a form field has the
   * focus: there they undo the typing. */
  function keydown(event: KeyboardEvent) {
    if (!active) return;
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

<section class="editor" aria-labelledby={ids.heading}>
  <div class="header">
    <h2 id={ids.heading}>{heading}</h2>
    {#if shown !== undefined}
      <p class="note">
        <span>{t("inspector.controlFile", { file: shown.path })}</span>
        {#if fallen}
          <span class="layer">{t("inspector.fromGlobal")}</span>
        {/if}
      </p>
    {/if}
    <div class="buttons">
      {#if fallen}
        <button type="button" aria-label={t("inspector.override")} onclick={override}>
          {t("inspector.override")}
        </button>
      {/if}
      <button
        type="button"
        aria-label={t("inspector.clear")}
        disabled={view === undefined || !bound(view.fields)}
        onclick={clear}
      >
        {t("inspector.clear")}
      </button>
    </div>
  </div>
  {#if problem !== undefined}
    <Notice kind="error">{describe(problem)}</Notice>
  {/if}
  {#if view !== undefined}
    <form onsubmit={(event) => event.preventDefault()}>
      <label>
        {t("inspector.label")}
        <input type="text" aria-label={t("inspector.label")} bind:value={form.label} onchange={commit} />
      </label>
      {#if rotary}
        <RotaryFields
          {form}
          invalid={invalid === undefined || invalid.field === "action"
            ? undefined
            : { message: invalid.message, field: invalid.field }}
          {entries}
          {browse}
          onstep={(step) => (form.step = step)}
          onedit={edited}
          onswitch={switched}
          oncommit={commit}
          onfocus={focusDirection}
        />
      {:else}
        <ActionFields
          part={form.action}
          invalid={invalid?.field === "action" ? invalid.message : undefined}
          {entries}
          {browse}
          onedit={(next) => edited("action", next)}
          onswitch={(value) => switched("action", value)}
          oncommit={commit}
        />
        {#if shown?.on_fail !== undefined}
          <!-- Read-only like a key's (#183): the editor never writes on_fail. -->
          <p class="hint">
            {t("inspector.onFailRuns")} <code>{summaryOf(shown.on_fail)}</code>
            {t("inspector.onFailWhen")}
          </p>
        {/if}
      {/if}
    </form>
  {/if}
  <div class="buttons">
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
  <p role="status" aria-label={t("inspector.result")} class="result">{note ?? ""}</p>
</section>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }

  .header {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  h2 {
    margin: 0;
    font-size: 1em;
  }

  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: 48ch;
  }

  /* The Inspector's field vocabulary: the label above its field, muted at
     0.85 em, 2 px between them. */
  label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
    font-size: 0.85em;
  }

  .hint {
    margin: 0;
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
    font-size: 1em;
  }

  .note {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin: 0;
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
    overflow-wrap: anywhere;
  }

  .layer {
    background: var(--kv-bg);
    color: var(--kv-fg);
  }

  .result {
    margin: 0;
    background: var(--kv-bg);
    color: var(--kv-fg);
    overflow-wrap: anywhere;
  }

  /* One line is reserved, so the first message shifts nothing below it. */
  .result {
    min-height: 1.4em;
  }
</style>
