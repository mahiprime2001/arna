# Arna

Arna is the **Siri ecosystem's** self-hosted, all-in-one operations platform for the
store network — **remote support, fleet monitoring, team chat, video meetings, and
file sharing** — built on a single WebRTC engine. End-to-end encrypted, self-hosted,
and fully owned.

> **Status:** scaffolding. v1 ships the remote-support core; every other module is a
> layer on the same foundation. Full spec: [docs/PLAN.md](docs/PLAN.md).

## Modules (one app, one engine)

| Module | What it does |
|---|---|
| **Remote** | Full remote control of a store PC — screen, mouse, keyboard, multi-monitor |
| **Fleet**  | Per-store health (disk, `_MEI`, offline-queue, status) + one-click remote commands |
| **Chat**   | Live + persistent messaging, broadcast to all stores |
| **Meet**   | Teams-style audio/video calls + screen share |
| **Files**  | P2P drag-drop + backend-backed transfer |

## Repository layout

| Path | Purpose |
|---|---|
| `core/`    | Shared Rust crate — protocol, WebRTC wiring, data channels, file/chat |
| `agent/`   | Tauri app installed on **store PCs** (capture, input injection, consent popup) |
| `console/` | Tauri app for **admins** (control, files, chat, meet) |
| `backend/` | Signaling + device registry + messaging + SSO verification |
| `infra/`   | coturn relay + deployment |
| `docs/`    | `PLAN.md` (source of truth), `PROTOCOL.md` |

## Tech

Tauri · Rust (`webrtc-rs`, `windows-capture`, `vpx-encode`, `enigo`) · coturn · WebRTC (DTLS-SRTP, end-to-end encrypted)

## How it's built

Phased, riskiest-first — see the milestone table in [docs/PLAN.md](docs/PLAN.md). Each
phase is verified before the next. The billing app (and any future app) launches a
session via the `arnaremote://` deep link with an SSO handoff ticket.

---

Part of the **Siri ecosystem**. Proprietary — see [LICENSE](LICENSE).
