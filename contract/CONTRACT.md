# The Workspace Contract

**The canonical map.** This document defines how the pieces fit together — it
does **not** duplicate their details. Each capability and each core concept has
its own normative specification; this file points to them and states the rules
that bind them into one contract.

> A workspace is defined by a stable contract, not by the operating system or
> isolation technology used to implement it. Adapters are consumers of the
> contract, never authors of it.

Version: **v0.1** (`CONTRACT_VERSION`). See [core/capabilities.md](core/capabilities.md#versioning).

---

## Terminology

| Term | Meaning |
|------|---------|
| **Workspace** | An isolated execution environment — a *place*, not a connection. |
| **Capability** | Something a workspace *provides* (Applications, Clipboard, Storage, …). |
| **Access right** | Something a *member* may do (view, keyboard, clipboard-read, …). |
| **Adapter** | A platform's implementation of the contract. A consumer, never an author. |
| **Attestation** | An adapter's *evidence* of isolation; the engine evaluates it against policy. |
| **Conformance** | The pass/fail suite an adapter must satisfy to be a workspace at all. |

The frozen constitution is [SPEC.md](SPEC.md) (116 MUSTs). This contract is its
machine-checkable form; the Rust expression is the `engine/` crates.

## Architecture

```
Engine (orchestrator)  →  Capability interfaces  →  Adapter (per platform)  →  OS
        │                        │                         │
   owns POLICY            mechanical only            declares capabilities,
   (auth, isolation,      (no policy)                attests isolation,
   lifecycle, events)                                maps failures into the contract
```

The engine knows the contract; it never knows an OS. Everything platform-specific
lives below the adapter boundary.

## The core concepts

Each is normative in its own file:

- **[core/workspace.md](core/workspace.md)** — what a workspace is; the lifecycle
  state machine (§5); persistence.
- **[core/identity.md](core/identity.md)** — every persistent object has a stable
  contract identity (WorkspaceId, ResourceId, WindowId, …).
- **[core/capabilities.md](core/capabilities.md)** — the capability model,
  negotiation, maturity, the per-capability lifecycle, and versioning.
- **[core/permissions.md](core/permissions.md)** — roles, access rights, and the
  `Authorizer` policy interface (§4).
- **[core/errors.md](core/errors.md)** — the closed error vocabulary. Adapters map
  failures into it; they never invent errors.
- **[core/events.md](core/events.md)** — the observable event envelope. Adapters
  populate it; they never invent events.

## The capabilities

Normative per capability; the engine runs each one's conformance suite only for
adapters that declare it. See [capabilities/README.md](capabilities/README.md) for
the maturity table.

- [capabilities/clipboard.md](capabilities/clipboard.md) — Draft (§9)
- [capabilities/storage.md](capabilities/storage.md) — Draft (§8)
- Applications, Windows — Stable (specified in-engine + core conformance)

## The rules that bind it together

1. **Isolation is not a capability.** It is the mandatory core; an adapter that
   can't attest it does not run workspaces (SPEC §18.3). No partial tier.
2. **Capabilities are declared, negotiated, never assumed.** The engine asks
   "does this workspace provide X?", never "am I on platform Y?" (§18.2).
3. **The engine owns policy; adapters are mechanical.** Authorization and
   isolation-evaluation live in the engine, behind interfaces (`Authorizer`,
   `IsolationPolicy`). An adapter reads/writes; it never decides who may.
4. **One error vocabulary, one event envelope.** Adapters map failures into the
   error set and populate the event envelope; they invent neither.
5. **Stable identity everywhere.** Every persistent object has an unguessable,
   immutable id; deleted ids never resolve or reuse.
6. **Conformance is the definition of conforming.** `run_all(adapter)` — the core
   suite plus one suite per declared capability — is what an adapter must pass.
   The mock is the reference implementation.
7. **Platform independence.** Changing the implementation beneath a workspace
   must not change what the workspace *means* (SPEC §18.4).

## Conformance in one line

```rust
wse_conformance::run_all(MyAdapter::new).assert_ok();
```

The Windows, Linux, and macOS adapters will each be exactly that line. No adapter
gets special tests; none authors the contract.
