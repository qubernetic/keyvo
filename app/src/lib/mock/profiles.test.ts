// The mock's profile files: the three profiles of contrib/profiles/ in
// memory, and the header steps of tests/protocol-fixtures/editor/profile.json
// replayed against them the way keyvo_client::Editor::set_profile answers.
import { describe, expect, it } from "vitest";

import { editorFile, profileSteps } from "../fixtures";
import { CONFIG_DIR, contribProfiles } from "./data";
import { mockProfiles } from "./profiles";
import { Store } from "./store";

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
