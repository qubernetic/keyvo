// Entry point of the Tauri window.
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

export default mount(App, {
  target,
  props: { daemon: tauriDaemon, editor: tauriEditor, files: tauriProfiles },
});
