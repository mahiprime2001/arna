# agent — store PCs (Tauri wrap pending)

Runs quietly in the background on each store PC.

- Persistent signaling connection; waits for connect requests.
- **Consent** (Phase 3a, headless): on a connect request it decides via
  `ARNA_CONSENT` = `accept` (default) · `prompt` (terminal y/N) · `decline`, and
  only answers WebRTC offers from admitted consoles. A 6-digit session code is
  generated and echoed to both sides. → becomes an always-on-top **popup window**
  once wrapped in Tauri (Phase 3b).
- Screen capture (`scrap`) → JPEG → WebRTC data channel. *(VP8 video track later.)*
- Input injection (`enigo`) from the `input` data channel.
- *(Later: file send/receive, chat; tray icon; run-at-login → service/SYSTEM.)*

Run: `cargo run -p arna-agent --release -- ws://127.0.0.1:8081/ws agent-1`

**Depends on:** `../core`. **Status:** ✅ capture + control + consent (Phase 3a);
Tauri wrap is Phase 3b.
