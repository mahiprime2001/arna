# Arna — Security model

How Arna keeps remote sessions safe, and how to deploy it securely. This is the
"hardening" baseline; full **accounts + device ownership** come next (see PLAN §8).

## What's protected

- **Media is end-to-end encrypted.** Screen video, files, and chat travel over
  WebRTC (DTLS-SRTP) directly between the two machines. The signaling server only
  brokers the introduction — it never sees session content. Even when a session is
  relayed through coturn, the relay forwards only encrypted bytes.
- **Connections require consent.** The agent shows an Accept/Decline popup with a
  one-time 6-digit code before any session starts, and can end it at any time. The
  agent only answers WebRTC offers from peers it has admitted; approval is revoked
  on disconnect, so a reconnect must re-consent.
- **Optional require-code mode.** In `code` mode the agent admits a caller only
  after they type the 6-digit code the operator reads out (3 tries, then refused) —
  guards against blind-accept social engineering. `node scripts/smoke-code.mjs`.

## Hardening (implemented)

- **Device authentication.** When `ARNA_SSO_SECRET` is set, an agent must present a
  valid HS256 token (`sub` = its id, `role` = `agent`) to register. Without it,
  registration is denied — nobody can impersonate a device id.
- **Role-aware routing.** The backend only routes a `connect_request` to a peer
  that actually registered as an agent, and a console can't take over an id a live
  agent holds. (Closes interception + slot-hijacking.)
- **Console authorization (SSO ticket).** A `connect_request` carries a short-lived
  HS256 ticket scoped to the target agent; the backend verifies it before ringing
  the agent. An agent token can't be reused as a console ticket.
- **Rate limiting.** Per-connection caps: an overall message-flood limit (abusive
  sockets are dropped) and a `connect_request` throttle (one console can't spam an
  agent's consent popup or brute-force ids).

## Deploy securely (checklist)

1. **TLS everywhere.** Put the backend behind **Caddy** (auto-HTTPS) and have the
   apps connect over **`wss://`** — never plain `ws://` across a network. coturn
   runs with TLS too. The apps accept any URL, so point them at your `wss://` host.
2. **Set `ARNA_SSO_SECRET`** (a strong random secret). Unset = OPEN dev mode with no
   auth — fine for localhost testing, never for a real deployment.
3. **Provision agent tokens.** Each agent gets an `ARNA_AGENT_TOKEN` (an HS256 token
   with its id + `role: agent`). In production these come from your identity service;
   for testing, `/dev/ticket?role=agent&id=<id>` mints one.
4. **Never enable dev tickets in prod.** `/dev/ticket` only works with
   `ARNA_DEV_TICKETS=1` — leave it off in production.
5. **Short-lived console tickets.** Issue per-session, agent-scoped tickets (your
   identity service / the SSO handoff), not long-lived ones.

## Accounts & device ownership (implemented, backend)

- **Accounts.** SQLite store + email/password: `POST /auth/signup` / `POST /auth/login`
  (Argon2 hashing) return a 7-day **session token**. Requires `ARNA_SSO_SECRET`.
- **Device registry.** A logged-in user registers a device — `POST /devices`
  (Bearer session) records it under the user and returns the **agent token** that
  device uses to come online. `GET /devices` lists the user's devices.
- **Ownership enforced on connect.** A `connect_request` carrying a **session token**
  is allowed only if that user **owns** the target device (checked against the
  registry); otherwise it's denied ("you don't have access" / "device not
  registered"). The legacy agent-scoped SSO ticket still works for the handoff path.
- Verify: `node scripts/smoke-accounts.mjs` and `node scripts/smoke-devices.mjs`.

## Known gaps

- **Apps don't have the login/device UIs yet** — the backend enforces ownership, but
  the desktop apps still use env-provided tokens; login + a device list come next.
- No device **sharing** between users yet (only the owner can connect).
- No audit log; coturn not deployed.
- No audit log of who connected to what, when.
- coturn not deployed yet (P2P/STUN only; LAN-reliable).

## Verify it

`node scripts/smoke-auth.mjs` against an SSO-enabled backend checks device auth,
role-aware routing, anti-hijack, and rate limiting.
