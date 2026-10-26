// The mock's key editor: the steps of tests/protocol-fixtures/editor/steps.json
// replayed against the in-memory profile the way keyvo_client::Editor
// answers them.
import { describe, expect, it } from "vitest";

import { editorFile, editorSteps } from "../fixtures";
import { CONFIG_DIR, contribProfiles } from "./data";
import { mockEditor } from "./editor";
import { Store } from "./store";

describe("the mock key editor", () => {
  it("answers each key step with the edited view of the fixture", async () => {
    const steps = editorSteps();
    const editor = mockEditor(new Store([editorFile(steps.file)]));
    const target = { profile: steps.profile, page: steps.page, key: steps.key };
    const path = `${CONFIG_DIR}/${steps.path}`;
    expect(await editor.key(target)).toEqual({ path, fields: steps.fields });
    let before = steps.fields;
    for (const step of steps.steps) {
      expect(await editor.editKey(target, step.fields)).toEqual({
        path,
        before,
        fields: step.fields,
      });
      expect(await editor.key(target)).toEqual({ path, fields: step.fields });
      before = step.fields;
    }
  });

  it("shows a key the page does not define as no fields, and writes it", async () => {
    const editor = mockEditor(new Store(contribProfiles()));
    const target = { profile: "konsole", page: 1, key: 3 };
    expect((await editor.key(target)).fields).toEqual({});
    await editor.editKey(target, { label: "New" });
    expect((await editor.key(target)).fields).toEqual({ label: "New" });
  });

  it("rejects an unknown profile, page or key with a Problem", async () => {
    const editor = mockEditor(new Store(contribProfiles()));
    await expect(editor.key({ profile: "nope", page: 1, key: 1 })).rejects.toEqual({
      message: "no profile `nope`",
    });
    await expect(editor.key({ profile: "global", page: 4, key: 1 })).rejects.toEqual({
      message: "profile `global` has no page 4",
    });
    await expect(editor.editKey({ profile: "global", page: 1, key: 10 }, {})).rejects.toEqual({
      message: "key 10 is not a key of the Keypad (1 to 9)",
    });
  });
});
