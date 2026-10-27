// The mock's entry: the three clients over one store, the theme and the
// error switch from the query string.
import { describe, expect, it } from "vitest";

import { mockClients } from "./index";

describe("the mock clients", () => {
  it("share one store, so an edit shows in the daemon's answers", async () => {
    const { daemon, files, editor } = mockClients("", document.createElement("div"));
    await files.editProfile("global", { name: "Global", accent: "#123456" });
    expect((await daemon.outline("global")).accent).toBe("#123456");
    await editor.editKey({ profile: "global", page: 1, key: 9 }, { label: "Nine" });
    const tiles = await daemon.tiles("global", 1, 9, { width: 118, height: 118 });
    expect(tiles[8]?.label).toBe("Nine");
  });

  it("starts from the files again each time", async () => {
    const first = mockClients("", document.createElement("div"));
    await first.files.editProfile("global", { name: "Changed" });
    const second = mockClients("", document.createElement("div"));
    expect((await second.files.profile("global")).fields.name).toBe("Global");
  });

  it("applies ?theme= and ?mock-error=unreachable", async () => {
    const root = document.createElement("div");
    const { daemon } = mockClients("?theme=adwaita-light&mock-error=unreachable", root);
    expect(root.style.getPropertyValue("--kv-bg")).toBe("#fafafb");
    await expect(daemon.status()).rejects.toMatch(/cannot reach the keyvo daemon/);
  });

  it("locks the session under ?mock-locked, and only there", async () => {
    const root = document.createElement("div");
    expect((await mockClients("?mock-locked", root).daemon.status()).view.locked).toBe(true);
    expect((await mockClients("", root).daemon.status()).view.locked).toBe(false);
  });

  it("adds a profile with no pages under ?mock-profiles=empty", async () => {
    const root = document.createElement("div");
    const ids = async (search: string) =>
      (await mockClients(search, root).files.list()).map((profile) => [profile.id, profile.pages]);
    expect(await ids("")).not.toContainEqual(["empty", []]);
    expect(await ids("?mock-profiles=empty")).toContainEqual(["empty", []]);
    expect(await ids("?mock-profiles=other")).not.toContainEqual(["empty", []]);
  });

  it("refuses the editor's writes under ?mock-error=write, and only there", async () => {
    const root = document.createElement("div");
    const target = { profile: "global", page: 1, key: 9 };
    const refused = mockClients("?mock-error=write", root);
    await expect(refused.editor.editKey(target, { label: "Nine" })).rejects.toMatchObject({
      message: expect.stringContaining("?mock-error=write"),
      line: expect.any(Number),
      column: 1,
    });
    expect((await refused.daemon.status()).device.connected).toBe(true);
    const written = mockClients("?mock-error=unreachable", root);
    await expect(written.editor.editKey(target, { label: "Nine" })).resolves.toMatchObject({
      fields: { label: "Nine" },
    });
  });

  it("answers status without ?mock-error", async () => {
    const root = document.createElement("div");
    const { daemon } = mockClients("?mock-error=other", root);
    expect((await daemon.status()).device.connected).toBe(true);
    expect(root.style.getPropertyValue("--kv-bg")).toBe("#202326");
  });
});
