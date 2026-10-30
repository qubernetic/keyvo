<script lang="ts">
  // The Dialpad panel under the mirror on the Pad (#188, docs/design/
  // gui-ux.md §3.3): the owner's drawing inline, coloured from --kv-device
  // like the Keypad's, with the four buttons, the roller and the dial as
  // selectable controls at their shapes, each showing its label (the GUI's
  // own; the Dialpad has no LCD). The labels are the shown profile's
  // `[dialpad]` controls, a control it does not bind falling back to the
  // global profile's of the same name (dimmed, as a fall-through key is on
  // the mirror). The panel collapses on its heading; it starts collapsed
  // while no Dialpad is connected and neither profile binds a control, and
  // says "No Dialpad connected" while none is, open or not. A control the
  // device turned or pressed pulses for PULSE_MS. While a modifier key has
  // the dial (#189), the dial and the roller it binds show the key's
  // binding instead, labelled "(held)" or "(toggled)"; a rotary the key
  // leaves out keeps the profile's.
  import { untrack } from "svelte";

  import dialpadSvg from "./assets/device/dialpad.svg?raw";
  import { controlPlaces, DEVICE_COLOURS, drawing, inline } from "./lib/device";
  import {
    binding,
    bound,
    CONTROLS,
    controlName,
    isRoller,
    modifierBinding,
    PULSE_MS,
    type Binding,
  } from "./lib/dialpad";
  import { problemOf, describe, type Editor, type KeyFields, type Target } from "./lib/editor";
  import { t } from "./lib/i18n";
  import { describeAction, phrase } from "./lib/pad";
  import { GLOBAL } from "./lib/profiles";
  import type { Control, DialpadStatus, ModifierMode } from "./lib/protocol";

  let {
    editor,
    status,
    profile,
    selected,
    pulse,
    modifier,
    revision = 0,
    onselect,
  }: {
    editor: Editor;
    /** The Dialpad the daemon reports; none reads as not connected. */
    status?: DialpadStatus;
    /** The profile whose controls the panel shows; none until the pad
     * reports one. */
    profile?: string;
    /** The selected control; none while a key or nothing is selected. */
    selected?: Control;
    /** The control the device last turned or pressed; a new `seq` pulses it. */
    pulse?: { control: Control; seq: number };
    /** The modifier key that has the dial, as the pad resolves it: the
     * view's page, or the global context page under a key it lacks; none
     * while no key has the dial. */
    modifier?: { key: Target; mode: ModifierMode };
    /** Bumped when the files may have changed: a reload or a write. */
    revision?: number;
    onselect: (control: Control) => void;
  } = $props();

  const id = $props.id();
  const device = drawing(dialpadSvg);
  const places = controlPlaces(dialpadSvg);
  const markup = inline(dialpadSvg, id, t("device.dialpad"));
  const ids = { heading: `kv-dialpad-${id}`, body: `kv-dialpad-${id}-body` };

  let bindings: Map<Control, Binding> = $state.raw(new Map());
  /** Why the bindings could not be read. */
  let readError: string | undefined = $state();
  /** The user's own choice on the heading; none follows the default. */
  let expanded: boolean | undefined = $state();

  const connected = $derived(status?.connected === true);
  const anyBound = $derived([...bindings.values()].some((found) => bound(found.fields)));
  const open = $derived(expanded ?? (connected || anyBound));

  /** Bumped by every read, so a late answer of an earlier one is dropped. */
  let reads = 0;
  /** Derived, so a parent that hands the same profile or revision again
   * reads nothing. */
  const shownProfile = $derived(profile);
  const seenRevision = $derived(revision);

  // The controls of the shown profile and of the global profile under it,
  // again after every reload and write.
  $effect(() => {
    void seenRevision;
    const shown = shownProfile;
    const mine = ++reads;
    if (shown === undefined) {
      bindings = new Map();
      return;
    }
    const read = (owner: string, control: Control) =>
      untrack(() => editor.control({ profile: owner, control })).then((view) => view.fields);
    Promise.all(
      CONTROLS.map(async (control): Promise<[Control, Binding]> => {
        const [own, global] = await Promise.all([
          read(shown, control),
          shown === GLOBAL ? Promise.resolve(undefined) : read(GLOBAL, control),
        ]);
        return [control, binding(shown, own, global)];
      }),
    ).then(
      (found) => {
        if (mine !== reads) return;
        bindings = new Map(found);
        readError = undefined;
      },
      (error: unknown) => {
        if (mine === reads) readError = describe(problemOf(error));
      },
    );
  });

  /** The fields of the modifier key; none while no key has the dial. */
  let modifierFields: KeyFields | undefined = $state.raw();
  /** Derived, so the same key handed again reads nothing. */
  const modifierId = $derived(
    modifier === undefined
      ? ""
      : `${modifier.key.profile}\n${modifier.key.page}\n${modifier.key.key}`,
  );
  let modifierReads = 0;

  $effect(() => {
    void seenRevision;
    void modifierId;
    const at = untrack(() => modifier?.key);
    const mine = ++modifierReads;
    if (at === undefined) {
      modifierFields = undefined;
      return;
    }
    untrack(() => editor.key({ ...at })).then(
      (view) => {
        if (mine === modifierReads) modifierFields = view.fields;
      },
      () => {
        // The panel keeps the profile's bindings when the key cannot be read.
        if (mine === modifierReads) modifierFields = undefined;
      },
    );
  });

  /** What each control shows: the modifier key's binding of a rotary it
   * binds, else the profile's. */
  const shown = $derived.by(() => {
    if (modifier === undefined || modifierFields === undefined) return bindings;
    const fields = modifierFields;
    const { key, mode } = modifier;
    return new Map(
      CONTROLS.map((control): [Control, Binding] => [
        control,
        (isRoller(control) ? modifierBinding(fields, control, key.key, mode) : undefined) ??
          bindings.get(control) ?? { fields: {} },
      ]),
    );
  });

  /** The control pulsing now, and the pulse it shows. */
  let pulsing: { control: Control; seq: number } | undefined = $state();

  $effect(() => {
    const next = pulse;
    if (next === undefined) return;
    pulsing = next;
    const timer = setTimeout(() => {
      if (pulsing?.seq === next.seq) pulsing = undefined;
    }, PULSE_MS);
    return () => clearTimeout(timer);
  });

  /** What `control` does, in a few words, for its accessible name. */
  function does(control: Control, found: Binding | undefined): string {
    const fields = found?.fields ?? {};
    const unbound = t("home.unbound");
    if (!isRoller(control)) return describeAction(fields.action) ?? unbound;
    if (fields.cw === undefined && fields.ccw === undefined) return unbound;
    return t("dialpad.turns", {
      cw: describeAction(fields.cw) ?? unbound,
      ccw: describeAction(fields.ccw) ?? unbound,
    });
  }

  function label(control: Control): string {
    const found = shown.get(control);
    const parts = phrase([
      found?.fields.label,
      does(control, found),
      found?.layer === undefined ? undefined : t("layer", { layer: found.layer }),
      found?.modifier === undefined ? undefined : t("dialpad.fromKey", { key: found.modifier }),
    ]);
    return t("dialpad.control", { control: controlName(control), parts });
  }

  /** A global control under an app profile (spec §7.2). */
  function fallen(control: Control): boolean {
    return shown.get(control)?.layer === "global" && profile !== GLOBAL;
  }

  function placed(control: Control): string {
    const place = places.get(control);
    if (place === undefined) return "";
    return `left: ${place.left}%; top: ${place.top}%; width: ${place.width}%; height: ${place.height}%`;
  }

  /** Sets the derived colours, the pulse's length and the clear fill of
   * the controls on the root. */
  function colours(node: HTMLElement): void {
    for (const [name, value] of Object.entries(DEVICE_COLOURS)) {
      node.style.setProperty(name, value);
    }
    node.style.setProperty("--kv-dialpad-pulse", `${PULSE_MS}ms`);
    node.style.setProperty("--kv-dialpad-clear", "transparent");
  }
</script>

<section class="dialpad" aria-labelledby={ids.heading}>
  <h2 id={ids.heading}>
    <button
      type="button"
      class="toggle"
      aria-label={t("dialpad.title")}
      aria-expanded={open}
      aria-controls={ids.body}
      onclick={() => (expanded = !open)}
    >
      <span class="chevron" class:open aria-hidden="true"></span>
      {t("dialpad.title")}
    </button>
  </h2>
  {#if !connected}
    <p class="line">{t("dialpad.none")}</p>
  {:else if status?.slot !== undefined}
    <p class="line">{t("dialpad.slot", { slot: status.slot })}</p>
  {/if}
  {#if readError !== undefined}
    <p class="line">{t("dialpad.readError", { error: readError })}</p>
  {/if}
  {#if open}
    <div
      id={ids.body}
      class="device"
      style:max-width={`${device.width}px`}
      style:aspect-ratio={`${device.width} / ${device.height}`}
      use:colours
    >
      <!-- The drawing is the repository's own asset, its ids prefixed per instance. -->
      {@html markup}
      <div class="controls" role="group" aria-label={t("dialpad.controls")}>
        {#each CONTROLS as control (control)}
          <button
            type="button"
            class="control"
            class:round={places.get(control)?.round === true}
            class:selected={control === selected}
            class:fallen={fallen(control)}
            class:modifier={shown.get(control)?.modifier !== undefined}
            class:pulse={pulsing?.control === control}
            style={placed(control)}
            aria-label={label(control)}
            aria-pressed={control === selected}
            onclick={() => onselect(control)}
          >
            {#key pulsing?.control === control ? pulsing.seq : 0}
              <span class="ring" aria-hidden="true"></span>
            {/key}
            {#if shown.get(control)?.fields.label !== undefined}
              <span class="label">{shown.get(control)?.fields.label}</span>
            {/if}
          </button>
        {/each}
      </div>
    </div>
  {/if}
</section>

<style>
  .dialpad {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  h2 {
    margin: 0;
    font-size: 1em;
  }

  .toggle {
    display: flex;
    gap: 8px;
    align-items: center;
    border-color: transparent;
    background: var(--kv-bg);
    color: var(--kv-fg);
    font-weight: 600;
    text-align: start;
  }

  /* A filled triangle in the body colour: right while collapsed, down open. */
  .chevron {
    flex: none;
    width: 8px;
    height: 8px;
    background: var(--kv-fg);
    color: var(--kv-bg);
    clip-path: polygon(0 0, 100% 50%, 0 100%);
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .line {
    margin: 0;
    background: var(--kv-bg);
    color: var(--kv-fg-muted);
  }

  .device {
    position: relative;
    width: 100%;
  }

  .device :global(svg) {
    display: block;
    width: 100%;
    height: 100%;
  }

  .controls {
    position: absolute;
    inset: 0;
  }

  /* A control is part of the device: clear over its shape, the accent
     rim when selected, at least the 40 px of the Dialpad panel (§6). */
  .control {
    position: absolute;
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 40px;
    min-height: 40px;
    padding: 0;
    border: 2px solid transparent;
    border-radius: var(--kv-radius);
    background: var(--kv-dialpad-clear);
    color: var(--kv-fg);
    font: inherit;
    cursor: pointer;
  }

  .control.round,
  .control.round .ring {
    border-radius: 50%;
  }

  .control:hover {
    border-color: var(--kv-fg);
  }

  .control.selected {
    border-color: var(--kv-accent);
  }

  .ring {
    position: absolute;
    inset: 0;
    border-radius: var(--kv-radius);
    pointer-events: none;
  }

  /* The pulse of a turn or a press: an accent ring that fades out. */
  .control.pulse .ring {
    animation: kv-dialpad-pulse var(--kv-dialpad-pulse) ease-out;
  }

  @keyframes kv-dialpad-pulse {
    from {
      box-shadow: 0 0 0 6px var(--kv-accent);
    }

    to {
      box-shadow: 0 0 0 0 var(--kv-accent);
    }
  }

  /* The label sits on the shape like the mirror's layer mark. */
  .label {
    max-width: 100%;
    padding: 0 4px;
    overflow: hidden;
    border-radius: var(--kv-radius);
    background: var(--kv-bg-alt);
    color: var(--kv-fg);
    font-size: 0.75em;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .control.fallen .label {
    opacity: 0.7;
  }

  /* A rotary a modifier key has: its label in the accent ring of the key. */
  .control.modifier .label {
    box-shadow: 0 0 0 2px var(--kv-accent);
  }
</style>
