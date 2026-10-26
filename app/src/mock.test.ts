// The app as `just app-dev` mounts it: the real shell against the
// in-memory mock of lib/mock/. Home shows the mirror of the global profile
// and the trace the mock replays; ?mock-error=unreachable shows Home's
// error state.
import { render, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";

import App from "./App.svelte";
import { mockClients } from "./lib/mock";

function key(n: number): HTMLElement {
  return screen.getByRole("button", { name: new RegExp(`^Key ${n}\\b`) });
}

describe("the app on the mock", () => {
  it("mirrors the global profile's keys", async () => {
    render(App, { props: mockClients("", document.createElement("div")) });
    await waitFor(() => expect(key(5)).toHaveAccessibleName("Key 5: OK, exec true, global layer"));
    expect(key(3)).toHaveAccessibleName("Key 3: Type, chord shift+k, e, y, global layer");
  });

  it("shows Home's error state with ?mock-error=unreachable", async () => {
    render(App, {
      props: mockClients("?mock-error=unreachable", document.createElement("div")),
    });
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Daemon unreachable: cannot reach the keyvo daemon at /run/user/1000/keyvo/keyvo.sock",
    );
  });
});
