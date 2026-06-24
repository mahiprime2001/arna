import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Vite is also driven by the Tauri CLI (`npm run tauri dev`), so: keep the dev
// server on a fixed port the Tauri window points at, don't wipe Tauri's logs,
// and don't watch the Rust crate.
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 4310,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  preview: { port: 4310 },
});
