# Arna — Current Status (handoff)

Snapshot of where the build is, so work can continue in a fresh chat. The full
design is in [PLAN.md](PLAN.md); this is "what exists and works right now."

## Repos
- **Platform:** `github.com/mahiprime2001/arna` (private) — local: `d:\Siri-apps\arna-remote`
- **Website:** `github.com/mahiprime2001/arna-website` (private) — local: `d:\Siri-apps\arna-website`

Commits are authored as `mahiprime2001` (noreply email) + `Co-Authored-By: Claude`.

## What Arna is
A self-hosted, all-in-one remote platform: **Remote control, Fleet, Chat, Meet,
Files, SSH/FTP** — built on one WebRTC engine. v1 focus = remote control. Later
wraps into **Tauri** desktop apps (Agent on machines, Console for the controller).

**Audience & distribution:** general-purpose, **for everyone** (personal +
enterprise) — like TeamViewer/AnyDesk. **Open source + hosted:** we host it so
non-devs just sign up (apps default to the hosted server), and developers can
**self-host** on their own server (configurable address). The "store/admin/fleet"
wording here is the enterprise lens, not the whole product; a **Personal** vs
**Enterprise** edition split is planned later. Keep UI copy generic + the server
address configurable. See [PLAN.md](PLAN.md) §0.

## Repo layout (`arna`)
| Path | What | State |
|---|---|---|
| `backend/` | Rust **axum** signaling hub (WS: register / signal / ice) | ✅ works, dockerized |
| `core/` | Rust lib: signaling client + **webrtc-rs** P2P (`p2p` module) | ✅ verified |
| `poc/` | CLI to test signaling + P2P | ✅ verified |
| `agent/` | Rust **lib + headless bin**: capture (`scrap`) → **H.264** (`openh264`) → WebRTC video; input (`enigo`); consent | ✅ verified |
| `agent-desktop/` | **Tauri v2** wrap of `agent`: tray + **consent popup window** (Vue) | ✅ builds + runs |
| `console/` | **Vue 3 + Vite** app, wrapped in **Tauri v2** (`console/src-tauri/`) | ✅ desktop wrap builds + launches |
| `infra/` | docker-compose (Caddy + backend + coturn), `.env.example` | scaffold |
| `.github/workflows/` | `backend`, `core`, `console`, `release` (tag-driven) | ✅ green |
| `docs/` | `PLAN.md`, `PROTOCOL.md`, `RELEASING.md`, this file | — |

> `backend` and the Tauri shells (`console/src-tauri`, `agent-desktop/src-tauri`)
> are **excluded** from the Cargo workspace (`members = core, poc, agent`): backend
> builds standalone in Docker, and the Tauri CLI manages the shells as their own
> crates. `agent-desktop/src-tauri` depends on the `agent` crate by path.

## Phase progress
| Phase | What | Status |
|---|---|---|
| 0 | Backend signaling + infra + CI + release pipeline | ✅ done |
| 1a | Peer discovery via signaling | ✅ verified |
| 1b | WebRTC P2P data channel (SDP/ICE over signaling) | ✅ verified |
| 1c | **See the remote screen** (capture→JPEG→WebRTC→browser) | ✅ verified (user saw it live) |
| 2 | **Remote control** (mouse/keyboard → `enigo`) | ✅ built; user verifying |
| 3a | **Consent + SSO auth** (connect_request → popup/policy → accept; HS256 ticket) | ✅ built + smoke-tested |
| 3b | **Tauri** wrapping — console desktop ✅; agent tray + consent popup ✅ | ✅ done |
| 4a | **H.264 video track** (OpenH264) replaces JPEG-over-data-channel | ✅ verified (decodes in Chrome) |
| 4b | **File transfer** console → agent (drag-drop → `~/ArnaRemote/Incoming`) | ✅ verified (byte-identical) |
| 4c | **Live chat** both ways (console panel ↔ agent terminal / desktop chat window) | ✅ verified two-way |
| 4d | **File download** agent → console (operator picks via native dialog) | ✅ verified (byte-identical) |
| later | SSH/FTP, fleet, meet; multi-monitor; coturn | ⏳ |

## Run it locally (Windows)
```bash
cd d:\Siri-apps\arna-remote
cargo build --release -p arna-agent
# 1) backend
cargo run --manifest-path backend/Cargo.toml          # ws://127.0.0.1:8081/ws , GET /health
# 2) agent — headless (auto-consent, fast for 2-machine testing) ...
cargo run -p arna-agent --release -- ws://127.0.0.1:8081/ws agent-1
#    ... OR the desktop app: tray + a real Accept/Decline consent popup
cd agent-desktop && npm install && npm run tauri:dev
# 3) console — browser (fast dev) OR desktop app
cd console && npm install && npm run dev              # http://localhost:4310  (Agent id = agent-1)
cd console && npm run tauri:dev                       #   or the Tauri desktop window
```
Website: `cd d:\Siri-apps\arna-website && npm run dev` (port 4300).

## How it works (current)
- **Consent gate first** (Phase 3a): console sends `connect_request{to,ticket}`;
  backend verifies the SSO ticket (or admits openly if `ARNA_SSO_SECRET` unset) and
  forwards `incoming_request` to the agent; agent decides (policy/popup), replies
  `signal{kind:"consent",accepted,code}`. Only on accept does the console send the
  WebRTC offer, and the **agent only answers offers from admitted peers** (approval
  revoked on disconnect → reconnect re-asks). See [PROTOCOL.md](PROTOCOL.md) §1/§4.
- Signaling: Console (browser) is the **offerer**; agent is the **answerer**.
- Agent builds a **fresh RTCPeerConnection per viewer** (reconnect + multi-viewer safe).
- **Screen = a real H.264 video track** (agent captures → downscales ≤1280w →
  OpenH264 → WebRTC track; browser plays it in `<video>`). Replaced JPEG frames.
- **input** data channel (viewer → agent, JSON mouse/key events; injected via `enigo`).
- **files** data channel (both ways): **upload** — drag a file onto the console (or
  "Send file") → agent saves to `~/ArnaRemote/Incoming`. **download** — console
  "Download" button → the operator picks a file (native dialog; `ARNA_DOWNLOAD_FILE`
  for the headless agent) → streams back → browser saves it.
- **chat** data channel (both ways): console chat panel ↔ agent. The headless
  agent chats via the terminal; `agent-desktop` opens a chat window on the first
  message. `node scripts/chat-check.mjs` verifies both directions.
- Domains (planned): `arna.ifleon.com` site · `api.arna.ifleon.com` backend ·
  `turn.arna.ifleon.com` coturn. Console launch deep link: `arnaremote://`.

## Gotchas (important)
- **Run the agent with `--release`** (debug capture is slow).
- **Serve the Console over http** (Vite `npm run dev`), never `file://` (WS blocked).
- **mDNS** in `core/p2p.rs` is now `MulticastDnsMode::QueryOnly` (agent resolves
  the browser's `.local` candidates but advertises its own real IPs). `QueryAndGather`
  put a 2nd mDNS responder on the box and made same-machine ICE flaky.
- **H.264 send gotchas** (cost hours — don't undo): the agent registers a *single*
  H.264 codec and calls `add_track` **before** `set_remote_description`, or webrtc-rs
  never binds the sender → zero RTP. `block_in_place` in the encode loop stalled it
  after one frame; encode inline. See `core/p2p.rs` + `agent/src/lib.rs`.
- **Single-machine testing:** Chrome hides its host IP behind mDNS, so loopback ICE
  is flaky (~1 in 3 fails to connect). Launch Chrome with
  `--disable-features=WebRtcHideLocalIpsWithMdns` for reliable local tests; real
  two-machine / coturn setups don't have this issue. Verify script:
  `scripts/video-check.mjs` (needs `playwright` + Chrome).
- Testing on **one machine** = the remote cursor fights your real cursor; use two PCs.
- Input wire format (over `input` channel): `{t:"m",x,y}` move (normalized 0..1),
  `{t:"d"/"u",b}` button, `{t:"w",dy}` wheel, `{t:"kd"/"ku",k}` key.

## Consent + auth config (Phase 3a)
- **Headless agent** `ARNA_CONSENT` = `accept` (default) · `prompt` (terminal y/N)
  · `decline`. The **desktop agent** (`agent-desktop`) ignores this and shows the
  real Accept/Decline popup instead (`ARNA_BACKEND` / `ARNA_AGENT_ID` configure it).
- **Backend** `ARNA_SSO_SECRET` (HS256 secret; unset = open dev mode). When set:
  agents must present a token (`ARNA_AGENT_TOKEN`) to register, and consoles need a
  ticket to connect. For testing, `ARNA_DEV_TICKETS=1` →
  `GET /dev/ticket?role=agent&id=<id>` (agent token) or `?agent=<id>&name=<n>`
  (console ticket). Console has an optional **Ticket** field.
- **Security model + secure-deploy checklist:** see [SECURITY.md](SECURITY.md).
  Hardening done: device auth (no impersonation), role-aware routing, rate limiting.
  Verify: `node scripts/smoke-auth.mjs` against an SSO backend.
- Smoke test (signaling-level, no GUI): `node scripts/smoke-consent.mjs` (open
  mode) or `SSO=1 node scripts/smoke-consent.mjs` against an SSO-enabled backend.

## Next steps
1. **Security — finish hardening:** optional **require-code** consent mode; then the
   big one — **accounts + device ownership** (sign in, your devices belong to you,
   connect only to your own/shared devices). See [SECURITY.md](SECURITY.md).
2. **Accounts/identity backend:** SQLite, users, device registry, pairing — replaces
   the shared-secret tokens with per-user authorization.
3. **Bundle + ship:** configurable/remembered server address (hosted default +
   custom), `tauri build` installers, deploy backend + **coturn** so two machines
   across the internet connect reliably (current P2P/STUN is LAN-reliable only).
4. **More features:** fleet health + remote commands, clipboard sync, multi-monitor,
   SSH/FTP, meet.
5. **Phase 5 polish:** reconnect, run-as-service/SYSTEM (UAC), signed installers,
   deep link (`arnaremote://`), audit log.

Known limitations: view+control + files (both ways) + chat work. Security: device
auth + role routing + rate limiting done; **no user accounts yet** (token-based);
no audit log; consent is Accept-only (require-code planned). Two-machine Accept→
stream best confirmed on real machines; coturn not deployed (P2P/STUN, LAN-reliable).
