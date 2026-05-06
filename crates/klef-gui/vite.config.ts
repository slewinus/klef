import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Vite's job: bundle the Svelte app from `ui/` into `build/`.
// Tauri's job: load `build/` at runtime (configured in tauri.conf.json).
export default defineConfig({
  plugins: [svelte()],
  // Use a fixed dev port so tauri.conf.json's devUrl can match.
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    outDir: "build",
    emptyOutDir: true,
    target: "safari14",
  },
  // Quiet down vite's chatter when invoked by `cargo tauri dev`.
  clearScreen: false,
});
