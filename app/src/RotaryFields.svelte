<script lang="ts">
  // The rotary part of an editor form: the step (detents per run, empty for
  // 1) with its hint, and the two directions, each the action fields of
  // ActionFields.svelte. A Dialpad rotary in the control editor (#188), and
  // a modifier key's dial and roller in the Inspector (#189), where `name`
  // tells the two rotaries' fields apart. The fields are the caller's: each
  // change goes to a callback, a finished one calls `oncommit`, and the
  // direction the focus enters goes to `onfocus`, for the catalogue.
  import type { Snippet } from "svelte";

  import ActionFields from "./ActionFields.svelte";
  import type { CatalogueEntry } from "./lib/catalogue";
  import type { ActionPart, ControlForm } from "./lib/editor";
  import { t } from "./lib/i18n";

  type Direction = "cw" | "ccw";

  let {
    form,
    name,
    invalid,
    entries,
    browse,
    onstep,
    onedit,
    onswitch,
    oncommit,
    onfocus,
  }: {
    form: ControlForm;
    /** The rotary's name before each field's, where two rotaries share a
     * form; none in the control editor, which edits one. */
    name?: string;
    /** Why a field cannot be written yet, and which. */
    invalid?: { message: string; field: Direction | "step" };
    /** The catalogue's entries for the Action selects, while its column is
     * collapsed. */
    entries?: readonly CatalogueEntry[];
    /** Beside the clockwise Action select: the Pad's Browse actions button. */
    browse?: Snippet;
    /** The step as typed. */
    onstep: (step: string) => void;
    /** A field of `direction` changed: its part as it reads now. */
    onedit: (direction: Direction, part: ActionPart) => void;
    /** The select of `direction` chose `value`. */
    onswitch: (direction: Direction, value: string) => void;
    /** A field was finished: write the form. */
    oncommit: () => void;
    /** The focus entered the fields of `direction`. */
    onfocus?: (direction: Direction) => void;
  } = $props();

  const uid = $props.id();
  const ids = { step: `kv-rotary-${uid}-step`, stepHint: `kv-rotary-${uid}-step-hint` };

  /** A field's name, after the rotary's when there is one. */
  function named(field: string): string {
    return name === undefined ? field : t("inspector.fieldOf", { direction: name, field });
  }

  /** The directions, each with the label of its Action select. */
  const directions = $derived(
    (["cw", "ccw"] as const).map((part) => ({
      part,
      label: named(t(part === "cw" ? "inspector.clockwise" : "inspector.counterClockwise")),
    })),
  );
  const stepLabel = $derived(named(t("inspector.step")));
</script>

<div class="field">
  <label for={ids.step}>{stepLabel}</label>
  <input
    id={ids.step}
    type="text"
    inputmode="numeric"
    aria-label={stepLabel}
    placeholder={t("editor.stepExample")}
    aria-invalid={invalid?.field === "step" ? true : undefined}
    aria-describedby={ids.stepHint}
    value={form.step}
    oninput={(event) => onstep(event.currentTarget.value)}
    onchange={oncommit}
  />
  {#if invalid?.field === "step"}
    <p role="alert" class="invalid">{invalid.message}</p>
  {/if}
  <p id={ids.stepHint} class="hint">{t("inspector.stepHint")}</p>
</div>
{#each directions as { part, label } (part)}
  <div class="part" onfocusin={() => onfocus?.(part)}>
    <ActionFields
      part={form[part]}
      {label}
      direction={label}
      invalid={invalid?.field === part ? invalid.message : undefined}
      {entries}
      browse={part === "cw" ? browse : undefined}
      onedit={(next) => onedit(part, next)}
      onswitch={(value) => onswitch(part, value)}
      {oncommit}
    />
  </div>
{/each}

<style>
  .part {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: 48ch;
  }

  /* The Inspector's field vocabulary: the label above its field, muted at
     0.85 em, 2 px between them; a label inside a field sized once. */
  .field {
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
    font-size: 0.85em;
  }

  .field label {
    font-size: 1em;
  }

  .hint {
    margin: 0;
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
    font-size: 1em;
  }

  .invalid {
    margin: 0;
    background: var(--kv-bg);
    color: var(--kv-fg);
    overflow-wrap: anywhere;
  }
</style>
