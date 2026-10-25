// Every string the app shows, by key (docs/design/gui-ux.md §6: English
// only in v1, through the i18n layer from the start). A placeholder is a
// name in braces that `t` fills; components and helpers never write user
// text themselves.

export const messages = {
  "app.name": "keyvo",

  "rail.label": "Screens",
  "rail.home": "Home",
  "rail.profiles": "Profiles",
  "rail.behaviors": "Behaviors",
  "rail.projects": "Projects",
  "rail.integrations": "Integrations",
  "rail.devices": "Devices",
  "rail.settings": "Settings",
  "soon": "Coming soon",
  "soon.behaviors": "Behaviors are not built yet; they arrive with M4a-5 on the roadmap.",
  "soon.projects": "Projects are not built yet; they arrive with M4a-6 on the roadmap.",
  "soon.integrations": "Integrations are not built yet; they arrive with M4a-8 on the roadmap.",
  "soon.devices": "Devices are not built yet; they arrive with M4a-8 on the roadmap.",
  "soon.settings": "Settings are not built yet; they arrive with M4a-8 on the roadmap.",

  "header.label": "keyvo header",
  "header.device": "Device",
  "header.keypad": "Keypad",
  "header.simulator": "Simulator",
  "header.menu": "More actions",
  "menu.export": "Export",
  "menu.import": "Import",
  "menu.config": "Open config folder",
  "menu.setup": "Re-run setup step",
  "menu.about": "About",

  "columns.list": "List",
  "columns.editor": "Editor",
  "columns.right": "Preview",
  "columns.showList": "Show the list column",
  "columns.hideList": "Hide the list column",
  "columns.showRight": "Show the right column",
  "columns.hideRight": "Hide the right column",

  "profiles.list": "The profile list is not built yet; it arrives with the Profiles screen.",
  "profiles.editor": "The profile editor is not built yet; it arrives with the Profiles screen.",
  "profiles.right": "The page preview is not built yet; it arrives with the Profiles screen.",

  "home.unreachable": "Daemon unreachable: {reason}",
  "home.retry": "Retry",
  "home.connecting": "Connecting to the daemon",
  "home.configError": "Configuration not loaded: {error}",
  "home.status": "Status",
  "home.device": "Device",
  "home.profile": "Profile",
  "home.page": "Page",
  "home.mode": "Mode",
  "home.context": "Context",
  "home.unknown": "unknown",
  "home.noWindow": "no window reported",
  "home.window": "{class}: {title}",
  "home.keypad": "Keypad connected",
  "home.keypadKeys": "Keypad connected, {keys} keys",
  "home.noKeypad": "No Keypad",
  "home.noKeypadError": "No Keypad: {error}",
  "home.noGeometry": "The mirror appears once a Keypad has reported its geometry.",
  "home.mirror": "Keypad mirror",
  "home.mirrorView": "Keypad mirror: profile {profile}, page {page}",
  "home.key": "Key {key}: {parts}",
  "home.unbound": "unbound",
  "home.globalMark": "global",
  "home.tilesError": "Tiles not rendered: {error}",
  "home.result": "Press result",
  "home.resultHint": "Click a key for a dry-run press, right click for a long one.",
  "home.pressFailed": "Press failed: {reason}",
  "home.events": "Recent events",
  "home.noEvents": "No presses yet.",

  "action.chord": "chord {keys}",
  "action.exec": "exec {cmd}",
  "layer": "{layer} layer",

  "trace.line": "{head}: {body}",
  "trace.key": "Key {key}",
  "trace.keyLong": "Key {key} long",
  "trace.simulated": "simulated",
  "trace.dryRun": "dry run, nothing executed",
  "trace.running": "running",
  "trace.exited": "exited {code} after {ms} ms",
  "trace.failed": "failed: {error}",

  "editor.title": "Key editor",
  "editor.key": "Key {key}",
  "editor.select": "Select a key on the mirror to edit it.",
  "editor.file": "File:",
  "editor.label": "Label",
  "editor.color": "Color",
  "editor.icon": "Icon",
  "editor.action": "Action",
  "editor.none": "none",
  "editor.chord": "chord",
  "editor.exec": "exec",
  "editor.keys": "Chord",
  "editor.command": "Command",
  "editor.colorExample": "#rrggbb",
  "editor.iconExample": "lucide:hammer",
  "editor.chordExample": "ctrl+shift+g",
  "editor.commandExample": "cargo build",
  "editor.undo": "Undo",
  "editor.redo": "Redo",
  "editor.result": "Editor result",
  "editor.wrote": "Wrote key {key} to {path}",
  "editor.undid": "Undid the change to key {key}",
  "editor.redid": "Redid the change to key {key}",
  "editor.badColor": "The color {color} is not written #rrggbb.",
  "editor.noKeys": "Enter the chord's keys to write it.",
  "editor.noCommand": "Enter the command to write it.",
} as const;

export type MessageKey = keyof typeof messages;

/** The string of `key` with each `{name}` replaced by `values[name]`;
 * throws when a placeholder has no value, so a missing one fails a test
 * instead of reaching the screen. */
export function t(key: MessageKey, values: Record<string, string | number> = {}): string {
  return messages[key].replace(/\{(\w+)\}/g, (_, name: string) => {
    const value = values[name];
    if (value === undefined) throw new Error(`${key} needs {${name}}`);
    return String(value);
  });
}
