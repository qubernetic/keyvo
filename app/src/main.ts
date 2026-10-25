// Entry point of the Tauri window.
import { mount } from "svelte";

import App from "./App.svelte";
import { tauriDaemon } from "./lib/daemon";
import { tauriEditor } from "./lib/editor";
import "./styles/tokens.css";

const target = document.getElementById("app");
if (target === null) {
  throw new Error("index.html has no #app element");
}

export default mount(App, { target, props: { daemon: tauriDaemon, editor: tauriEditor } });
