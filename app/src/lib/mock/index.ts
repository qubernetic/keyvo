// The in-memory mock of the daemon for the Vite dev server (`just app-dev`,
// VITE_KEYVO_MOCK=1): main.ts mounts the app with these clients instead of
// the Tauri shell's. Only the dev server loads this module; a build without
// the variable leaves it out.
import type { Daemon } from "../daemon";
import type { Editor } from "../editor";
import type { ProfileFiles } from "../profiles";
import { mockDaemon } from "./daemon";
import { contribProfiles, EMPTY_PROFILE } from "./data";
import { mockEditor } from "./editor";
import { mockProfiles } from "./profiles";
import { Store } from "./store";
import { applyTheme, themeOf } from "./themes";

export interface Clients {
  daemon: Daemon;
  editor: Editor;
  files: ProfileFiles;
}

/** The three clients over fresh copies of the contrib profiles, the token
 * set `?theme=` names applied to `root`; `?mock-error=unreachable` makes
 * `status` reject, `?mock-error=write` makes every `editKey` reject with a
 * Problem carrying file, line and column, `?mock-profiles=empty` adds a
 * profile with no pages, `?mock-locked` locks the session,
 * `?mock-dialpad=none` reports no Dialpad. */
export function mockClients(search: string, root: HTMLElement): Clients {
  applyTheme(themeOf(search), root);
  const query = new URLSearchParams(search);
  const empty = query.get("mock-profiles") === "empty";
  const store = new Store([...contribProfiles(), ...(empty ? [EMPTY_PROFILE] : [])]);
  const unreachable = query.get("mock-error") === "unreachable";
  const refuseWrites = query.get("mock-error") === "write";
  const locked = query.has("mock-locked");
  const dialpad = query.get("mock-dialpad") !== "none";
  return {
    daemon: mockDaemon(store, { unreachable, locked, dialpad }),
    editor: mockEditor(store, { refuseWrites }),
    files: mockProfiles(store),
  };
}
