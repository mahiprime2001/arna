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
| `backend/` | Signaling + device registry + messaging + SSO verification (Rust / axum) |
| `website/` | Public site at `arna.ifleon.com` (Vue 3 + Vite, served via nginx) |
| `infra/`   | Dockerized stack — Caddy (auto-HTTPS) + coturn + `docker-compose` |
| `.github/` | CI — backend, website, and Tauri build workflows |
| `docs/`    | `PLAN.md` (source of truth), `PROTOCOL.md` |

## Tech

Tauri · Rust (`axum` backend; `webrtc-rs`, `windows-capture`, `vpx-encode`, `enigo` apps) · Vue 3 + Vite (website) · coturn · WebRTC (DTLS-SRTP, end-to-end encrypted)

## Domains

| Subdomain | Service |
|---|---|
| `arna.ifleon.com` | Website (Next.js) |
| `api.arna.ifleon.com` | Backend signaling (WSS + REST) |
| `turn.arna.ifleon.com` | coturn TURN/STUN relay |

## Run the stack (on the VPS)

```bash
cd infra
cp .env.example .env                                   # set a real TURN secret
cp coturn/turnserver.conf.example coturn/turnserver.conf
docker compose up -d --build
```

Caddy obtains HTTPS certificates automatically once the A records point at the VPS.
Open the coturn UDP ports (3478, 5349, 49152–65535) on the firewall + provider panel.

## How it's built

Phased, riskiest-first — see the milestone table in [docs/PLAN.md](docs/PLAN.md). Each
phase is verified before the next. The billing app (and any future app) launches a
session via the `arnaremote://` deep link with an SSO handoff ticket.

---

Part of the **Siri ecosystem**. Proprietary — see [LICENSE](LICENSE).
