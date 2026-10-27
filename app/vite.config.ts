// Vite builds the frontend into dist/ for the Tauri shell (src-tauri,
// frontendDist) and serves it on a fixed port for `tauri dev` (devUrl);
// vitest runs the unit and component tests in jsdom. With
// VITE_KEYVO_MOCK=1 (`just app-dev`) the dev server serves the app against
// the in-memory mock of src/lib/mock/, which reads its data in place from
// the repository directories below; vitest reads them too. The tile icons
// of the icon picker (src/lib/tileIcons.ts) are read from keyvo-core's
// bundle in every mode.
import { resolve } from "node:path";

import { svelte } from "@sveltejs/vite-plugin-svelte";
import { svelteTesting } from "@testing-library/svelte/vite";
import { defineConfig } from "vitest/config";

const TILE_ICONS = "crates/keyvo-core/assets/lucide";
const MOCK_DATA = ["contrib/profiles", "tests/protocol-fixtures", "crates/keyvo-core/tests/snapshots"];

const mock = process.env.VITE_KEYVO_MOCK === "1" || process.env.VITEST !== undefined;
const app = import.meta.dirname;
const allowed = [TILE_ICONS, ...(mock ? MOCK_DATA : [])].map((dir) => resolve(app, "..", dir));

export default defineConfig({
  plugins: [svelte(), svelteTesting()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    fs: { allow: [app, ...allowed] },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.ts"],
    setupFiles: ["src/test-setup.ts"],
  },
});
