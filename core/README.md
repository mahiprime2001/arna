# core — shared Rust crate

The shared library both `agent` and `console` depend on, so the common logic is
written once.

**Responsibilities**
- Signaling client (WSS): register, presence, connect request/accept, SDP + ICE.
- WebRTC wiring (`webrtc-rs`): peer connection, video track, data channels.
- Data-channel protocol: `input`, `files`, `chat`, `control` (see `../docs/PLAN.md` §7).
- File transfer: chunking, backpressure, progress.
- Auth/token helpers: SSO ticket + agent token.

**Status:** not yet implemented (begins Phase 1).
