# backend — signaling + registry + messaging

Arna's own lightweight service. It owns Arna's identity, with an SSO handoff so the
billing app can launch a connection without a second login.

- **Signaling:** brokers connect requests, consent results, SDP/ICE between peers.
- **Device registry:** which agents (stores) exist and are online.
- **Identity:** owns Arna users/devices; verifies the SSO ticket from billing-app.
- **Messaging:** persistent chat + file metadata for offline delivery.
- **TURN credentials:** mints short-lived `coturn` creds per session.

**Open choices** (PLAN.md §11): language (Rust `axum` vs Node), DB (SQLite vs Postgres).
**Status:** begins Phase 0 (signaling skeleton).
