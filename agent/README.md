# agent — Tauri app (store PCs)

Runs quietly in the background on each store PC.

- Persistent signaling connection; waits for connect requests.
- **Consent popup** — small always-on-top window: "Admin wants to connect" +
  6-digit code + Accept/Decline (does not block the cashier's work).
- Screen capture (`windows-capture`) → VP8 encode (`vpx-encode`) → WebRTC video track.
- Input injection (`enigo`) from the `input` data channel.
- File send/receive, chat.
- Tray icon; run-at-login (v1) → Windows service / SYSTEM (Phase 5, for UAC).

**Depends on:** `../core`. **Status:** not yet implemented (begins Phase 1).
