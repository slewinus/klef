import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Vite's job: bundle the Svelte app from `ui/` into `build/`.
// Tauri's job: load `build/` at runtime (configured in tauri.conf.json).
//
// `stripCrossorigin` removes the `crossorigin` attribute Vite injects on
// generated <script> and <link> tags. Tauri 2 serves built assets over its
// own `tauri://localhost` protocol; the `crossorigin` attribute combined
// with that scheme triggers CORS validation that fails silently — leaving
// us with a blank window. Stripping it lets us keep a strict CSP without
// an `unsafe-inline` workaround.
const stripCrossorigin = () => ({
  name: "klef-strip-crossorigin",
  transformIndexHtml(html: string) {
    return html.replace(/\s+crossorigin/g, "");
  },
});

export default defineConfig({
  plugins: [svelte(), stripCrossorigin()],
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
