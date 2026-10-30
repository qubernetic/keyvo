// The Dialpad on the Pad (#188, spec §7.2): its controls in the order the
// panel reads them, their names, and the binding the panel shows for each:
// the shown profile's own `[dialpad]` control, else the global profile's
// control of the same name, as the daemon resolves a turn or a press.
import type { ControlFields, KeyFields } from "./editor";
import { t } from "./i18n";
import { GLOBAL } from "./profiles";
import type { Control, Layer, ModifierMode, Roller } from "./protocol";

/** The controls in the drawing's reading order: the top buttons, the
 * roller, the dial, the bottom buttons. */
export const CONTROLS: readonly Control[] = [
  "button:1",
  "button:2",
  "roller",
  "dial",
  "button:3",
  "button:4",
];

/** How long a control pulses after the device turned or pressed it. */
export const PULSE_MS = 400;

export function isRoller(control: Control): control is Roller {
  return control === "dial" || control === "roller";
}

/** The control's name: Dial, Roller, Button N. */
export function controlName(control: Control): string {
  if (control === "dial") return t("dialpad.dial");
  if (control === "roller") return t("dialpad.roller");
  return t("dialpad.button", { button: control.slice("button:".length) });
}

/** Whether `fields` binds anything. */
export function bound(fields: ControlFields): boolean {
  return Object.values(fields).some((value) => value !== undefined);
}

/** A control as the panel shows it: its fields and the layer that
 * supplies them, none when no profile binds it; or the modifier key whose
 * binding it shows (#189). */
export interface Binding {
  fields: ControlFields;
  layer?: Layer;
  /** The modifier key that has the dial, when its binding is shown. */
  modifier?: number;
}

/** The rotary `roller` while key `key`, with the key fields `fields`, has
 * the dial in `mode` (#189): the key's binding, labelled with the rotary's
 * label, else the key's, else the key's number, and the mode; none for a
 * rotary the key leaves to the profile. */
export function modifierBinding(
  fields: KeyFields,
  roller: Roller,
  key: number,
  mode: ModifierMode,
): Binding | undefined {
  const rotary = fields[roller];
  if (rotary === undefined) return undefined;
  const name = rotary.label ?? fields.label ?? t("dialpad.key", { key });
  const label = t(mode === "held" ? "dialpad.held" : "dialpad.toggled", { label: name });
  return { fields: { ...rotary, label }, modifier: key };
}

/** The control of `profile` from its own fields `own`, else the global
 * profile's `global` (spec §7.2: an app profile's control falls back to
 * the global one of the same name). */
export function binding(
  profile: string,
  own: ControlFields | undefined,
  global: ControlFields | undefined,
): Binding {
  if (own !== undefined && bound(own)) {
    return { fields: own, layer: profile === GLOBAL ? "global" : "app" };
  }
  if (profile !== GLOBAL && global !== undefined && bound(global)) {
    return { fields: global, layer: "global" };
  }
  return { fields: {} };
}
