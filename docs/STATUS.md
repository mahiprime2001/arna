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
wraps into **Tauri** desktop apps (Agent on machines, Console for the admin).

## Repo layout (`arna`)
| Path | What | State |
|---|---|---|
| `backend/` | Rust **axum** signaling hub (WS: register / signal / ice) | ✅ works, dockerized |
| `core/` | Rust lib: signaling client + **webrtc-rs** P2P (`p2p` module) | ✅ verified |
| `poc/` | CLI to test signaling + P2P | ✅ verified |
| `agent/` | Rust: screen capture (`scrap`) → JPEG (`image`) → WebRTC; input injection (`enigo`) | ✅ verified |
| `console/` | **Vue 3 + Vite** app (becomes the Tauri Console) | ✅ works |
| `infra/` | docker-compose (Caddy + backend + coturn), `.env.example` | scaffold |
| `.github/workflows/` | `backend`, `core`, `console`, `release` (tag-driven) | ✅ green |
| `docs/` | `PLAN.md`, `PROTOCOL.md`, `RELEASING.md`, this file | — |

> `backend` is **excluded** from the Cargo workspace (`members = core, poc, agent`)
> so its Docker image builds standalone.

## Phase progress
| Phase | What | Status |
|---|---|---|
| 0 | Backend signaling + infra + CI + release pipeline | ✅ done |
| 1a | Peer discovery via signaling | ✅ verified |
| 1b | WebRTC P2P data channel (SDP/ICE over signaling) | ✅ verified |
| 1c | **See the remote screen** (capture→JPEG→WebRTC→browser) | ✅ verified (user saw it live) |
| 2 | **Remote control** (mouse/keyboard → `enigo`) | ✅ built; user verifying |
| 3a | **Consent + SSO auth** (connect_request → popup/policy → accept; HS256 ticket) | ✅ built + smoke-tested |
| 3b | **Tauri** wrapping (agent tray + consent window; console desktop) | ⏳ next |
| later | VP8/H.264 **video track** (replace JPEG), files, chat, SSH/FTP, fleet, meet | ⏳ |

## Run it locally (Windows)
```bash
cd d:\Siri-apps\arna-remote
cargo build --release -p arna-agent
# 1) backend
cargo run --manifest-path backend/Cargo.toml          # ws://127.0.0.1:8081/ws , GET /health
# 2) agent (shares this screen)
cargo run -p arna-agent --release -- ws://127.0.0.1:8081/ws agent-1
# 3) console
cd console && npm install && npm run dev              # http://localhost:4310  (Agent id = agent-1)
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
- Two data channels: `screen` (agent → viewer, JPEG frames ~12fps) and `input`
  (viewer → agent, JSON mouse/key events; agent injects via `enigo`).
- Domains (planned): `arna.ifleon.com` site · `api.arna.ifleon.com` backend ·
  `turn.arna.ifleon.com` coturn. Console launch deep link: `arnaremote://`.

## Gotchas (important)
- **Run the agent with `--release`** (debug capture is slow).
- **Serve the Console over http** (Vite `npm run dev`), never `file://` (WS blocked).
- **mDNS is enabled** in `core/p2p.rs` (`MulticastDnsMode::QueryAndGather`) — required
  or Chrome↔webrtc-rs ICE gets stuck at "connecting".
- Testing on **one machine** = the remote cursor fights your real cursor; use two PCs.
- Input wire format (over `input` channel): `{t:"m",x,y}` move (normalized 0..1),
  `{t:"d"/"u",b}` button, `{t:"w",dy}` wheel, `{t:"kd"/"ku",k}` key.

## Consent + auth config (Phase 3a)
- **Agent** `ARNA_CONSENT` = `accept` (default) · `prompt` (terminal y/N) · `decline`.
- **Backend** `ARNA_SSO_SECRET` (HS256 secret; unset = open dev mode) and, for
  testing, `ARNA_DEV_TICKETS=1` → `GET /dev/ticket?agent=<id>&name=<n>` mints a
  5-min ticket. Console has an optional **Ticket** field (paste the dev JWT).
- Smoke test (signaling-level, no GUI): `node scripts/smoke-consent.mjs` (open
  mode) or `SSO=1 node scripts/smoke-consent.mjs` against an SSO-enabled backend.

## Next steps (Phase 3b+)
1. **Tauri wrap**: turn `agent/` and `console/` into Tauri apps. Console reuses the
   existing Vue frontend; Agent gets a tray + the real consent **popup window**
   (replaces the `ARNA_CONSENT` policy with `Accept/Decline` + code UI).
2. **Video track**: replace JPEG-over-data-channel with a real VP8/H.264 track.
3. Then: files, chat, SSH/FTP, fleet health + remote commands, meet.

Known limitations: view+control only; consent is **policy/terminal** until the
Tauri popup lands; no UAC/secure-desktop control (needs SYSTEM service); coturn not
deployed yet (P2P/STUN only).
