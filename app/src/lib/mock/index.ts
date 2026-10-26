// The in-memory mock of the daemon for the Vite dev server (`just app-dev`,
// VITE_KEYVO_MOCK=1): main.ts mounts the app with these clients instead of
// the Tauri shell's. Only the dev server loads this module; a build without
// the variable leaves it out.
import type { Daemon } from "../daemon";
import type { Editor } from "../editor";
import type { ProfileFiles } from "../profiles";
import { mockDaemon } from "./daemon";
import { contribProfiles } from "./data";
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
 * `status` reject. */
export function mockClients(search: string, root: HTMLElement): Clients {
  applyTheme(themeOf(search), root);
  const store = new Store(contribProfiles());
  const unreachable = new URLSearchParams(search).get("mock-error") === "unreachable";
  return {
    daemon: mockDaemon(store, { unreachable }),
    editor: mockEditor(store),
    files: mockProfiles(store),
  };
}
