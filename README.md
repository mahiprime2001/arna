# Arna — Workspace Platform

**Lend your computer's power, not your computer.**

Arna lets a host share compute, storage, and selected resources with people they invite —
as isolated **workspaces**, not as remote control of their desktop. A workspace is a
place, not a connection: it has its own screen, input, clipboard, filesystem, and
processes. The host keeps using their machine the whole time, and always stays in control.

> **A workspace is a place, not a connection.**

## Status

**Design phase.** The product and its contract are settled and frozen; implementation is
sequenced and about to begin.

- ✅ [`docs/SPEC.md`](docs/SPEC.md) — the platform specification, **frozen v1.0**. Defines
  what a workspace *is* and what any implementation MUST guarantee. Implementation-free by
  design: it names no operating system, VM, or language.
- ✅ [`docs/adr/`](docs/adr/) — Architecture Decision Records (ADR-001..007): product
  scope, audience, hardware ownership, workspace identity, the platform-adapter
  architecture, the Windows rendering/input evidence, and the spec-before-implementation
  rule.
- ◐ [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — the component map, the adapter
  interface (each method cited to a spec section), and the build sequence.

## How it's built

The specification is the contract. Platform **adapters** implement it per OS; the layers
above the adapter boundary never name an operating system. Build order (see
[ADR-007](docs/adr/0007-specification-before-platform-integration.md) and
[ARCHITECTURE.md](docs/ARCHITECTURE.md)):

```
Spec → Architecture → Domain + Adapter interface → Mock adapter →
Manager → Policy → Runtime → Protocol → Windows/Linux/macOS adapters → Client
```

The mock adapter comes first, so the platform is built and tested with no OS involved. The
UI comes near last, by design.

## History

This repository previously held a remote-desktop application. That code is preserved in
git history and was retired in favour of the workspace-platform design above. Proven
pieces (low-latency capture, H.264 encoding, WebRTC transport, input injection) are
recoverable from history and will be reused as the streaming path per
[ARCHITECTURE.md](docs/ARCHITECTURE.md).

## License

See [LICENSE](LICENSE).
