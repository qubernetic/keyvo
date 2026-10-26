// The editor's pure parts: the form both ways, problems as one line, and
// the session history.
import { describe, expect, it } from "vitest";

import {
  afterRedo,
  afterUndo,
  describe as describeProblem,
  emptyHistory,
  fromForm,
  problemOf,
  recorded,
  sameFields,
  sameTarget,
  toForm,
  viewOf,
  type Change,
  type Form,
} from "./editor";
import { editorSteps } from "./fixtures";

const BLANK: Form = { label: "", color: "", icon: "", actionType: "none", action: "" };

describe("the form", () => {
  it("round-trips the fixture's fields", () => {
    const steps = editorSteps();
    for (const fields of [steps.fields, ...steps.steps.map((step) => step.fields)]) {
      expect(fromForm(toForm(fields))).toEqual({ fields });
    }
  });

  it("shows every field as text", () => {
    expect(
      toForm({ icon: "lucide:hammer", label: "Build", color: "#aa3300", action: { type: "exec", cmd: "make" } }),
    ).toEqual({ label: "Build", color: "#aa3300", icon: "lucide:hammer", actionType: "exec", action: "make" });
    expect(toForm({})).toEqual(BLANK);
  });

  it("leaves empty inputs out and trims icon, color and action", () => {
    expect(fromForm(BLANK)).toEqual({ fields: {} });
    expect(
      fromForm({ label: " A ", color: " #AA3300 ", icon: " lucide:x ", actionType: "chord", action: " ctrl+c " }),
    ).toEqual({
      fields: { label: " A ", color: "#aa3300", icon: "lucide:x", action: { type: "chord", keys: "ctrl+c" } },
    });
  });

  it("says why a color or an empty action cannot be written, and which field", () => {
    expect(fromForm({ ...BLANK, color: "red" })).toEqual({
      invalid: "The color red is not written #rrggbb.",
      field: "color",
    });
    expect(fromForm({ ...BLANK, actionType: "chord" })).toEqual({
      invalid: "Enter the chord's keys to write it.",
      field: "action",
    });
    expect(fromForm({ ...BLANK, actionType: "exec", action: " " })).toEqual({
      invalid: "Enter the command to write it.",
      field: "action",
    });
  });

  it("compares fields the way the file does", () => {
    expect(sameFields({ color: "#AA3300" }, { color: "#aa3300" })).toBe(true);
    expect(sameFields({}, { label: "" })).toBe(false);
    const chord = { action: { type: "chord" as const, keys: "x" } };
    expect(sameFields(chord, { action: { type: "exec", cmd: "x" } })).toBe(false);
    expect(sameFields(chord, { action: { type: "chord", keys: "x" } })).toBe(true);
    expect(sameFields(chord, {})).toBe(false);
  });

  it("compares targets by profile, page and key", () => {
    const target = { profile: "terminal", page: 1, key: 2 };
    expect(sameTarget(target, { ...target })).toBe(true);
    expect(sameTarget(target, { ...target, page: 2 })).toBe(false);
    expect(sameTarget(target, undefined)).toBe(false);
    expect(sameTarget(undefined, undefined)).toBe(true);
  });
});

describe("the view after a write", () => {
  it("keeps the file, the fields and the line of the key", () => {
    const steps = editorSteps();
    for (const step of steps.steps) {
      const edited = {
        path: "/c/profiles/terminal.toml",
        before: steps.fields,
        fields: step.fields,
        line: step.line,
        snapshot: "/s/1",
      };
      expect(viewOf(edited)).toEqual({ path: edited.path, fields: step.fields, line: step.line });
    }
  });

  it("has no line when the write answers none", () => {
    const view = viewOf({ path: "/c/profiles/t.toml", before: {}, fields: {} });
    expect(view).toEqual({ path: "/c/profiles/t.toml", fields: {} });
    expect(view).not.toHaveProperty("line");
  });
});

describe("problems", () => {
  it("are one line with file, line and column like a diagnostic", () => {
    expect(
      describeProblem({ message: "invalid type", path: "/c/profiles/t.toml", line: 4, column: 8 }),
    ).toBe("/c/profiles/t.toml:4:8: invalid type");
    expect(describeProblem({ message: "gone", path: "/c/profiles/t.toml" })).toBe(
      "/c/profiles/t.toml: gone",
    );
    expect(describeProblem({ message: "no state directory" })).toBe("no state directory");
  });

  it("come from the shell's rejection, an Error or anything else", () => {
    expect(problemOf({ message: "bad", path: "/p", line: 1, column: 2, extra: true })).toEqual({
      message: "bad",
      path: "/p",
      line: 1,
      column: 2,
    });
    expect(problemOf({ message: "bad", line: "1" })).toEqual({ message: "bad" });
    expect(problemOf(new Error("broken pipe"))).toEqual({ message: "broken pipe" });
    expect(problemOf("refused")).toEqual({ message: "refused" });
  });
});

describe("the history", () => {
  const change = (key: number): Change => ({
    target: { profile: "terminal", page: 1, key },
    before: {},
    after: { label: String(key) },
  });

  it("undoes the newest write first and redoes in order", () => {
    let history = recorded(recorded(emptyHistory(), change(1)), change(2));
    history = afterUndo(history);
    expect(history).toEqual({ done: [change(1)], undone: [change(2)] });
    history = afterUndo(history);
    expect(history).toEqual({ done: [], undone: [change(2), change(1)] });
    expect(afterUndo(history)).toBe(history);
    history = afterRedo(history);
    expect(history).toEqual({ done: [change(1)], undone: [change(2)] });
  });

  it("drops what is left to redo on a new write", () => {
    const history = afterUndo(recorded(emptyHistory(), change(1)));
    expect(recorded(history, change(3))).toEqual({ done: [change(3)], undone: [] });
    expect(afterRedo(emptyHistory())).toEqual(emptyHistory());
  });
});
