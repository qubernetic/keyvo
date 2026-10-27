// The pin (pad screen brief §5.2, decision Q2; #108 open point 7): the
// inspector follows the pad until something pins it, and from then on
// the pad never retargets it.
import { describe, expect, it } from "vitest";

import { difference, follow, kept, pinOnSelect, pinPage, pinProfile, target, type Pin } from "./pin";
import type { View } from "./protocol";

const PAD: View = { profile: "terminal", page: 2, mode: "auto" };
const MOVED: View = { profile: "vscode", page: 1, mode: "auto" };

describe("the pin", () => {
  it("follows the pad by default", () => {
    expect(target(undefined, PAD, 5)).toEqual({ profile: "terminal", page: 2, key: 5 });
    expect(target(undefined, MOVED, 5)).toEqual({ profile: "vscode", page: 1, key: 5 });
    expect(difference(undefined, PAD)).toBeUndefined();
  });

  it("has no target before the pad reports a view", () => {
    expect(target(undefined, undefined, 1)).toBeUndefined();
    expect(pinOnSelect(undefined, undefined)).toBeUndefined();
  });

  it("pins the pad's profile and page when a key is selected", () => {
    expect(pinOnSelect(undefined, PAD)).toEqual({ profile: "terminal", page: 2, fromKey: true });
  });

  it("keeps a pin on the pad's page when a key is selected", () => {
    const pinned = pinOnSelect(undefined, PAD);
    expect(pinOnSelect(pinned, PAD)).toBe(pinned);
  });

  it("keeps a profile or page the user chose when a key is selected", () => {
    const pinned = pinProfile("vscode");
    expect(pinOnSelect(pinned, PAD)).toBe(pinned);
    const page = pinPage(pinOnSelect(undefined, PAD) as Pin, 3);
    expect(pinOnSelect(page, PAD)).toBe(page);
  });

  it("pins the pad's page again when a key is selected while the pad is away from a key's pin", () => {
    const pinned = pinOnSelect(undefined, MOVED);
    expect(pinOnSelect(pinned, PAD)).toEqual({ profile: "terminal", page: 2, fromKey: true });
  });

  it("stays where it is when the pad moves", () => {
    const pinned = pinOnSelect(undefined, PAD);
    expect(target(pinned, MOVED, 5)).toEqual({ profile: "terminal", page: 2, key: 5 });
  });

  it("names the pad's profile and page when they differ from the pin", () => {
    const pinned = pinOnSelect(undefined, PAD);
    expect(difference(pinned, MOVED)).toEqual({ profile: "vscode", page: 1 });
    expect(difference(pinned, { ...PAD, page: 1 })).toEqual({ profile: "terminal", page: 1 });
  });

  it("says nothing when the pad and the pin agree", () => {
    expect(difference(pinOnSelect(undefined, PAD), PAD)).toBeUndefined();
    expect(difference(pinProfile("terminal"), undefined)).toBeUndefined();
  });

  it("starts a pinned profile on page 1, the context page", () => {
    expect(pinProfile("vscode")).toEqual({ profile: "vscode", page: 1 });
  });

  it("moves a pin to another page of its profile", () => {
    expect(pinPage(pinProfile("vscode"), 3)).toEqual({ profile: "vscode", page: 3 });
  });

  it("clears on follow", () => {
    expect(follow()).toBeUndefined();
    expect(target(follow(), MOVED, 2)).toEqual({ profile: "vscode", page: 1, key: 2 });
  });

  it("unpins a profile missing from the list", () => {
    const pinned = pinProfile("vscode");
    expect(kept(pinned, [{ id: "global" }, { id: "vscode" }])).toBe(pinned);
    expect(kept(pinned, [{ id: "global" }, { id: "terminal" }])).toBeUndefined();
    expect(kept(undefined, [{ id: "global" }])).toBeUndefined();
  });
});
