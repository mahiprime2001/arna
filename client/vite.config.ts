import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";
import fs from "node:fs";

// Opt-in HTTPS for LAN testing: set ARNA_TLS_CERT / ARNA_TLS_KEY to the mkcert
// files. Without them, dev serves plain http (fine for localhost).
const cert = process.env.ARNA_TLS_CERT;
const key = process.env.ARNA_TLS_KEY;
const https =
  cert && key && fs.existsSync(cert) && fs.existsSync(key)
    ? { cert: fs.readFileSync(cert), key: fs.readFileSync(key) }
    : undefined;

export default defineConfig({
  plugins: [react()],
  resolve: { alias: { "@": path.resolve(__dirname, "./src") } },
  server: { port: 4320, host: true, strictPort: true, https },
  clearScreen: false,
});
