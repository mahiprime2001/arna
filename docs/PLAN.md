# Arna — Self-Hosted All-in-One Platform (Technical Plan)

Arna is an internal **all-in-one platform** for the store network — remote
support, fleet monitoring, team chat, video meetings, and file sharing — all on a
single WebRTC engine, self-hosted, end-to-end encrypted, built on our Tauri/Rust
stack. v1 ships the remote-support core; every other tool is a layer on the same
foundation. The billing app (and any future app) launches it with one click.

Status: PLAN — no code written yet. Source of truth for the build.
See §12 for the full product vision & module roadmap.

---

## 1. Final decisions (locked)

- **Route:** build fresh on Tauri + proven Rust crates (not a RustDesk fork).
- **Product shape:** **standalone product**, its own repo — not embedded in billing-app.
- **Apps:** **two separate Tauri apps** — `agent` (stores) and `console` (admin) —
  sharing a **`core` Rust crate** so common logic is written once.
- **Identity:** Arna Remote **owns its own users/devices**, with an **SSO handoff**
  so the billing app can launch a connection with no second login.
- **Server:** its **own lightweight backend** (signaling + registry + messaging) +
  **`coturn`** relay on our VPS.
- **Streaming:** **end-to-end encrypted, peer-to-peer** (server never sees the screen).
- **Messaging/files:** **hybrid** — backend for persistent/offline, P2P for live/large.

---

## 2. Goals & non-goals

**Goals**
- Admin clicks **Connect** (from the Console, or deep-linked from billing-app).
- Store shows a small **consent popup** (doesn't block work) with a one-time
  **code** + **Accept/Decline**.
- Console gets **full control** (mouse, keyboard, multi-monitor).
- **Drag-and-drop file transfer** both ways (native local folders — desktop app).
- **Chat / messaging** between admin and stores (and store↔store).
- **Self-hosted**, lightweight, no banners/limits/fees.
- Reusable: any app can launch it via deep link + SSO ticket.

**Non-goals (v1)**
- Mobile control, session recording, multi-org console.
- Controlling Windows UAC / secure desktop (needs SYSTEM service — Phase 5).

---

## 3. The "smart build" principle

We assemble proven Rust crates under our own UI — no codecs from scratch:

| Hard piece | Library |
|---|---|
| Screen capture (Windows) | `windows-capture` (WGC); fallback `scrap` |
| Video encode | `vpx-encode` (VP8); browsers/decoders handle VP8 natively |
| Transport: video + data + NAT traversal | `webrtc` (`webrtc-rs`) |
| Mouse/keyboard injection | `enigo` |
| Relay + STUN | `coturn` on the VPS |
| Signaling + registry + messaging | our own small backend (Rust `axum`, or Node) |

---

## 4. Architecture

```
   ┌──────────────────────────┐         ┌──────────────────────────┐
   │   CONSOLE app (Tauri)    │         │    AGENT app (Tauri)     │
   │   you / admin            │         │    store PC (background) │
   │   - view store screen    │         │    - capture (WGC)       │
   │   - send mouse/keyboard  │         │    - encode VP8          │
   │   - file drag-drop (local)│        │    - inject input (enigo)│
   │   - chat                 │         │    - consent popup       │
   │        ▲  uses core ▲    │         │     ▲  uses core ▲       │
   └────────┼─────────────────┘         └────────┼─────────────────┘
            │  WSS signaling + WebRTC (DTLS-SRTP, P2P)│
            └──────────────────┬──────────────────────┘
                               │
                 ┌─────────────▼──────────────┐
                 │            VPS             │
                 │  Arna Remote backend       │  signaling + device
                 │  (signaling/registry/chat) │  registry + messaging
                 │  coturn (relay/STUN)       │
                 └────────────────────────────┘
```

- **Direct (P2P):** screen flows Console ↔ Agent directly; backend only brokers.
- **Relayed:** if NAT blocks P2P, `coturn` forwards the *encrypted* stream.
- **Encryption:** DTLS-SRTP end-to-end; WSS/TLS signaling; TLS coturn.

---

## 5. Components

### 5.1 `core` (shared Rust crate)
Written once, used by both apps:
- Signaling client (WSS): register, presence, request/accept, SDP + ICE exchange.
- WebRTC wiring (`webrtc-rs`): peer, video track, data channels.
- Data-channel protocol: `input`, `files`, `chat`, `control` (see §7).
- File transfer (chunking + backpressure + progress).
- Crypto/auth helpers (token handling).

### 5.2 `agent` (Tauri app — store PCs)
- `core` + capture (`windows-capture`) + encode (`vpx-encode`) + input (`enigo`).
- **Consent popup**: small always-on-top window — "Admin X wants to connect" +
  6-digit code + Accept/Decline. Non-modal; staff keep working.
- Tray icon, run-at-login (v1) → Windows **service/SYSTEM** (Phase 5).
- Config: backend URL, server key, `store_id`, agent token, optional unattended pw.

### 5.3 `console` (Tauri app — admin)
- `core` + a desktop UI: device list (your stores), **Connect**, the control
  window (`<video>` of the screen, mouse/keyboard capture), the **right-side file
  panel** (drag to/from real local folders), and **chat**.
- Native desktop = smooth control, true local drag-drop, multi-monitor.
- Can be opened directly, or deep-linked: `arnaremote://connect?store=<id>&ticket=<signed>`.

### 5.4 `backend` (its own service on the VPS)
- **Signaling**: brokers requests, consent results, SDP/ICE between peers.
- **Device registry**: which agents (stores) exist and are online.
- **Identity**: owns Arna Remote users/devices; verifies the **SSO handoff ticket**
  from billing-app (signed JWT) so Connect is one click, no second login.
- **Messaging**: stores persistent chat + file metadata for offline delivery.
- **TURN credentials**: mints short-lived `coturn` creds per session.
- Small DB (its own — e.g. Postgres/SQLite) for users/devices/messages.

### 5.5 `infra`
- `coturn` (TLS + time-limited HMAC creds), backend service, `systemd`/Docker.

---

## 6. Session flow (the exact UX)

```
1. Console (or billing-app deep link) -> Connect to store_id.
2. SSO: billing-app issues a signed ticket (existing admin login) OR console logs in.
3. Console -> backend: connect_request { store_id, ticket }.
4. Backend verifies + finds the online agent -> incoming_request to the agent.
5. Agent popup: "Admin X wants to connect" + 6-digit code + Accept/Decline.
      - Accept-only (default), or read code to admin, or both.
6. Agent -> backend: accepted.
7. WebRTC handshake brokered: offer/answer SDP + ICE candidates.
8. Media + data channels connect (P2P, or coturn relay).
9. Admin views, controls, transfers files, chats. Either side ends -> hangup.
```

Consent modes (configurable): Accept-only / Code / Both / Unattended (off by default).

---

## 7. Data-channel protocol (v1)

- `input` (console → agent): `mmove{x,y,mon}` (normalized), `mdown/mup{btn}`,
  `wheel{dx,dy}`, `kdown/kup{code}`. Agent scales to real resolution + monitor offset.
- `files` (both ways): `file_start{id,name,size}` → binary chunks → `file_end{id}`;
  backpressure via `bufferedAmount`; progress in UI.
- `chat` (both ways): `msg{id,text,ts}` for live chat during a session.
- `control` (both ways): monitor list/resolutions, quality/FPS, clipboard sync,
  Ctrl-Alt-Del request, session end.

Persistent chat + offline file metadata go through the **backend** (reusing the
notification system); P2P channels handle **live** chat + **large** transfers.

---

## 8. Security & identity

- Screen stream **end-to-end encrypted** (relay can't read it).
- **SSO handoff**: billing-app issues a short-lived signed ticket (existing admin
  login/roles) that the backend verifies → one-click Connect, no double login.
- Arna Remote **owns its users/devices**; standalone login also available.
- Each **agent provisioned with a signed token** + embedded server key (only our
  devices talk to our backend).
- Store **must Accept** by default, even in unattended-capable builds.
- WSS/TLS signaling; TLS coturn; short-lived TURN credentials.
- Audit log of who connected to which store, when (Phase 5).

---

## 9. Milestones (riskiest-first, each verified before the next)

| Phase | Deliverable |
|---|---|
| **0. Plumbing** | `backend` signaling skeleton + `coturn` on VPS (WSS/TLS, healthcheck) |
| **1. VIEW POC** | `agent` captures 1 monitor → VP8 → `console` shows it (hardcoded auth). Check latency/quality. |
| **2. CONTROL** | `input` channel + `enigo`; coordinate mapping; single monitor |
| **3. CONNECT/CONSENT** | registry, presence, request→popup→accept→code; SSO handoff + billing-app Connect button |
| **4. FILES + CHAT** | two-way drag-drop transfer + live chat (P2P) + persistent messaging (backend) |
| **5. HARDENING** | multi-monitor, reconnect, unattended pw, run-as-service, signed installer, perf (damage regions / HW encode), audit log |

---

## 10. Division of work

**You provide**
- VPS (8 GB / 100 GB Ubuntu — have it).
- Subdomain (e.g. `remote.ifleon.com`) + TLS cert (Let's Encrypt).
- Later: code-signing certificate for the agent/console installers.
- Decisions on the open questions below.

**I build**
- `core` crate, `agent` app, `console` app, `backend` service, `coturn` config —
  phased, each verified.

---

## 11. Decisions (locked)

1. **Domains** — website `arna.ifleon.com`, backend `api.arna.ifleon.com`,
   coturn `turn.arna.ifleon.com`; fronted by Caddy (automatic HTTPS).
2. **Default consent mode** — Accept-only, with the 6-digit code shown as a bonus.
3. **Backend** — **Rust `axum`** (single small binary, matches the stack).
4. **Backend DB** — **SQLite** to start (added with identity/messaging); Phase 0
   needs no DB. Migrate to Postgres later if scale demands.
5. **Incoming files** — fixed `ArnaRemote/Incoming` folder on the store PC + notice.
6. **Packaging** — everything **Dockerized** (`infra/docker-compose.yml`); desktop
   apps built/published via **GitHub Actions** (`.github/workflows/tauri.yml`).
7. **Website** — Next.js, dark UI, standalone Docker image.

---

## 12. Product vision & module roadmap (all-in-one)

Arna becomes the store network's internal super-app. Every module runs on the
**same WebRTC engine + backend**, so they share code and grow in layers — not five
separate products, one platform with five tools.

```
                    ┌──────────────  ARNA  ──────────────┐
                    │   one app · one WebRTC core         │
   ┌──────────┬──────────┬───────────┬───────────┬──────────────┐
   │ REMOTE   │ FLEET    │ CHAT      │ MEET      │ FILES        │
   │ control+ │ health + │ messaging │ video/    │ P2P + cloud  │
   │ screen   │ remote   │ broadcast │ audio +   │ transfer     │
   │          │ commands │           │ screenshr │              │
   └──────────┴──────────┴───────────┴───────────┴──────────────┘
        shared backend (signaling + registry + messaging) + coturn [+ SFU later]
```

### Modules
- **Remote** (v1 core): full control, multi-monitor, consent popup. (§1–§9)
- **Fleet**: per-store health dashboard (online / disk free / `_MEI` folder count /
  offline-queue size / poison items) + one-click remote commands (pull
  `offline_events.jsonl`, clear a poison queue item, free `_MEI` folders, restart
  the billing backend). Directly closes the monitoring gap behind the offline saga.
- **Chat**: live P2P chat in-session + persistent 1:1 messaging (reuses the
  notification system) + group broadcast to all stores.
- **Meet** (Teams-style): audio/video calls + screen share, reusing the same
  engine. Small calls run peer-to-peer (mesh) on the current stack; **large
  all-hands** adds a self-hosted **SFU (LiveKit)** on the VPS — only when needed.
- **Files**: P2P drag-drop (large/live) + backend-backed transfer (offline/history)
  + push-to-all-stores.

### Session/platform extras
- Clipboard sync, hear remote PC audio, session recording, multi-admin viewing.
- Unattended after-hours access, address book / store groups, roles + audit log.

### Build sequencing (by value, layered)
1. **Remote control + Fleet health** — solves the current real pain (see/fix stuck
   stores remotely). Highest ROI.
2. **Files + Chat** — quick wins on the same data channels.
3. **Meet (small/mesh)** — reuse the engine for calls + screen share.
4. **Scale/polish** — broadcast, recording, big meetings (SFU), audit/roles.

Ship the core, use it, grow it. Each module clicks onto the same rails — no
re-architecting.
