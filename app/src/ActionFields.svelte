<script lang="ts">
  // The action part of an editor form (docs/design/gui-ux.md §3.1): the
  // Action select, or the catalogue's entries in it while the catalogue
  // column is collapsed, and the kind's fields: the chord with its syntax
  // hint, or the command, and for a launch its window class and the
  // switch Wait for the window. The key's action in the Inspector, and each
  // direction of a Dialpad rotary and a button's action in the control
  // editor (#188). The fields are the caller's: each change hands the new
  // part to `onedit`, a finished one calls `oncommit`, another select
  // value `onswitch`; the caller writes and says what is invalid.
  import type { Snippet } from "svelte";

  import { GROUPS, type CatalogueEntry } from "./lib/catalogue";
  import type { ActionPart } from "./lib/editor";
  import { t } from "./lib/i18n";

  let {
    part,
    label = t("inspector.action"),
    direction,
    invalid,
    entries,
    browse,
    onedit,
    onswitch,
    oncommit,
  }: {
    part: ActionPart;
    /** The select's label: Action, or a rotary's direction. */
    label?: string;
    /** Names the fields after the select's direction, so the two
     * directions of a rotary tell their fields apart. */
    direction?: string;
    /** Why the action cannot be written yet, under its field. */
    invalid?: string;
    /** The catalogue's entries for the select, while its column is
     * collapsed. */
    entries?: readonly CatalogueEntry[];
    /** Beside the select: the Pad's Browse actions button (#167). */
    browse?: Snippet;
    /** A field changed: the part as it reads now. */
    onedit: (part: ActionPart) => void;
    /** The select chose `value`: an action type, or a catalogue entry's key. */
    onswitch: (value: string) => void;
    /** A field was finished: write the form. */
    oncommit: () => void;
  } = $props();

  const uid = $props.id();
  const ids = {
    action: `kv-action-${uid}`,
    hint: `kv-action-${uid}-hint`,
    classHint: `kv-action-${uid}-class-hint`,
    select: `kv-action-${uid}-select`,
  };

  /** A field's name, after the direction when there is one. */
  function named(field: string): string {
    return direction === undefined ? field : t("inspector.fieldOf", { direction, field });
  }

  const actionLabel = $derived(
    named(part.actionType === "chord" ? t("inspector.chord") : t("inspector.command")),
  );
  /** The action field's message, then the chord syntax under a chord. */
  const describedBy = $derived(
    [
      ...(invalid !== undefined ? [ids.action] : []),
      ...(part.actionType === "chord" ? [ids.hint] : []),
    ].join(" ") || undefined,
  );
</script>

<div class="field">
  <label for={ids.select}>{label}</label>
  <span class="choice">
    <select
      id={ids.select}
      aria-label={label}
      value={part.actionType}
      onchange={(event) => onswitch(event.currentTarget.value)}
    >
      <option value="none">{t("inspector.kind.none")}</option>
      {#if entries === undefined}
        <option value="chord">{t("inspector.kind.chord")}</option>
        <option value="exec">{t("inspector.kind.exec")}</option>
        <option value="launch">{t("inspector.kind.launch")}</option>
      {:else}
        {#each GROUPS as group (group.id)}
          <optgroup label={t(group.label)}>
            {#each entries.filter((entry) => entry.group === group.id) as entry (entry.key)}
              <option value={entry.kind === "tool" ? entry.id : entry.key}>{entry.name}</option>
            {/each}
          </optgroup>
        {/each}
      {/if}
    </select>
    {@render browse?.()}
  </span>
</div>
{#if part.actionType !== "none"}
  <div class="field">
    <label>
      {actionLabel}
      <input
        type="text"
        aria-label={actionLabel}
        placeholder={part.actionType === "chord"
          ? t("editor.chordExample")
          : t("editor.commandExample")}
        aria-invalid={invalid !== undefined ? true : undefined}
        aria-describedby={describedBy}
        value={part.action}
        oninput={(event) => onedit({ ...part, action: event.currentTarget.value })}
        onchange={oncommit}
      />
    </label>
    {#if invalid !== undefined}
      <p id={ids.action} role="alert" class="invalid">{invalid}</p>
    {/if}
    {#if part.actionType === "chord"}
      <p id={ids.hint} class="hint">{t("editor.chordHint")}</p>
    {/if}
  </div>
{/if}
{#if part.actionType === "launch"}
  <div class="field">
    <label>
      {named(t("inspector.windowClass"))}
      <input
        type="text"
        aria-label={named(t("inspector.windowClass"))}
        aria-describedby={ids.classHint}
        value={part.windowClass}
        oninput={(event) => onedit({ ...part, windowClass: event.currentTarget.value })}
        onchange={oncommit}
      />
    </label>
    <p id={ids.classHint} class="hint">{t("inspector.windowClassHint")}</p>
  </div>
  <label class="switch">
    <input
      type="checkbox"
      role="switch"
      aria-label={named(t("inspector.waitFocus"))}
      checked={part.waitFocus}
      onchange={(event) => {
        onedit({ ...part, waitFocus: event.currentTarget.checked });
        oncommit();
      }}
    />
    {named(t("inspector.waitFocus"))}
  </label>
{/if}

<style>
  /* The Inspector's field vocabulary (DESIGN.md "Inputs / Fields"): the
     label above its field, muted at 0.85 em, 2 px between them. */
  label,
  .field {
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
    font-size: 0.85em;
  }

  /* A label inside a field is sized once: the field already carries the
     label size, and nesting the two shrank the Command label to 9.6 px. */
  .field label {
    font-size: 1em;
  }

  /* The syntax under a field, in the label size of its field. */
  .hint {
    margin: 0;
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
    font-size: 1em;
  }

  .choice {
    display: flex;
    gap: 8px;
  }

  /* The switch sits before its label on one line. */
  label.switch {
    flex-direction: row;
    align-items: center;
    gap: 8px;
  }

  .choice select {
    flex: 1;
    min-width: 0;
  }

  .choice :global(button) {
    flex: none;
  }

  .invalid {
    margin: 0;
    background: var(--kv-bg);
    color: var(--kv-fg);
    overflow-wrap: anywhere;
  }
</style>
