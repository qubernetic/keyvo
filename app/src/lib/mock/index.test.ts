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

  it("answers status without ?mock-error", async () => {
    const root = document.createElement("div");
    const { daemon } = mockClients("?mock-error=other", root);
    expect((await daemon.status()).device.connected).toBe(true);
    expect(root.style.getPropertyValue("--kv-bg")).toBe("#202326");
  });
});
