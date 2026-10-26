// Test helper: the shared socket fixtures in tests/protocol-fixtures/socket/
// as parsed messages, and the key editor steps in
// tests/protocol-fixtures/editor/ (keys and the profile header). Only the
// tests import it.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import type { KeyFields } from "./editor";
import type { ProfileFields } from "./profiles";
import { parseServerMessage, type Event, type Reply } from "./protocol";

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
  steps: EditorStep[];
}

export function editorSteps(): EditorSteps {
  return JSON.parse(readFileSync(join(EDITOR, "steps.json"), "utf8")) as EditorSteps;
}

/** One step of tests/protocol-fixtures/editor/profile.json. */
export interface ProfileStep {
  /** The header field the step changes. */
  field: "name" | "accent";
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

/** The text of an editor fixture file. */
export function editorFile(name: string): string {
  return readFileSync(join(EDITOR, name), "utf8");
}

/** The result of every ok line of responses.ndjson, in file order. */
export function responses(): Reply[] {
  return messages("responses.ndjson").flatMap((message) =>
    message.kind === "response" && message.response.ok ? [message.response.result] : [],
  );
}
