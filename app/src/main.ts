// Entry point of the Tauri window. On the Vite dev server with
// VITE_KEYVO_MOCK=1 (`just app-dev`) the app runs against the in-memory
// mock of lib/mock/ instead; a build without the variable leaves it out.
import { mount } from "svelte";

import App from "./App.svelte";
import { tauriDaemon } from "./lib/daemon";
import { tauriEditor } from "./lib/editor";
import { tauriProfiles } from "./lib/profiles";
import "./styles/tokens.css";
import "./styles/controls.css";

const target = document.getElementById("app");
if (target === null) {
  throw new Error("index.html has no #app element");
}

const clients =
  import.meta.env.VITE_KEYVO_MOCK === "1"
    ? (await import("./lib/mock")).mockClients(window.location.search, document.documentElement)
    : { daemon: tauriDaemon, editor: tauriEditor, files: tauriProfiles };

export default mount(App, { target, props: clients });
