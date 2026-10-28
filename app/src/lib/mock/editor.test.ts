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
    expect(await editor.key(target)).toEqual({ path, fields: steps.fields, line: steps.line });
    let before = steps.fields;
    for (const step of steps.steps) {
      expect(await editor.editKey(target, step.fields)).toEqual({
        path,
        before,
        fields: step.fields,
        line: step.line,
      });
      expect(await editor.key(target)).toEqual({ path, fields: step.fields, line: step.line });
      before = step.fields;
    }
  });

  it("shows the error key's on_fail and keeps it through a write", async () => {
    const editor = mockEditor(new Store(contribProfiles()));
    const target = { profile: "global", page: 1, key: 2 };
    const onFail = { type: "exec", cmd: "notify-send 'keyvo' 'key 2 failed'" };
    const view = await editor.key(target);
    expect(view.on_fail).toEqual(onFail);
    expect(view.fields).not.toHaveProperty("on_fail");
    await editor.editKey(target, { ...view.fields, label: "Broken" });
    expect(await editor.key(target)).toMatchObject({ fields: { label: "Broken" }, on_fail: onFail });
    expect((await editor.key({ ...target, key: 1 })).on_fail).toBeUndefined();
  });

  it("answers the line of a key's header on its own page", async () => {
    const steps = editorSteps();
    const text = `${editorFile(steps.file)}\n[[pages]]\nname = "two"\n\n[pages.keys.2]\nlabel = "Two"\n`;
    const editor = mockEditor(new Store([text]));
    expect((await editor.key({ profile: steps.profile, page: 1, key: 1 })).line).toBe(15);
    expect((await editor.key({ profile: steps.profile, page: 2, key: 2 })).line).toBe(29);
  });

  it("shows a key the page does not define as no fields and no line, and writes it", async () => {
    const editor = mockEditor(new Store(contribProfiles()));
    const target = { profile: "konsole", page: 1, key: 3 };
    expect(await editor.key(target)).not.toHaveProperty("line");
    expect((await editor.key(target)).fields).toEqual({});
    await editor.editKey(target, { label: "New" });
    expect((await editor.key(target)).fields).toEqual({ label: "New" });
  });

  it("refuses every write with a placed Problem when told to, and leaves the key", async () => {
    const editor = mockEditor(new Store(contribProfiles()), { refuseWrites: true });
    const target = { profile: "konsole", page: 1, key: 1 };
    const before = await editor.key(target);
    const refused = editor.editKey(target, { label: "New" });
    await expect(refused).rejects.toEqual({
      message: "the mock refuses every write (?mock-error=write)",
      path: before.path,
      line: before.line,
      column: 1,
    });
    expect(await editor.key(target)).toEqual(before);
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
