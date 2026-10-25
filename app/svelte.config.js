// TypeScript in <script lang="ts"> through Vite's own transform.
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

export default {
  preprocess: vitePreprocess(),
};
