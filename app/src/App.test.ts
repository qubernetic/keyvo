// The app shell against a fake daemon and editor: the heading and Home,
// whose states are tested in Home.test.ts and KeyEditor.test.ts.
import { render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";

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
      press: () => Promise.reject(new Error("not in this test")),
      subscribe: () => Promise.resolve(() => {}),
    },
    editor: EDITOR,
  };
}

describe("App", () => {
  it("names the app in its heading", () => {
    render(App, { props: props(() => new Promise(() => {})) });
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("keyvo");
    expect(screen.getByText("Connecting to the daemon")).toBeInTheDocument();
  });

  it("shows Home with the view the daemon reports", async () => {
    render(App, { props: props(() => Promise.resolve(STATUS)) });
    expect(
      await screen.findByRole("grid", { name: "Keypad mirror: profile terminal, page 1" }),
    ).toBeInTheDocument();
    expect(await screen.findByRole("heading", { name: "Key 1" })).toBeInTheDocument();
  });

  it("says when the daemon cannot be reached", async () => {
    render(App, {
      props: props(() => Promise.reject(new Error("cannot reach the keyvo daemon"))),
    });
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(
        "Daemon unreachable: cannot reach the keyvo daemon",
      ),
    );
  });
});
