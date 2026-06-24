# console — Arna Console

The desktop app the admin/owner runs to connect to machines. A **Vue 3 + Vite +
Tailwind** frontend wrapped in a **Tauri v2** desktop shell (`src-tauri/`). The
same frontend runs in a browser for quick dev and inside the Tauri window for the
shipped app.

## Develop

In the browser (fastest iteration):

```bash
npm install
npm run dev      # http://localhost:4310
```

As the desktop app (Tauri window around the same frontend):

```bash
npm run tauri:dev    # builds the Rust shell, opens the window, HMR on :4310
```

Then start the backend + an agent and connect:

```bash
# backend
cargo run --manifest-path ../backend/Cargo.toml
# agent (shares its screen)
cargo run -p arna-agent --release -- ws://127.0.0.1:8081/ws agent-1
```

Set Agent = `agent-1`, optionally paste an SSO **Ticket**, click **Connect**. The
agent must accept (consent), then the live screen appears.

## Build

```bash
npm run tauri:build              # installer (msi/nsis) — needs WiX/NSIS on Windows
npm run tauri:build -- --no-bundle   # just the binary, no packaging
```

## Layout

- `src/App.vue` — the console UI (connect bar + live screen view).
- `src/composables/useRemote.ts` — signaling, consent handshake, WebRTC session.
- `src-tauri/` — Tauri v2 Rust shell (window config, icons, capabilities). Excluded
  from the root Cargo workspace; the Tauri CLI manages it.

## Status

Phase 3b — desktop wrap. View + control + consent flow work. Files, chat, and the
multi-monitor/quality controls land in later phases.
