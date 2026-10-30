// The mock's profile files: the three profiles of contrib/profiles/ in
// memory, the header steps of tests/protocol-fixtures/editor/profile.json
// replayed against them the way keyvo_client::Editor::set_profile answers,
// and the page steps of pages.json the way Editor::edit_page answers.
import { describe, expect, it } from "vitest";

import { editorFile, pageSteps, profileSteps } from "../fixtures";
import { CONFIG_DIR, contribProfiles } from "./data";
import { mockProfiles } from "./profiles";
import { parseProfile, Store } from "./store";

describe("the mock profile files", () => {
  it("lists the three contrib profiles in id order", async () => {
    const files = mockProfiles(new Store(contribProfiles()));
    const list = await files.list();
    expect(list.map((profile) => profile.id)).toEqual(["global", "konsole", "vscode"]);
    expect(list[1]).toEqual({
      id: "konsole",
      name: "Konsole",
      accent: "#3fae5a",
      priority: 0,
      match: { class: ["org.kde.konsole", "konsole"] },
      pages: ["context"],
      path: `${CONFIG_DIR}/profiles/konsole.toml`,
      file: true,
    });
    expect(list[0]?.pages).toEqual(["context", "general", "third"]);
  });

  it("answers a header step the way the editor does", async () => {
    const steps = profileSteps();
    const files = mockProfiles(new Store([editorFile(steps.file)]));
    const path = `${CONFIG_DIR}/${steps.path}`;
    expect(await files.profile(steps.profile)).toEqual({ path, fields: steps.fields });
    let before = steps.fields;
    for (const step of steps.steps) {
      const edited = await files.editProfile(steps.profile, step.fields);
      expect(edited).toEqual({ path, before, fields: step.fields });
      expect(await files.profile(steps.profile)).toEqual({ path, fields: step.fields });
      before = step.fields;
    }
  });

  it("answers the page steps the way the editor does, each file's pages after it", async () => {
    const steps = pageSteps();
    const store = new Store([editorFile(steps.file)]);
    const files = mockProfiles(store);
    const path = `${CONFIG_DIR}/${steps.path}`;
    for (const step of steps.steps) {
      const edited = await files.editPage(steps.profile, step.edit);
      expect(edited).toEqual({
        path,
        edit: step.edit,
        ...(step.page === undefined ? {} : { page: step.page }),
        ...(step.undo === undefined ? {} : { undo: step.undo }),
        snapshot: expect.stringMatching(/-konsole\.toml$/) as string,
      });
      const after = parseProfile(editorFile(step.file));
      expect(store.get(steps.profile).pages.map(({ name, enabled }) => ({ name, enabled }))).toEqual(
        after.pages.map(({ name, enabled }) => ({ name, enabled })),
      );
    }
    expect((await files.list())[0]).toMatchObject({ pages: ["context", "edit", "debug"], disabled: [2, 3] });
  });

  it("undoes each page step with the edit it answers", async () => {
    const steps = pageSteps();
    const store = new Store([editorFile(steps.file)]);
    const files = mockProfiles(store);
    const pages = () => structuredClone(store.get(steps.profile).pages);
    for (const step of steps.steps.filter((step) => step.undo !== undefined)) {
      const before = pages();
      const edited = await files.editPage(steps.profile, step.edit);
      await files.editPage(steps.profile, edited.undo ?? step.edit);
      expect(pages(), JSON.stringify(step.edit)).toEqual(before);
      await files.editPage(steps.profile, step.edit);
    }
  });

  it("restores a deleted page from the snapshot the delete took", async () => {
    const steps = pageSteps();
    const store = new Store([editorFile(steps.file)]);
    const files = mockProfiles(store);
    const before = await files.list();
    const deleted = await files.editPage(steps.profile, { op: "delete", page: 2 });
    expect((await files.list())[0]?.pages).toEqual(["context", "debug"]);
    const restored = await files.restore(steps.profile, deleted.snapshot ?? "");
    expect(restored).toEqual({
      path: `${CONFIG_DIR}/${steps.path}`,
      snapshot: expect.stringMatching(/-konsole\.toml$/) as string,
    });
    expect(restored.snapshot).not.toBe(deleted.snapshot);
    expect(await files.list()).toEqual(before);
    await expect(files.restore(steps.profile, "/nowhere.toml")).rejects.toEqual({
      message: "no snapshot /nowhere.toml of `konsole`",
    });
  });

  it("refuses the edits the editor refuses, after the steps, and leaves the pages", async () => {
    const steps = pageSteps();
    const store = new Store([editorFile(steps.file)]);
    const files = mockProfiles(store);
    for (const step of steps.steps) await files.editPage(steps.profile, step.edit);
    const before = await files.list();
    for (const refused of steps.refused) {
      await expect(files.editPage(steps.profile, refused.edit)).rejects.toHaveProperty(
        "message",
        expect.stringContaining(refused.reason === "context" ? "the context page" : "the only enabled page"),
      );
    }
    expect(await files.list()).toEqual(before);
  });

  it("keeps global_pages false through a header write and lists it", async () => {
    const files = mockProfiles(new Store(contribProfiles()));
    await files.editProfile("vscode", { name: "Code", global_pages: false });
    expect((await files.profile("vscode")).fields).toEqual({ name: "Code", global_pages: false });
    expect((await files.list()).find((profile) => profile.id === "vscode")?.global_pages).toBe(false);
    await files.editProfile("vscode", { name: "Code" });
    expect((await files.list()).find((profile) => profile.id === "vscode")).not.toHaveProperty("global_pages");
  });

  it("drops the accent an edit leaves out", async () => {
    const files = mockProfiles(new Store(contribProfiles()));
    await files.editProfile("vscode", { name: "Code" });
    expect((await files.profile("vscode")).fields).toEqual({ name: "Code" });
    expect((await files.list()).find((profile) => profile.id === "vscode")?.accent).toBeUndefined();
  });

  it("rejects an unknown profile with a Problem", async () => {
    const files = mockProfiles(new Store(contribProfiles()));
    await expect(files.profile("nope")).rejects.toEqual({ message: "no profile `nope`" });
    await expect(files.editProfile("nope", { name: "x" })).rejects.toEqual({
      message: "no profile `nope`",
    });
    await expect(files.open("nope")).rejects.toEqual({ message: "no profile `nope`" });
  });

  it("opens a profile by logging its file", async () => {
    const lines: string[] = [];
    const files = mockProfiles(new Store(contribProfiles()), (line) => lines.push(line));
    await files.open("global");
    expect(lines).toEqual([`mock: open ${CONFIG_DIR}/profiles/global.toml`]);
  });
});
