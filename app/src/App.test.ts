// The app shell against a fake daemon: the heading and Home, whose states
// are tested in Home.test.ts.
import { render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";

import App from "./App.svelte";
import type { Daemon } from "./lib/daemon";
import type { Status } from "./lib/protocol";

const STATUS: Status = {
  version: "0.0.0",
  device: { connected: true, keys: 9 },
  view: { profile: "terminal", page: 1, mode: "auto" },
  trace: 0,
};

function daemon(status: () => Promise<Status>): Daemon {
  return {
    status,
    tiles: () => Promise.resolve([]),
    press: () => Promise.reject(new Error("not in this test")),
    subscribe: () => Promise.resolve(() => {}),
  };
}

describe("App", () => {
  it("names the app in its heading", () => {
    render(App, { props: { daemon: daemon(() => new Promise(() => {})) } });
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("keyvo");
    expect(screen.getByText("Connecting to the daemon")).toBeInTheDocument();
  });

  it("shows Home with the view the daemon reports", async () => {
    render(App, { props: { daemon: daemon(() => Promise.resolve(STATUS)) } });
    expect(
      await screen.findByRole("grid", { name: "Keypad mirror: profile terminal, page 1" }),
    ).toBeInTheDocument();
  });

  it("says when the daemon cannot be reached", async () => {
    render(App, {
      props: { daemon: daemon(() => Promise.reject(new Error("cannot reach the keyvo daemon"))) },
    });
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(
        "Daemon unreachable: cannot reach the keyvo daemon",
      ),
    );
  });
});
