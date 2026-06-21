# infra — deployment

Target: the existing **8 GB / 100 GB Ubuntu VPS**.

- **coturn** — TURN/STUN relay; TLS + short-lived HMAC credentials.
- **backend** service unit — `systemd` and/or Docker Compose.
- *(Later)* **LiveKit SFU** — only when large Meet calls are needed.

`turnserver.conf` is git-ignored; commit a `turnserver.conf.example` instead.

**Status:** coturn + backend skeleton begin Phase 0.
