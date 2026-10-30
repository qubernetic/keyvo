// Test helper: the shared socket fixtures in tests/protocol-fixtures/socket/
// as parsed messages, and the key editor steps in
// tests/protocol-fixtures/editor/ (keys, the profile header, the page edits
// and the page list). Only the tests import it.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import type { Editor, KeyFields } from "./editor";
import type { PageEdit, ProfileFields } from "./profiles";
import { parseServerMessage, type Event, type Recipes, type Reply } from "./protocol";

const FIXTURES = join(
  dirname(fileURLToPath(import.meta.url)),
  "../../../tests/protocol-fixtures/socket",
);

function messages(name: string) {
  return readFileSync(join(FIXTURES, name), "utf8")
    .split("\n")
    .filter((line) => line.trim() !== "")
    .map(parseServerMessage);
}

/** Every line of events.ndjson, in file order. */
export function events(): Event[] {
  return messages("events.ndjson").flatMap((message) =>
    message.kind === "event" ? [message.event] : [],
  );
}

/** The events of class `name` in events.ndjson, in file order. */
export function eventsOf<E extends Event["event"]>(name: E): (Event & { event: E })[] {
  return events().filter((event): event is Event & { event: E } => event.event === name);
}

/** `list[index]`, which the test knows is there. */
export function nth<T>(list: readonly T[], index: number): T {
  const item = list[index];
  if (item === undefined) {
    throw new Error(`the fixture has no item ${index}`);
  }
  return item;
}

const EDITOR = join(dirname(fileURLToPath(import.meta.url)), "../../../tests/protocol-fixtures/editor");

/** One step of tests/protocol-fixtures/editor/steps.json. */
export interface EditorStep {
  /** The form field the step changes. */
  field: "label" | "action";
  fields: KeyFields;
  /** The file the step gives. */
  file: string;
  /** The line of the key's table in that file, from 1. */
  line: number;
}

/** tests/protocol-fixtures/editor/steps.json. */
export interface EditorSteps {
  profile: string;
  page: number;
  key: number;
  /** The file before the steps. */
  file: string;
  /** The file's path inside the configuration directory. */
  path: string;
  /** The key's fields before the steps. */
  fields: KeyFields;
  /** The line of the key's table before the steps, from 1. */
  line: number;
  steps: EditorStep[];
}

export function editorSteps(): EditorSteps {
  return JSON.parse(readFileSync(join(EDITOR, "steps.json"), "utf8")) as EditorSteps;
}

/** One step of tests/protocol-fixtures/editor/profile.json. */
export interface ProfileStep {
  /** The header field the step changes. */
  field: "name" | "accent" | "global_pages";
  fields: ProfileFields;
  /** The file the step gives. */
  file: string;
}

/** tests/protocol-fixtures/editor/profile.json. */
export interface ProfileSteps {
  profile: string;
  /** The file before the steps. */
  file: string;
  /** The file's path inside the configuration directory. */
  path: string;
  /** The header's fields before the steps. */
  fields: ProfileFields;
  steps: ProfileStep[];
}

export function profileSteps(): ProfileSteps {
  return JSON.parse(readFileSync(join(EDITOR, "profile.json"), "utf8")) as ProfileSteps;
}

/** A profile of a case of tests/protocol-fixtures/editor/page-list.json. */
export interface PageListProfile {
  /** `global` for the global profile itself; absent in a case's `global`. */
  id?: string;
  pages?: (string | null)[];
  /** The disabled pages, from 1. */
  disabled?: number[];
  /** Absent for true. */
  global_pages?: boolean;
}

/** One case of tests/protocol-fixtures/editor/page-list.json, which
 * keyvo-core's PageList runs too. */
export interface PageListCase {
  name: string;
  global: PageListProfile;
  owner: PageListProfile;
  list: { index: number; profile: string; page: number }[];
}

export function pageListCases(): PageListCase[] {
  const path = join(EDITOR, "page-list.json");
  return (JSON.parse(readFileSync(path, "utf8")) as { cases: PageListCase[] }).cases;
}

/** One step of tests/protocol-fixtures/editor/pages.json. */
export interface PageStep {
  edit: PageEdit;
  /** The file the step gives. */
  file: string;
  /** The edited page's number after the step; absent for a delete. */
  page?: number;
  /** The edit that undoes the step; absent for a delete. */
  undo?: PageEdit;
}

/** tests/protocol-fixtures/editor/pages.json: page edits applied one after
 * the other, and the edits the editor refuses. */
export interface PageSteps {
  profile: string;
  /** The file before the steps. */
  file: string;
  /** The file's path inside the configuration directory. */
  path: string;
  steps: PageStep[];
  refused: { edit: PageEdit; reason: "context" | "only" }[];
}

export function pageSteps(): PageSteps {
  return JSON.parse(readFileSync(join(EDITOR, "pages.json"), "utf8")) as PageSteps;
}

/** tests/protocol-fixtures/editor/recipe.json: variant `variant` of recipe
 * `recipe` applied to a key, as keyvo_client::Editor::apply_recipe writes
 * it. */
export interface RecipeStep {
  profile: string;
  page: number;
  key: number;
  /** The file before and the file's path inside the configuration directory. */
  file: string;
  path: string;
  recipe: string;
  variant: string;
  before: KeyFields;
  fields: KeyFields;
  /** The file the step gives and the line of the key's table in it. */
  expected: string;
  line: number;
}

export function recipeStep(): RecipeStep {
  return JSON.parse(readFileSync(join(EDITOR, "recipe.json"), "utf8")) as RecipeStep;
}

/** The text of an editor fixture file. */
export function editorFile(name: string): string {
  return readFileSync(join(EDITOR, name), "utf8");
}

/** The answers of recipes.ndjson after its request line: the packs of
 * contrib/recipes/ with the user pack `screenshot`, then an answer naming
 * a broken user pack in `problems`. */
export function recipeReplies(): Recipes[] {
  const [, ...answers] = readFileSync(join(FIXTURES, "recipes.ndjson"), "utf8")
    .split("\n")
    .filter((line) => line.trim() !== "");
  return answers.flatMap((line) => {
    const message = parseServerMessage(line);
    return message.kind === "response" && message.response.ok
      ? [message.response.result as Recipes]
      : [];
  });
}

/** The result of every ok line of responses.ndjson, in file order. */
export function responses(): Reply[] {
  return messages("responses.ndjson").flatMap((message) =>
    message.kind === "response" && message.response.ok ? [message.response.result] : [],
  );
}

/** The Dialpad half of an Editor for a test of keys: no profile binds a
 * control, and a control write is refused. */
export const UNBOUND_CONTROLS: Pick<Editor, "control" | "editControl"> = {
  control: ({ profile }) =>
    Promise.resolve({ path: `/home/u/.config/keyvo/profiles/${profile}.toml`, fields: {} }),
  editControl: () => Promise.reject(new Error("no control is written in this test")),
};

/** One bad chord of tests/protocol-fixtures/chord/chords.json: the error
 * as keyvo_core::chord::ChordError names it, and the token it names. */
export interface BadChord {
  chord: string;
  error: "empty" | "missing_key" | "unknown_key" | "not_a_modifier" | "expected_separator";
  token?: string;
}

/** tests/protocol-fixtures/chord/chords.json, which keyvo-core's chord
 * tests run too. */
export function chordCases(): { valid: string[]; invalid: BadChord[] } {
  const path = join(dirname(fileURLToPath(import.meta.url)), "../../../tests/protocol-fixtures/chord/chords.json");
  return JSON.parse(readFileSync(path, "utf8")) as { valid: string[]; invalid: BadChord[] };
}
