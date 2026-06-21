# Arna Wire Protocol

> **Stub.** Filled in across Phases 0–4. The v1 data-channel protocol
> (`input`, `files`, `chat`, `control`) is specified in [PLAN.md](PLAN.md) §7;
> signaling and identity in §5–§6.

## 1. Signaling (WSS) — defined in Phase 0
Messages brokered by the backend between Console and Agent: `register`,
`connect_request`, `incoming_request`, `accept`/`decline`, `offer`, `answer`,
`ice`, `hangup`. (Schema TBD.)

## 2. Data channels — see PLAN.md §7
- `input` — mouse/keyboard (Console → Agent)
- `files` — chunked transfer (both ways)
- `chat`  — live messages (both ways)
- `control` — monitor list, quality, clipboard, session end

## 3. File transfer framing — defined in Phase 4

## 4. Identity / SSO ticket — defined in Phase 3
