import { readFileSync } from "node:fs";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// The version the app reports comes from tauri.conf.json, which is what the
// bundle is stamped with. Reading it here rather than asking the backend keeps
// the About line out of the IPC surface entirely — it is a build constant, not
// a piece of state.
const appVersion = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8")).version;

// Tauri expects a fixed port and does not want vite obscuring rust errors.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  define: { __APP_VERSION__: JSON.stringify(appVersion) },
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "safari15", // matches the WKWebView floor on macOS 13
    sourcemap: true,
  },
});
