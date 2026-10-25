// The app shell against a fake daemon: the status line shows what the
// daemon reports, or why it cannot be reached.
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
  return { status };
}

describe("App", () => {
  it("shows the view and the Keypad the daemon reports", async () => {
    render(App, { props: { daemon: daemon(() => Promise.resolve(STATUS)) } });

    await waitFor(() => {
      const line = screen.getByRole("status");
      expect(line).toHaveTextContent("Keypad connected");
      expect(line).toHaveTextContent("profile terminal, page 1, auto");
    });
  });

  it("says when no Keypad is connected", async () => {
    const status = { ...STATUS, device: { connected: false, error: "no hidraw node" } };
    render(App, { props: { daemon: daemon(() => Promise.resolve(status)) } });

    await waitFor(() =>
      expect(screen.getByRole("status")).toHaveTextContent("No Keypad: no hidraw node"),
    );
  });

  it("says when the daemon cannot be reached", async () => {
    const failing = daemon(() => Promise.reject(new Error("cannot reach the keyvo daemon")));
    render(App, { props: { daemon: failing } });

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Daemon unreachable: cannot reach the keyvo daemon",
    );
  });

  it("names the app in its heading", () => {
    render(App, { props: { daemon: daemon(() => new Promise(() => {})) } });
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("keyvo");
    expect(screen.getByRole("status")).toHaveTextContent("Connecting to the daemon");
  });
});
