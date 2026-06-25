# Arna Wire Protocol

> Filled in across Phases 0–4. The v1 data-channel protocol (`input`, `files`,
> `chat`, `control`) is specified in [PLAN.md](PLAN.md) §7; signaling and identity
> in §5–§6. Phases 0–3 are implemented below.

## 1. Signaling (WSS) — Phases 0 + 3

JSON messages, tagged by `type`. The backend is a relay + a thin consent/auth
gate; it never sees media.

**Console/Agent → backend**
```jsonc
{ "type": "register", "role": "agent|console", "id": "<peer-id>" }
{ "type": "connect_request", "to": "<agent-id>", "ticket": "<jwt?>" }  // console only
{ "type": "signal", "to": "<peer-id>", "data": { ... } }              // opaque (WebRTC + consent)
{ "type": "ping" }
```

**Backend → Console/Agent**
```jsonc
{ "type": "registered",       "id": "<peer-id>" }
{ "type": "incoming_request", "from": "<console-id>", "name": "<admin>" }  // to the agent
{ "type": "request_denied",   "to": "<agent-id>", "reason": "..." }       // to the console
{ "type": "signal",           "from": "<peer-id>", "data": { ... } }
{ "type": "peer_offline",     "to": "<peer-id>" }
{ "type": "error",            "message": "..." }
{ "type": "pong" }
```

### Connect + consent handshake (Phase 3)
```
Console                         Backend                          Agent
  register(console) ───────────►│                                  │
  connect_request{to,ticket} ──►│ verify ticket (if SSO on)        │
                                │  ├─ bad/expired ─► request_denied │
                                │  └─ ok ─────────► incoming_request{from,name}
                                │                                  │ popup / policy
                                │       signal{kind:"consent",...}  │ + 6-digit code
  ◄───────────────────────────── (relayed) ◄──────────────────────│
  if accepted: signal{kind:"offer"} ─────────────────────────────►│ (answered only
  ◄── signal{kind:"answer"} / {kind:"ice"} (WebRTC) ──────────────►│  if admitted)
```

`signal.data.kind` values: `consent` (`{accepted, code?, reason?}`), `offer`
(`{sdp}`), `answer` (`{sdp}`), `ice` (`{candidate}`). The agent only answers an
`offer` from a peer it has admitted; approval is revoked when the session ends, so
a reconnect requires fresh consent.

## 2. Media + data channels — see PLAN.md §7
- **screen video** — a real **H.264 WebRTC video track** (agent → console). The
  agent captures the primary display, downscales to ≤1280-wide (even dims),
  encodes with OpenH264 (one encoder per viewer), and writes samples to the
  track. The browser adds a `recvonly` video transceiver and plays it in a
  `<video>`. *(Replaced the old JPEG-over-data-channel.)*
- `input` — mouse/keyboard data channel (console → agent). Wire format:
  `{t:"m",x,y}` move (normalized 0..1), `{t:"d"/"u",b}` button,
  `{t:"w",dy}` wheel, `{t:"kd"/"ku",k}` key.
- `files` — file transfer (console → agent, Phase 4b). Text control frames +
  binary chunks: `{t:"file_start",id,name,size}` → binary chunks (~16 KB, throttled
  by `bufferedAmount`) → `{t:"file_end",id}`. The agent saves to
  `~/ArnaRemote/Incoming` (name sanitized, de-duplicated) and replies
  `{t:"file_done",name,bytes}`. *(Agent → console download: later.)*
- `chat` — live messages (both ways). *(Phase 4.)*
- `control` — monitor list, quality, clipboard, session end. *(later.)*

> **H.264 negotiation notes (webrtc-rs answerer):** the agent registers a
> **single** H.264 codec (`packetization-mode=1; profile-level-id=42e01f`) and
> builds its track from that exact capability, and it calls `add_track` **before**
> `set_remote_description` — otherwise webrtc-rs never binds the sender and emits
> no RTP. See `core/src/p2p.rs` (`h264_capability`, `make_api`, `answer_streaming`).

## 3. File transfer framing — defined in Phase 4

## 4. Identity / SSO ticket — Phase 3

The `connect_request.ticket` is an **HS256 JWT** minted by the billing app (or the
dev helper below). Claims:

```jsonc
{ "sub": "<admin display name>",   // shown in the agent popup
  "agent": "<agent-id?>",          // optional: pins the ticket to one agent
  "exp": 1750000000 }              // Unix seconds; enforced by the backend
```

The backend verifies the signature + `exp` (and `agent` if present) using
`ARNA_SSO_SECRET`. When that env var is **unset**, auth is **open** (dev mode):
any `connect_request` is admitted and the console is shown as `Console (<id>)`.

**Dev ticket helper** (never enable in production): with `ARNA_SSO_SECRET` set and
`ARNA_DEV_TICKETS=1`, `GET /dev/ticket?agent=<id>&name=<n>` returns
`{ "ticket": "<jwt>" }` valid for 5 minutes.

### Agent consent policy (Phase 3, headless)
`ARNA_CONSENT` selects how the agent answers `incoming_request` until the Tauri
popup lands: `accept` (default — auto-admit), `prompt` (terminal y/N), `decline`.
A one-time 6-digit code is generated and echoed to both sides regardless.
