// The daemon as the frontend sees it. The Tauri shell (src-tauri) talks to
// the socket through keyvo-client; tests pass a fake.
import { invoke } from "@tauri-apps/api/core";

import type { Status } from "./protocol";

export interface Daemon {
  /** The daemon's `status` reply; rejects when the daemon cannot be reached. */
  status(): Promise<Status>;
}

/** The daemon through the Tauri shell's commands. */
export const tauriDaemon: Daemon = {
  status: () => invoke<Status>("status"),
};
