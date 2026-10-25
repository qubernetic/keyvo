// The shell's rules (docs/design/gui-ux.md §2, §5.2, §6): the rail's
// screens in order with their shortcuts, the widths at which the columns
// collapse, and what the header's device selector offers.
import type { MessageKey } from "./i18n";
import type { IconName } from "./icons";
import type { DeviceStatus } from "./protocol";

export type ScreenId =
  | "home"
  | "profiles"
  | "behaviors"
  | "projects"
  | "integrations"
  | "devices"
  | "settings";

export interface Screen {
  id: ScreenId;
  label: MessageKey;
  icon: IconName;
  /** For a screen not built yet: the sentence naming its roadmap item. */
  soon?: MessageKey;
}

/** The rail, top to bottom; `Ctrl+1` is the first. */
export const SCREENS: readonly Screen[] = [
  { id: "home", label: "rail.home", icon: "house" },
  { id: "profiles", label: "rail.profiles", icon: "layers" },
  { id: "behaviors", label: "rail.behaviors", icon: "workflow", soon: "soon.behaviors" },
  { id: "projects", label: "rail.projects", icon: "folder-git-2", soon: "soon.projects" },
  { id: "integrations", label: "rail.integrations", icon: "plug", soon: "soon.integrations" },
  { id: "devices", label: "rail.devices", icon: "keyboard", soon: "soon.devices" },
  { id: "settings", label: "rail.settings", icon: "settings", soon: "soon.settings" },
];

/** The index into SCREENS that `Ctrl+1` to `Ctrl+7` names, if `event` is one. */
export function shortcut(event: KeyboardEvent): number | undefined {
  if (!event.ctrlKey || event.altKey || event.shiftKey || event.metaKey) return undefined;
  if (!/^[1-9]$/.test(event.key)) return undefined;
  const index = Number(event.key) - 1;
  return index < SCREENS.length ? index : undefined;
}

// Design detail: the window opens at 1200 px with every column; the right
// column collapses first, then the rail drops its labels (§2: below
// 1100 px), then the list column collapses; at the 960 px minimum only the
// rail icons and the editor remain.
/** The narrowest window that shows the right column. */
export const RIGHT_FROM = 1200;
/** The narrowest window whose rail shows labels. */
export const RAIL_LABELS_FROM = 1100;
/** The narrowest window that shows the list column. */
export const LIST_FROM = 1000;

export interface Columns {
  railLabels: boolean;
  list: boolean;
  right: boolean;
}

/** What a window `width` px wide shows. */
export function layout(width: number): Columns {
  return {
    railLabels: width >= RAIL_LABELS_FROM,
    list: width >= LIST_FROM,
    right: width >= RIGHT_FROM,
  };
}

export type DeviceChoice = "keypad" | "simulator";

/** The header selector's entries: every device the daemon reports, then
 * the Simulator. */
export function deviceChoices(device: DeviceStatus | undefined): DeviceChoice[] {
  return device?.connected === true ? ["keypad", "simulator"] : ["simulator"];
}

/** The entry the selector shows: the user's choice while it is offered,
 * else the Keypad when connected (§5.2: the Simulator while it is missing). */
export function chosenDevice(
  device: DeviceStatus | undefined,
  chosen: DeviceChoice | undefined,
): DeviceChoice {
  const choices = deviceChoices(device);
  return chosen !== undefined && choices.includes(chosen) ? chosen : (choices[0] ?? "simulator");
}
