// The daemon as the frontend sees it. The Tauri shell (src-tauri) talks to
// the socket through keyvo-client; tests pass a fake.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { Action, Event, PressKind, Status, TraceRecord, View } from "./protocol";

/** What a subscription hands on, as keyvo_client::Update: the view, every
 * event, then why it ended. */
export type Update =
  | { kind: "subscribed"; view: View }
  | { kind: "event"; event: Event }
  | { kind: "closed"; reason: string };

/** One key of the mirror: the tile as the pad shows it, and what it says. */
export interface Tile {
  /** A PNG data URL. */
  image: string;
  label?: string;
  action?: Action;
}

export interface Daemon {
  /** The daemon's `status` reply; rejects when the daemon cannot be reached. */
  status(): Promise<Status>;
  /** The keys of `profile`'s page `page` from the configuration directory,
   * in key order; rejects when they do not render. */
  tiles(profile: string, page: number): Promise<Tile[]>;
  /** A dry-run press: the daemon resolves and traces it, executes nothing. */
  press(key: number, kind: PressKind): Promise<TraceRecord>;
  /** Follows a subscription that replays the newest `replay` trace entries;
   * `listener` gets every update, the last one `closed`. Resolves with the
   * function that stops listening. */
  subscribe(replay: number, listener: (update: Update) => void): Promise<() => void>;
}

/** The event the shell emits each update on. */
export const UPDATE_EVENT = "keyvo://update";

/** The daemon through the Tauri shell's commands. */
export const tauriDaemon: Daemon = {
  status: () => invoke<Status>("status"),
  tiles: (profile, page) => invoke<Tile[]>("tiles", { profile, page }),
  press: (key, kind) => invoke<TraceRecord>("press", { key, kind }),
  async subscribe(replay, listener) {
    const unlisten = await listen<Update>(UPDATE_EVENT, (message) => listener(message.payload));
    await invoke("subscribe", { replay });
    return unlisten;
  },
};
