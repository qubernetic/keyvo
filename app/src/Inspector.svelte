<script lang="ts">
  // The pinned inspector of the Pad (pad screen brief §5.2, #108 component
  // breakdown): the key the pin and the selection name, with its kind, file
  // and line in the header; Press (a dry run, through the Pad) and Clear;
  // for a key that falls through from the global profile, "from global"
  // and "Override here". The tile row (image, Label, Icon, Colour) and the
  // Action with its field are written to the profile file the moment a
  // field changes, after a snapshot, one write after the other; undo and
  // redo for the session. An invalid field says why under itself (a chord
  // is checked with the daemon's grammar, the wrong token named, and its
  // syntax is a hint line under the field, #167); a
  // refused write is a Notice with its place and the typed value stays. A
  // tool assigned from the actions catalogue sets the action type with an
  // empty field, written once the field is entered; a preset is written at
  // once with its default variant, label and icon. While the catalogue
  // column is collapsed, the Action select carries its entries. A key that
  // carries a preset (lib/catalogue.ts recipeOf) shows its "What it does"
  // select above the Action; another variant rewrites the action. With a
  // Dialpad control selected on the Pad's panel (#188) it is the control
  // editor (ControlEditor.svelte) instead; the key's history stays. Under
  // the Action, the Modifier section (#189): its Mode (Off, Held, Toggle)
  // and, unless off, the dial and the roller the key binds while it has
  // the dial, each with a label and the rotary fields of the control
  // editor (RotaryFields.svelte). Off writes neither; a mode with neither
  // bound writes an empty dial. An entry the actions catalogue assigns
  // while the focus was last in a direction of these goes to that
  // direction, as the control editor's does.
  import { untrack, type Snippet } from "svelte";

  import ActionFields from "./ActionFields.svelte";
  import ControlEditor from "./ControlEditor.svelte";
  import IconPicker from "./IconPicker.svelte";
  import Notice from "./Notice.svelte";
  import RotaryFields from "./RotaryFields.svelte";
  import {
    applied,
    appliedToControl,
    catalogueOf,
    recipeOf,
    withVariant,
    type CatalogueEntry,
  } from "./lib/catalogue";
  import type { Tile } from "./lib/daemon";
  import { controlName } from "./lib/dialpad";
  import {
    actionForm,
    afterRedo,
    afterUndo,
    colorOf,
    describe,
    emptyHistory,
    fromForm,
    problemOf,
    recorded,
    ROTARIES,
    sameFields,
    sameTarget,
    toForm,
    summaryOf,
    viewOf,
    type ActionPart,
    type ActionType,
    type Change,
    type Edited,
    type Editor,
    type Form,
    type FormField,
    type History,
    type KeyFields,
    type KeyView,
    type ModifierChoice,
    type Problem,
    type Target,
    type ControlTarget,
  } from "./lib/editor";
  import { t, type MessageKey } from "./lib/i18n";
  import { clock } from "./lib/pad";
  import type { Recipe, Roller } from "./lib/protocol";

  type Direction = "cw" | "ccw";
  /** A direction of a modifier rotary, where the catalogue assigns. */
  type Aim = { rotary: Roller; direction: Direction };

  let {
    editor,
    target,
    global,
    tile,
    revision = 0,
    active = true,
    connected = true,
    onpress,
    onedited,
    assign,
    entries,
    recipes = [],
    browse,
    control,
    assignControl,
    ondirection,
    onaim,
  }: {
    editor: Editor;
    /** The key to edit: the selected key of the pinned page; none until
     * the user selects one. */
    target?: Target;
    /** The global profile's key under `target` when the pinned page does
     * not define it and the global layer does (spec §6.1). */
    global?: Target;
    /** The key's tile as the pinned page renders it. */
    tile?: Tile;
    /** False while the daemon is unreachable or not yet reached: there is
     * no key to select, and the empty state says so instead. */
    connected?: boolean;
    /** Bumped when the files may have changed on disk (a reload). */
    revision?: number;
    /** False while another screen shows: Ctrl+Z and Ctrl+Shift+Z are not ours. */
    active?: boolean;
    /** Presses the key as a dry run; the Pad shows the result. */
    onpress?: () => void;
    /** Called after every write. */
    onedited?: () => void;
    /** A catalogue entry, by its key, assigned to `target`; a new `seq`
     * applies it once the key is read. With `aim`, to that direction of a
     * modifier rotary while the mode is on. */
    assign?: { entry: string; target: Target; seq: number; aim?: Aim };
    /** The catalogue's entries for the Action select, while its column is
     * collapsed. */
    entries?: readonly CatalogueEntry[];
    /** The daemon's recipe packs: what an assigned preset writes and the
     * "What it does" of a key that carries one. */
    recipes?: readonly Recipe[];
    /** Beside the Action select: the Pad's Browse actions button, while
     * the catalogue column is collapsed (#167). */
    browse?: Snippet;
    /** The Dialpad control selected on the Pad's panel (#188): the
     * inspector shows the control editor instead of the key's. */
    control?: ControlTarget;
    /** A catalogue entry, by its key, assigned to `control`. */
    assignControl?: { entry: string; target: ControlTarget; seq: number };
    /** The direction of a rotary the catalogue assigns to now. */
    ondirection?: (direction: "cw" | "ccw") => void;
    /** The direction of a modifier rotary of the key the catalogue
     * assigns to now; none for the key's action. */
    onaim?: (aim: Aim | undefined) => void;
  } = $props();

  const uid = $props.id();
  let view: KeyView | undefined = $state.raw();
  /** The global key under a key that falls through. */
  let below: KeyView | undefined = $state.raw();
  let form: Form = $state(toForm({}));
  /** Why the key cannot be read or the last write failed. */
  let problem: Problem | undefined = $state();
  /** Why the form cannot be written yet, and under which field. */
  let invalid: { message: string; field: FormField } | undefined = $state();
  /** What the last write did. */
  let note: string | undefined = $state();
  let history: History = $state.raw(emptyHistory());

  const KINDS: Record<ActionType, MessageKey> = {
    none: "inspector.kind.none",
    chord: "inspector.kind.chord",
    exec: "inspector.kind.exec",
    launch: "inspector.kind.launch",
  };

  const fallen = $derived(global !== undefined);
  /** The key the header describes: the global one while it falls through. */
  const shown = $derived(fallen ? below : view);
  const kind = $derived(t(KINDS[shown?.fields.action?.type ?? "none"]));
  const subtitle = $derived(
    shown === undefined
      ? undefined
      : shown.line === undefined
        ? t("inspector.notOnPage", { file: shown.path })
        : t("inspector.subtitle", { file: shown.path, line: shown.line }),
  );
  const swatch = $derived(colorOf(form.color) ?? "");
  /** The preset the key carries as written, and the variant it runs. */
  const carried = $derived(view === undefined ? undefined : recipeOf(recipes, view.fields));
  const ids = {
    heading: `kv-inspector-${uid}`,
    color: `kv-inspector-${uid}-color`,
    modifierHint: `kv-inspector-${uid}-modifier-hint`,
  };

  const MODES: readonly { mode: ModifierChoice; label: MessageKey }[] = [
    { mode: "off", label: "inspector.modifierOff" },
    { mode: "held", label: "inspector.modifierHeld" },
    { mode: "toggle", label: "inspector.modifierToggle" },
  ];

  /** Where the catalogue assigns: a direction of a modifier rotary, the
   * one the focus was last in, or none for the key's action. */
  let aim: Aim | undefined;

  function aimAt(next: Aim | undefined) {
    if (aim?.rotary === next?.rotary && aim?.direction === next?.direction) return;
    aim = next;
    onaim?.(next);
  }

  /** The focus entered the form outside the Modifier section: the
   * catalogue assigns to the key's action again. */
  function focused(event: FocusEvent) {
    if (!(event.target instanceof Element) || event.target.closest(".modifier") === null) {
      aimAt(undefined);
    }
  }

  /** The message of `invalid` when it belongs under a field of `rotary`. */
  function invalidOf(rotary: Roller): { message: string; field: Direction | "step" } | undefined {
    const [owner, field] = invalid?.field.split(".") ?? [];
    return invalid === undefined || owner !== rotary
      ? undefined
      : { message: invalid.message, field: field as Direction | "step" };
  }

  /** The time of a result, 24 h. */
  function now(): string {
    return clock(Date.now());
  }

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
      a.action === b.action &&
      a.windowClass === b.windowClass &&
      a.waitFocus === b.waitFocus &&
      a.modifier === b.modifier &&
      JSON.stringify([a.dial, a.roller]) === JSON.stringify([b.dial, b.roller])
    );
  }

  /** Bumped by every read, so a late answer of an earlier one is dropped. */
  let reads = 0;
  /** The key `view` was read for. */
  let readAt: Target | undefined;
  /** An assigned catalogue entry waiting for its key to be read. */
  let pending: { entry: CatalogueEntry; target: Target; aim?: Aim } | undefined;
  /** The `seq` of the last assignment taken. */
  let assigned = 0;

  /** Applies the pending entry once its key is the one the form shows: to
   * the direction of a modifier rotary it aims at while the mode is on,
   * else to the key. */
  function settle() {
    if (pending === undefined || view === undefined || !sameTarget(readAt, pending.target)) return;
    const { entry, aim: at } = pending;
    if (at !== undefined && form.modifier !== "off") {
      form = { ...form, [at.rotary]: appliedToControl(entry, at.rotary, at.direction, form[at.rotary]) };
    } else {
      form = applied(entry, form);
    }
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
    pending = {
      entry,
      target: { ...next.target },
      ...(next.aim === undefined ? {} : { aim: { ...next.aim } }),
    };
    untrack(settle);
  });

  async function read(at: Target, keepEdits: boolean) {
    const mine = ++reads;
    try {
      const got = await editor.key(at);
      if (mine !== reads) return;
      const editing = keepEdits && view !== undefined && !sameForm(form, toForm(view.fields));
      view = got;
      readAt = at;
      if (!editing) form = toForm(got.fields);
      problem = undefined;
      settle();
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
  let shownId = "";

  $effect(() => {
    const id = identity;
    void revision;
    const at = untrack(() => target);
    if (at === undefined) {
      view = undefined;
      untrack(() => aimAt(undefined));
      return;
    }
    // An assignment for a key the user left is dropped.
    if (pending !== undefined && !sameTarget(pending.target, at)) pending = undefined;
    // A new revision of the same key keeps a field the user is editing.
    const again = id === shownId;
    shownId = id;
    if (!again) {
      invalid = undefined;
      note = undefined;
      untrack(() => aimAt(undefined));
    }
    void read({ ...at }, again);
  });

  /** Bumped by every read of the global key. */
  let belowReads = 0;

  $effect(() => {
    void revision;
    const at = global === undefined ? undefined : { ...global };
    const mine = ++belowReads;
    if (at === undefined) {
      below = undefined;
      return;
    }
    untrack(() => editor.key(at)).then(
      (got) => {
        if (mine === belowReads) below = got;
      },
      (error: unknown) => {
        if (mine !== belowReads) return;
        below = undefined;
        problem = problemOf(error);
      },
    );
  });

  /** Writes `fields` into the key `at`; the form shows them when `show`. */
  async function write(at: Target, fields: KeyFields, show: boolean): Promise<Edited | undefined> {
    try {
      const edited = await editor.editKey(at, fields);
      if (sameTarget(at, target)) {
        view = viewOf(edited, view);
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

  /** Writes `fields` as one change of the session's history. */
  async function change(fields: KeyFields, show: boolean) {
    if (target === undefined) return;
    const at = { ...target };
    const edited = await write(at, fields, show);
    if (edited === undefined) return;
    history = recorded(history, { target: at, before: edited.before, after: edited.fields });
    note = t("inspector.wroteAt", { key: at.key, path: edited.path, time: now() });
  }

  function commit() {
    enqueue(async () => {
      if (target === undefined || view === undefined) return;
      const result = fromForm(form);
      if ("invalid" in result) {
        invalid = { message: result.invalid, field: result.field };
        return;
      }
      invalid = undefined;
      if (sameFields(result.fields, view.fields)) return;
      await change(result.fields, false);
    });
  }

  /** A new action type starts with an empty chord or command, a launch
   * without a window class that waits for its window; a preset of the
   * collapsed catalogue is applied like an assigned one. */
  function switched(value: string) {
    const preset = entries?.find((entry) => entry.kind === "recipe" && entry.key === value);
    if (preset !== undefined) {
      form = applied(preset, form);
      invalid = undefined;
    } else {
      form = { ...form, ...actionForm(undefined), actionType: value as ActionType };
    }
    commit();
  }

  /** Another variant of the carried preset rewrites the action. */
  function vary(variant: string) {
    if (carried === undefined) return;
    form = withVariant(carried.recipe, variant, form);
    commit();
  }

  /** Another Mode: off writes no modifier, a mode with nothing bound an
   * empty dial; the rotaries the form holds come back with a mode. */
  function moded(value: string) {
    form = { ...form, modifier: value as ModifierChoice };
    if (form.modifier === "off") aimAt(undefined);
    commit();
  }

  /** The select of `direction` of `rotary` chose `value`, as the control
   * editor's does. */
  function switchedRotary(rotary: Roller, direction: Direction, value: string) {
    const preset = entries?.find((entry) => entry.kind === "recipe" && entry.key === value);
    form = {
      ...form,
      [rotary]:
        preset !== undefined
          ? appliedToControl(preset, rotary, direction, form[rotary])
          : { ...form[rotary], [direction]: { ...actionForm(undefined), actionType: value as ActionType } },
    };
    if (preset !== undefined) invalid = undefined;
    commit();
  }

  function editedRotary(rotary: Roller, direction: Direction, part: ActionPart) {
    form = { ...form, [rotary]: { ...form[rotary], [direction]: part } };
  }

  function pickIcon(value: string) {
    form.icon = value;
    commit();
  }

  /** Every field absent: the key the page does not define. */
  function clear() {
    enqueue(async () => {
      if (view === undefined || sameFields(view.fields, {})) return;
      await change({}, true);
    });
  }

  /** The global key's fields written into the pinned profile's key. */
  function override() {
    enqueue(async () => {
      if (below === undefined) return;
      await change(structuredClone(below.fields), true);
    });
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
      note = t("inspector.undidAt", { key: String(change?.target.key), time: now() });
    });
  }

  function redo() {
    const change = history.undone.at(-1);
    replay(change, (c) => c.after, () => {
      history = afterRedo(history);
      note = t("inspector.redidAt", { key: String(change?.target.key), time: now() });
    });
  }

  /** Ctrl+Z and Ctrl+Shift+Z for the session, unless a form field has the
   * focus: there they undo the typing. */
  function keydown(event: KeyboardEvent) {
    // The control editor keeps its own history.
    if (!active || control !== undefined) return;
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

{#if control !== undefined}
  <ControlEditor
    {editor}
    target={control}
    {revision}
    {active}
    assign={assignControl}
    {entries}
    {recipes}
    {browse}
    {onedited}
    {ondirection}
  />
{:else}
<section class="inspector" aria-labelledby={ids.heading}>
  {#if target === undefined}
    <h2 id={ids.heading}>{t("inspector.title")}</h2>
    <p class="note">{t(connected ? "inspector.select" : "inspector.noPad")}</p>
  {:else}
    <div class="header">
      <h2 id={ids.heading}>{t("inspector.key", { key: target.key, kind })}</h2>
      {#if subtitle !== undefined}
        <p class="note">
          <span>{subtitle}</span>
          {#if fallen}
            <span class="layer">{t("inspector.fromGlobal")}</span>
          {/if}
        </p>
      {/if}
      <div class="buttons">
        {#if fallen}
          <button
            type="button"
            aria-label={t("inspector.override")}
            disabled={below === undefined}
            onclick={override}
          >
            {t("inspector.override")}
          </button>
        {/if}
        <button
          type="button"
          aria-label={t("inspector.press")}
          disabled={onpress === undefined}
          onclick={() => onpress?.()}
        >
          {t("inspector.press")}
        </button>
        <button
          type="button"
          aria-label={t("inspector.clear")}
          disabled={view === undefined}
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
      <form onsubmit={(event) => event.preventDefault()} onfocusin={focused}>
        <div class="tile">
          {#if tile !== undefined}
            <img src={tile.image} alt={t("inspector.tile", { key: target.key })} />
          {/if}
          <div class="fields">
            <label>
              {t("inspector.label")}
              <input
                type="text"
                aria-label={t("inspector.label")}
                bind:value={form.label}
                onchange={commit}
              />
            </label>
            <div class="field">
              <span aria-hidden="true">{t("inspector.icon")}</span>
              <!-- A search typed for one key does not carry over to another. -->
              {#key identity}
                <IconPicker label={t("inspector.icon")} value={form.icon} onpick={pickIcon} />
              {/key}
            </div>
            <div class="field">
              <span aria-hidden="true">{t("inspector.colour")}</span>
              <span class="colour">
                <input
                  type="color"
                  aria-label={t("inspector.swatch")}
                  value={swatch}
                  oninput={(event) => (form.color = event.currentTarget.value)}
                  onchange={commit}
                />
                <input
                  type="text"
                  aria-label={t("inspector.colour")}
                  placeholder={t("editor.colorExample")}
                  aria-invalid={invalid?.field === "color" ? true : undefined}
                  aria-describedby={invalid?.field === "color" ? ids.color : undefined}
                  bind:value={form.color}
                  onchange={commit}
                />
              </span>
              {#if invalid?.field === "color"}
                <p id={ids.color} role="alert" class="invalid">{invalid.message}</p>
              {/if}
            </div>
          </div>
        </div>
        {#if carried !== undefined}
          {@const preset = carried}
          <label>
            {t("inspector.whatItDoes")}
            <select
              aria-label={t("inspector.whatItDoes")}
              value={preset.variant}
              onchange={(event) => vary(event.currentTarget.value)}
            >
              {#each preset.recipe.variants as variant (variant.id)}
                <option value={variant.id}>{variant.name}</option>
              {/each}
            </select>
          </label>
        {/if}
        <ActionFields
          part={form}
          invalid={invalid?.field === "action" ? invalid.message : undefined}
          {entries}
          {browse}
          onedit={(part) => (form = { ...form, ...part })}
          onswitch={switched}
          oncommit={commit}
        />
        {#if shown?.on_fail !== undefined}
          <!-- Read-only in the beta (#183): the editor never writes on_fail. -->
          <p class="hint">
            {t("inspector.onFailRuns")} <code>{summaryOf(shown.on_fail)}</code>
            {t("inspector.onFailWhen")}
          </p>
        {/if}
        <fieldset class="modifier">
          <legend>{t("inspector.modifier")}</legend>
          <label>
            {t("inspector.modifierMode")}
            <select
              aria-label={t("inspector.modifierMode")}
              aria-describedby={form.modifier === "off" ? undefined : ids.modifierHint}
              value={form.modifier}
              onchange={(event) => moded(event.currentTarget.value)}
            >
              {#each MODES as { mode, label } (mode)}
                <option value={mode}>{t(label)}</option>
              {/each}
            </select>
          </label>
          {#if form.modifier !== "off"}
            <p id={ids.modifierHint} class="hint">
              {t(form.modifier === "held" ? "inspector.modifierHeldHint" : "inspector.modifierToggleHint")}
            </p>
            {#each ROTARIES as rotary (rotary)}
              {@const name = controlName(rotary)}
              {@const labelName = t("inspector.fieldOf", { direction: name, field: t("inspector.label") })}
              <div class="rotary" role="group" aria-label={name}>
                <label>
                  {labelName}
                  <input
                    type="text"
                    aria-label={labelName}
                    value={form[rotary].label}
                    oninput={(event) =>
                      (form = { ...form, [rotary]: { ...form[rotary], label: event.currentTarget.value } })}
                    onchange={commit}
                  />
                </label>
                <RotaryFields
                  form={form[rotary]}
                  {name}
                  invalid={invalidOf(rotary)}
                  {entries}
                  onstep={(step) => (form = { ...form, [rotary]: { ...form[rotary], step } })}
                  onedit={(direction, part) => editedRotary(rotary, direction, part)}
                  onswitch={(direction, value) => switchedRotary(rotary, direction, value)}
                  oncommit={commit}
                  onfocus={(direction) => aimAt({ rotary, direction })}
                />
              </div>
            {/each}
          {/if}
        </fieldset>
      </form>
    {/if}
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
{/if}

<style>
  .inspector {
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

  .tile {
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }

  .tile img {
    flex: none;
    width: 72px;
    height: 72px;
    border-radius: var(--kv-radius);
  }

  .fields {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }

  label,
  .field {
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
    font-size: 0.85em;
  }

  /* The syntax under a field, in the label size of its field. */
  .hint {
    margin: 0;
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
    font-size: 1em;
  }

  .colour {
    display: flex;
    gap: 8px;
  }

  /* The Modifier section: a group like the form, its legend a label. */
  .modifier,
  .rotary {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
    margin: 0;
    padding: 0;
    border: 0;
  }

  .modifier legend {
    margin-bottom: 8px;
    padding: 0;
    background: var(--kv-bg);
    color: var(--kv-fg);
    font-weight: 600;
  }

  .colour input[type="color"] {
    flex: none;
    width: 40px;
    padding: 2px;
  }

  .colour input[type="text"] {
    flex: 1;
    min-width: 0;
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

  .invalid,
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
