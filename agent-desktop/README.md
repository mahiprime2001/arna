# agent-desktop — Arna Agent (Tauri v2)

The store-PC desktop app. It runs the reusable agent loop (`arna_agent::run`
from `../agent`) in the background and gives it a **GUI consent**: when a console
asks to connect, a small always-on-top popup shows the admin's name, a 6-digit
session code, and **Accept / Decline**. The agent only streams/admits the console
once the operator accepts.

- **Tray app** — no main window; lives in the system tray (right-click → Quit).
- **Consent popup** — created on demand per request; the button choice flows back
  to the agent over a oneshot channel (auto-declines after 60s).
- The headless `../agent` binary still exists for two-machine testing (consent via
  `ARNA_CONSENT` policy); this app is the shippable version with a real popup.

## Run

```bash
# 1) backend
cargo run --manifest-path ../backend/Cargo.toml
# 2) the agent desktop app (registers as agent-1, sits in the tray)
cd agent-desktop && npm install && npm run tauri:dev
# 3) a console (browser or the Tauri console) -> Connect to agent-1
```

Config via env: `ARNA_BACKEND` (default `ws://127.0.0.1:8081/ws`),
`ARNA_AGENT_ID` (default `agent-1`).

## Build

```bash
npm run tauri:build                  # installer (needs WiX/NSIS on Windows)
npm run tauri:build -- --no-bundle   # just the binary
```

## Layout

- `src/` — the Vue consent-popup frontend (`ConsentApp.vue` reads the request from
  the window query string and calls the `respond_consent` command).
- `src-tauri/` — the Rust shell: tray, the agent-loop spawn, and the consent
  bridge. Depends on `../agent`. Excluded from the root Cargo workspace.

## Status

Phase 3b — built and verified: registers, captures, and shows the consent popup
on an incoming request. Run-at-login / Windows service hardening is Phase 5.
