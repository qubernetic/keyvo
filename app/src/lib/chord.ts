// The chord check of the key editor: the grammar of keyvo_core::chord
// (spec §7.2), ported token for token, so a chord the daemon would refuse
// at its reload is refused before the write and the wrong token named.
// Steps separated by `,`; a step is modifiers joined with `+` and one key
// last; names are case-insensitive with spaces around them; where a key is
// expected, `+` and `,` are the keys themselves. The evdev names come from
// keynames.ts, which tools/keycodes/generate.py writes with the Rust table.
// The tests run both against tests/protocol-fixtures/chord/.
import { KEY_NAMES } from "./keynames";

/** Why a chord does not parse, as keyvo_core::chord::ChordError. */
export type ChordError =
  | { error: "empty" }
  | { error: "missing_key" }
  | { error: "unknown_key"; token: string }
  | { error: "not_a_modifier"; token: string }
  | { error: "expected_separator"; token: string };

const MODIFIERS = new Set(["ctrl", "control", "shift", "alt", "altgr", "super", "meta"]);

/** The friendly names of keyvo_core::chord, by the evdev name they press. */
const ALIASES: Record<string, string> = {
  escape: "esc",
  return: "enter",
  del: "delete",
  ins: "insert",
  pgup: "pageup",
  pgdn: "pagedown",
  period: "dot",
  quote: "apostrophe",
  backtick: "grave",
};

/** The symbols of the US layout a key may be written as. */
const SYMBOLS = new Set("-=[]\\;'`,./!@#$%^&*()_+{}|:\"~<>?");

const NAMES = new Set(KEY_NAMES);

function isModifier(name: string): boolean {
  return MODIFIERS.has(name.toLowerCase());
}

function isKey(name: string): boolean {
  if (isModifier(name)) return true;
  if ([...name].length === 1 && SYMBOLS.has(name)) return true;
  let evdev = name.toLowerCase();
  evdev = ALIASES[evdev] ?? evdev;
  if (evdev.startsWith("key_")) evdev = evdev.slice("key_".length);
  return NAMES.has(evdev);
}

/** The next name in `text` and what follows it; a `+` or `,` where a name
 * starts is the name itself. */
function nextName(text: string): { name: string; after: string } | undefined {
  const rest = text.trimStart();
  const first = rest[0];
  if (first === undefined) return undefined;
  if (first === "+" || first === ",") return { name: first, after: rest.slice(1).trimStart() };
  const end = rest.search(/[+,]/);
  const cut = end === -1 ? rest.length : end;
  return { name: rest.slice(0, cut).trimEnd(), after: rest.slice(cut) };
}

/** The first problem of one step's names: modifiers, then the key. */
function stepError(names: string[]): ChordError | undefined {
  const key = names[names.length - 1];
  if (key === undefined) return { error: "missing_key" };
  const wrong = names.slice(0, -1).find((name) => !isModifier(name));
  if (wrong !== undefined) return { error: "not_a_modifier", token: wrong };
  return isKey(key) ? undefined : { error: "unknown_key", token: key };
}

/** Why `text` is no chord the daemon accepts; undefined when it is one. */
export function checkChord(text: string): ChordError | undefined {
  if (text.trim() === "") return { error: "empty" };
  let names: string[] = [];
  let rest = text;
  for (;;) {
    const next = nextName(rest);
    if (next === undefined) return { error: "missing_key" };
    names.push(next.name);
    const separator = next.after[0];
    if (separator === undefined) return stepError(names);
    if (separator === "+") {
      rest = next.after.slice(1);
    } else if (separator === ",") {
      const wrong = stepError(names);
      if (wrong !== undefined) return wrong;
      names = [];
      rest = next.after.slice(1);
    } else {
      return { error: "expected_separator", token: next.name };
    }
  }
}
