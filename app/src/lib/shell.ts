// The shell's rules (docs/design/gui-ux.md §2, §5.2, §6; pad screen brief
// §5.1): the rail's screens in order with their shortcuts, the widths at
// which the columns collapse, the header's overflow menu and Preferences
// tabs, and what the header's device selector offers.
import type { MessageKey } from "./i18n";
import type { IconName } from "./icons";
import type { DeviceStatus } from "./protocol";

/** The Pad keeps the screen id `home`. */
export type ScreenId = "home" | "profiles" | "behaviors";

export interface Screen {
  id: ScreenId;
  label: MessageKey;
  icon: IconName;
  /** For a screen not built yet: the sentence naming its roadmap item. */
  soon?: MessageKey;
}

/** The rail, top to bottom; `Ctrl+1` is the first. */
export const SCREENS: readonly Screen[] = [
  { id: "home", label: "rail.pad", icon: "keyboard" },
  { id: "profiles", label: "rail.profiles", icon: "layers" },
  { id: "behaviors", label: "rail.behaviors", icon: "workflow", soon: "soon.behaviors" },
];

/** The index into SCREENS that `Ctrl+1` to `Ctrl+3` names, if `event` is one. */
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

export type MenuId = "export" | "import" | "config" | "setup" | "shortcuts" | "about";

/** The header's overflow menu (§2); an entry is enabled once it exists.
 * In the pre-alpha only the keyboard map does (export and import are M1
 * CLI commands, the others come with the screens of M4a). */
export const MENU: readonly { id: MenuId; label: MessageKey; enabled: boolean }[] = [
  { id: "export", label: "menu.export", enabled: false },
  { id: "import", label: "menu.import", enabled: false },
  { id: "config", label: "menu.config", enabled: false },
  { id: "setup", label: "menu.setup", enabled: false },
  { id: "shortcuts", label: "menu.shortcuts", enabled: true },
  { id: "about", label: "menu.about", enabled: false },
];

export type PreferenceId = "general" | "devices" | "integrations" | "projects";

/** The tabs of Preferences, which the header's gear opens (brief §5.1);
 * none is built yet, so each names its roadmap item. */
export const PREFERENCES: readonly { id: PreferenceId; label: MessageKey; soon: MessageKey }[] = [
  { id: "general", label: "preferences.general", soon: "soon.settings" },
  { id: "devices", label: "preferences.devices", soon: "soon.devices" },
  { id: "integrations", label: "preferences.integrations", soon: "soon.integrations" },
  { id: "projects", label: "preferences.projects", soon: "soon.projects" },
];

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
