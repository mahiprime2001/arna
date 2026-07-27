# WSE — Architecture

**Status:** draft for review · implements `VISION.md` · answers to `SPEC.md`

This document structures *how* WSE is built. It is deliberately mechanism-plural: the same
contract is satisfied by different adapters using different technologies, and that is the
point.

---

## 1. The layered stack

```
                         WSE Desktop            (Flutter)  — what the user touches
                              │
                       Workspace Manager                   — workspaces, membership, roles, lifecycle
                              │
                       Workspace Runtime                   — the capability services (below)
                              │
        ┌──────────┬──────────┼──────────┬──────────┬──────────┐
        ▼          ▼          ▼          ▼          ▼          ▼
     Storage   Clipboard   Network    Devices   Permissions  Windows   …   (Capability Contract)
        │          │          │          │          │          │
        └──────────┴──────────┼──────────┴──────────┴──────────┘
                              │
                       Workspace Engine          (Rust)     — lifecycle, isolation, streaming, sessions
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
   Windows Adapter       Linux Adapter        macOS Adapter  (Rust)
        │                     │                     │
        ▼                     ▼                     ▼
    Operating System     Operating System     Operating System
```

The dashed capability row is the **contract**. Everything above it is written once against the
contract; everything below it is an adapter's private business.

---

## 2. The contract is the center of the repository

```
contract/            ← THE PRODUCT: the Workspace Capability Contract
  SPEC.md               frozen constitution (116 MUSTs)
  VISION.md             the idea (why)
  ARCHITECTURE.md       this file (how)
  CONTRACT.md           the capability contract — exact semantics per capability
  conformance/          the pass/fail suite every adapter MUST pass
  adr/                  decision records
engine/              ← Rust: Workspace Engine + adapters (implements the contract)
  crates/contract/      the contract expressed as Rust traits + capability types
  crates/core/          domain types (Workspace, State, Role, Grant) + the adapter trait
  crates/adapter-mock/  in-memory adapter — proves the layers with zero OS
  crates/adapter-windows/  wraps WSL2 / Hyper-V (ports the proven Go work)
  crates/adapter-linux/    namespaces + native display
  crates/adapter-macos/    Virtualization.framework
desktop/             ← Flutter: the WSE Desktop client (canvas, launcher, management)
services/            ← Go: identity, friends, invites, signaling (existing backend, kept)
sdk/                 ← Workspace SDK for Tier-2 workspace-native apps
client/              ← React: the existing social app (Arna) — kept working, converges later
```

**Language rationale.** Rust for the engine/adapters (systems-level, memory-safe, the layer
that touches isolation). Flutter for the desktop client (one native UI across host + guest).
Go for the cloud backend (identity, signaling — already built and working). React stays as the
existing social app until Flutter converges; nothing that works is thrown away first.

---

## 3. The Capability Contract has two layers

Per `VISION.md` Pillar 3, and grounded in `SPEC.md §18.2/§18.3`:

- **Mandatory core (MUST, identical everywhere).** Isolation and the §18.3 invariants:
  a workspace cannot see the host's filesystem beyond grants, cannot see host processes,
  cannot reach the local network, cannot reach host-attached camera/mic, and input/display
  are its own. An adapter that cannot enforce these **does not conform and refuses to run** —
  there is no partial-isolation tier.
- **Declared capabilities (MAY differ, never faked).** GPU acceleration, live-memory snapshot
  across Save, USB passthrough, display hot-plug, audio, latency class, and **native-app
  support** (Tier-1). Each adapter declares what it provides; undeclared means absent.

Conformance is therefore two suites: a **core suite** all adapters must pass, and
**capability suites** gated by what an adapter declares.

---

## 4. The versioned cost table

An abstraction that hides cost is dishonest. This table is **dated** and lives in the ADR that
records it; it is expected to change as platforms evolve. It never belongs on the critical
path — the roadmap is built on the "today" rows only.

| Adapter | Mechanism (today) | Isolation | Startup | Memory | Native apps | Eng. cost |
|---|---|---|---|---|---|---|
| Linux | namespaces + cgroups | strong | very fast | low | Linux | low |
| Windows | WSL2 / Hyper-V (VM) | strong | medium | high (GB) | Linux (Wine for some Win) | medium |
| Windows *(future)* | native desktop container | strong | fast | low | Windows | very high / not yet possible |
| macOS | Virtualization.framework | strong | medium | medium | macOS/Linux | medium |

_As of 2026-07-27. See `adr/0008-runtime-abstraction.md`._

The "Windows (future)" row is **upside, not plan.** WSE ships on the VM row and inherits the
native row for free if and when the OS provides it.

---

## 5. What is proven today (mechanism reality)

The current Windows adapter is a VM (WSL2), and it is real — verified end to end on the dev
machine:

- Host capability probe (can this machine isolate? refuse if not — §18.3).
- Create a sealed workspace, harden it, and **verify the seal every start** (no host drives,
  no host `.exe`) or refuse to run.
- Run apps inside it and stream the canvas to a browser (noVNC).
- Apps as draggable windows, snap/tile like Windows.
- Debian base (glibc) so Wine can run some Tier-1 Windows `.exe` later.

This lives in `services/wsl/` (Go) and is the **reference adapter / bridge**: the Rust
`adapter-windows` must reach parity on the conformance suite before it replaces it.

Known honest gaps in the VM adapter today (both structurally removed by a purpose-built guest
image later): local-network blocking (§13.2) is not yet enforced by default WSL2 networking;
the in-guest lockdown is configuration a root user inside could rewrite.

---

## 6. Build order (each step gated by conformance)

0. **Freeze `CONTRACT.md` v1 + conformance suite.** Nothing downstream is real until an
   adapter can be *tested* against the contract.
1. **Rust engine core + mock adapter** → passes the core suite with zero OS involvement.
2. **Rust Windows adapter** → ports `services/wsl`, passes the core suite + declares its
   capabilities honestly.
3. **Flutter desktop shell** → canvas, launcher, workspace management, driving the engine.
4. **Go backend integration** → identity, invites, signaling → collaboration across machines.
5. **SDK + first Tier-2 app**.

Cross-cutting, scheduled where they fit: Wine (Tier-1 Windows apps), TURN (cross-network
calls), Linux/macOS adapters, the purpose-built guest image (closes §13.2 and the config gap).

---

## 7. The invariant to protect

Across every implementation and every change of implementation over time (`SPEC.md §18.4`):
workspace identity, the meaning of policies and grants, roles and authority, and state/
lifecycle transitions **mean the same thing**. Parity says the same things are *available*;
independence says they *mean the same*. The adapters, runtimes, and mechanisms are free to
change. The contract is the part that must not.
