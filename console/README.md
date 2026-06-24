# console — Arna Console

The desktop app the admin/owner runs to connect to machines. Built as a **Vue 3 +
Vite + Tailwind** frontend now; it becomes the **Tauri** desktop app later (Tauri
wraps this same web frontend with a Rust shell — `src-tauri/` is added then).

## Develop

```bash
npm install
npm run dev      # http://localhost:4310
```

Then start the backend + an agent and connect:

```bash
# backend
cargo run --manifest-path ../backend/Cargo.toml
# agent (shares its screen)
cargo run -p arna-agent --release -- ws://127.0.0.1:8081/ws agent-1
```

Open http://localhost:4310, set Agent = `agent-1`, click **Connect**.

## Layout

- `src/App.vue` — the console UI (connect bar + live screen view).
- `src/composables/useRemote.ts` — signaling + WebRTC session (view-only for now).

## Status

Phase 1c — **view only** (see the remote screen). Mouse/keyboard control, files,
chat, and the consent flow land in later phases.
