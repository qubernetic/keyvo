// The app shell against a fake daemon and editor (docs/design/gui-ux.md
// §2): header, rail and the screen the rail selects. Home's own states
// are tested in Home.test.ts and KeyEditor.test.ts.
import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, describe, expect, it } from "vitest";

import App from "./App.svelte";
import type { Daemon } from "./lib/daemon";
import type { Editor } from "./lib/editor";
import { responses } from "./lib/fixtures";
import type { Status } from "./lib/protocol";

const STATUS = responses()[0] as Status;

const EDITOR: Editor = {
  key: () => Promise.resolve({ path: "/c/profiles/terminal.toml", fields: {} }),
  editKey: () => Promise.reject(new Error("not in this test")),
};

/** The App's props with a daemon whose `status` is `status`. */
function props(status: () => Promise<Status>): { daemon: Daemon; editor: Editor } {
  return {
    daemon: {
      status,
      tiles: () => Promise.resolve([]),
      outline: () => Promise.resolve({ pages: 1 }),
      press: () => Promise.reject(new Error("not in this test")),
      subscribe: () => Promise.resolve(() => {}),
    },
    editor: EDITOR,
  };
}

const WIDTH = window.innerWidth;

async function resize(width: number) {
  window.innerWidth = width;
  await fireEvent(window, new Event("resize"));
}

afterEach(() => {
  window.innerWidth = WIDTH;
});

function app(status: () => Promise<Status> = () => Promise.resolve(STATUS)) {
  render(App, { props: props(status) });
}

describe("App", () => {
  it("names the app in its heading", () => {
    app(() => new Promise(() => {}));
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("keyvo");
    expect(screen.getByRole("list", { name: "Connecting to the daemon" })).toBeInTheDocument();
  });

  it("opens on Home with the view the daemon reports", async () => {
    app();
    expect(screen.getByRole("button", { name: "Home" })).toHaveAttribute("aria-current", "page");
    expect(
      await screen.findByRole("grid", { name: "Keypad mirror: profile terminal, page 1" }),
    ).toBeInTheDocument();
    expect(await screen.findByRole("heading", { name: "Key 1" })).toBeInTheDocument();
  });

  it("says when the daemon cannot be reached", async () => {
    app(() => Promise.reject(new Error("cannot reach the keyvo daemon")));
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(
        "Daemon unreachable: cannot reach the keyvo daemon",
      ),
    );
    expect(screen.getByRole("combobox", { name: "Device" })).toHaveValue("simulator");
  });

  it("offers the Keypad the daemon reports in the header", async () => {
    app();
    const selector = screen.getByRole("combobox", { name: "Device" });
    await waitFor(() => expect(selector).toHaveValue("keypad"));
    await fireEvent.change(selector, { target: { value: "simulator" } });
    expect(selector).toHaveValue("simulator");
  });

  it("switches to Profiles with Ctrl+2 and back to Home with Ctrl+1", async () => {
    app();
    await screen.findByRole("grid");
    await fireEvent.keyDown(window, { key: "2", ctrlKey: true });
    expect(screen.getByRole("button", { name: "Profiles" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(screen.queryByRole("grid")).toBeNull();
    expect(screen.getByRole("region", { name: "Editor" })).toHaveTextContent(
      "The profile editor is not built yet",
    );
    await fireEvent.keyDown(window, { key: "1", ctrlKey: true });
    expect(screen.getByRole("grid")).toBeInTheDocument();
  });

  it("opens Profiles from the profile in Home's status strip", async () => {
    app();
    await fireEvent.click(
      await screen.findByRole("button", { name: "Profile terminal, open Profiles" }),
    );
    expect(screen.getByRole("button", { name: "Profiles" })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it("keeps Home's undo to Home", async () => {
    app();
    await screen.findByRole("grid");
    await fireEvent.click(screen.getByRole("button", { name: "Profiles" }));
    expect(screen.queryByRole("button", { name: "Undo" })).toBeNull();
  });

  it("lays Profiles out by the window width", async () => {
    await resize(1200);
    app();
    await fireEvent.click(screen.getByRole("button", { name: "Profiles" }));
    expect(screen.getAllByRole("region").map((region) => region.getAttribute("aria-label"))).toEqual(
      ["List", "Editor", "Preview"],
    );
    expect(screen.getByRole("button", { name: "Home" })).toHaveTextContent("Home");
    await resize(960);
    expect(screen.getAllByRole("region").map((region) => region.getAttribute("aria-label"))).toEqual(
      ["Editor"],
    );
    expect(screen.getByRole("button", { name: "Home" })).toHaveTextContent("");
  });

  it("gives every control an aria-label", async () => {
    app();
    await screen.findByRole("heading", { name: "Key 1" });
    await fireEvent.click(screen.getByRole("button", { name: "More actions" }));
    const controls = document.querySelectorAll("button, select, input, textarea, [role=menuitem]");
    expect(controls.length).toBeGreaterThan(10);
    for (const control of controls) {
      if (control.closest("[hidden]") !== null) continue;
      expect(control.getAttribute("aria-label"), control.outerHTML).toBeTruthy();
    }
  });
});
