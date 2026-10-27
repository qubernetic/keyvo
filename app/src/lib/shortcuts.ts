// The keyboard map (pad screen brief §7; gui-ux.md §6, d15): the rows of
// the "Keyboard shortcuts" drawer. Only the shortcuts that work today are
// listed plainly; one still to come carries its roadmap sentence.
import type { MessageKey } from "./i18n";

export interface Shortcut {
  keys: MessageKey;
  action: MessageKey;
  /** For a shortcut not built yet: the sentence naming its roadmap item. */
  soon?: MessageKey;
}

export const SHORTCUTS: readonly Shortcut[] = [
  { keys: "shortcuts.keys.screens", action: "shortcuts.screens" },
  { keys: "shortcuts.keys.select", action: "shortcuts.select" },
  { keys: "shortcuts.keys.arrows", action: "shortcuts.move" },
  { keys: "shortcuts.keys.space", action: "shortcuts.press" },
  { keys: "shortcuts.keys.shiftSpace", action: "shortcuts.long" },
  { keys: "shortcuts.keys.undo", action: "shortcuts.undo" },
  { keys: "shortcuts.keys.redo", action: "shortcuts.redo" },
  { keys: "shortcuts.keys.search", action: "shortcuts.search" },
  { keys: "shortcuts.keys.escape", action: "shortcuts.dismiss" },
  { keys: "shortcuts.keys.cut", action: "shortcuts.cut", soon: "soon.move" },
  { keys: "shortcuts.keys.paste", action: "shortcuts.paste", soon: "soon.move" },
];
