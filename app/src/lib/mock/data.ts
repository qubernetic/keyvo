// The mock daemon's data (docs/plans/2026-10-26-gui-design-pass.md, slice
// 2), read in place from the repository: the socket fixtures of
// tests/protocol-fixtures/socket/, the example profiles of contrib/profiles/
// and the renderer's PNG snapshots of crates/keyvo-core/tests/snapshots/.
// vite.config.ts lets the dev server read these directories in mock mode;
// nothing is copied into app/.
import globalToml from "../../../../contrib/profiles/global.toml?raw";
import konsoleToml from "../../../../contrib/profiles/konsole.toml?raw";
import vscodeToml from "../../../../contrib/profiles/vscode.toml?raw";
import backgroundPng from "../../../../crates/keyvo-core/tests/snapshots/render__background.snap.png?url";
import iconLabelPng from "../../../../crates/keyvo-core/tests/snapshots/render__icon-label.snap.png?url";
import iconPng from "../../../../crates/keyvo-core/tests/snapshots/render__icon.snap.png?url";
import labelPng from "../../../../crates/keyvo-core/tests/snapshots/render__label.snap.png?url";
import eventsNdjson from "../../../../tests/protocol-fixtures/socket/events.ndjson?raw";
import recipesNdjson from "../../../../tests/protocol-fixtures/socket/recipes.ndjson?raw";
import responsesNdjson from "../../../../tests/protocol-fixtures/socket/responses.ndjson?raw";
import {
  parseServerMessage,
  type Event,
  type Recipes,
  type Status,
  type TraceRecord,
} from "../protocol";

function messages(text: string) {
  return text
    .split("\n")
    .filter((line) => line.trim() !== "")
    .map(parseServerMessage);
}

function isStatus(result: object): result is Status {
  return "device" in result && "config_dir" in result;
}

/** The fixture's `status` reply with a connected Keypad and its geometry. */
export function fixtureStatus(): Status {
  for (const message of messages(responsesNdjson)) {
    if (message.kind !== "response" || !message.response.ok) continue;
    const result = message.response.result;
    if (isStatus(result) && result.device.connected) return structuredClone(result);
  }
  throw new Error("responses.ndjson has no connected status reply");
}

function isRecipes(result: object): result is Recipes {
  return "recipes" in result;
}

/** The first `recipes` answer of recipes.ndjson: the packs of
 * contrib/recipes/ as the daemon lists them (its socket test writes the
 * line from the files), with one user pack. */
export function fixtureRecipes(): Recipes {
  const [, ...answers] = recipesNdjson.split("\n").filter((line) => line.trim() !== "");
  for (const message of messages(answers.join("\n"))) {
    if (message.kind !== "response" || !message.response.ok) continue;
    const result = message.response.result;
    if (isRecipes(result)) return structuredClone(result);
  }
  throw new Error("recipes.ndjson has no recipes reply");
}

/** The configuration directory of the fixture's status reply; the mock's
 * profile paths lie under it. */
export const CONFIG_DIR = fixtureStatus().config_dir;

/** The `trace` entries of events.ndjson, in file order. */
export function fixtureTrace(): TraceRecord[] {
  return messages(eventsNdjson).flatMap((message) =>
    message.kind === "event" && message.event.event === "trace"
      ? [(message.event as Event & { event: "trace" }).entry]
      : [],
  );
}

/** The text of each profile of contrib/profiles/. */
export function contribProfiles(): string[] {
  return [globalToml, konsoleToml, vscodeToml];
}

/** A mock-only profile with no pages (`?mock-profiles=empty`), for the
 * Pad's "profile with no pages" state. */
export const EMPTY_PROFILE = `schema_version = 1

[profile]
id    = "empty"
name  = "Empty"
match = { class = ["org.example.empty"] }
`;

/** The snapshot a key's tile stands in for, by what the key shows. */
export const SNAPSHOTS = {
  background: backgroundPng,
  icon: iconPng,
  label: labelPng,
  iconLabel: iconLabelPng,
} as const;
