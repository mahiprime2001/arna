import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Driven by the Tauri CLI (`npm run tauri:dev`): fixed port the consent windows
// point at, don't wipe Tauri's logs, and don't watch the Rust crate.
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 4320,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  preview: { port: 4320 },
});
