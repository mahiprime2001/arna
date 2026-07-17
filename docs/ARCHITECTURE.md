# Workspace Platform Architecture

**Version:** 0.1 (Draft)
**Date:** 2026-07-16
**Implements:** [SPEC.md](SPEC.md) v1.0 (Frozen)
**Governed by:** [ADR-001..007](adr/)

> This document may name implementation (languages, WSL, compositors). [SPEC.md](SPEC.md)
> may not. The direction of authority is one-way: **architecture adapts to the spec, the
> spec never adapts to architecture** ([ADR-007](adr/0007-specification-before-platform-integration.md)).
> Every component below cites the spec sections it exists to satisfy. A component that
> cannot cite a section does not belong here.

---

## 1. The full picture

```
┌──────────────────────────────────────────────────────────────┐
│  CLIENT                          the member's own machine      │
│  display · input · clipboard · files · presence · chat/AV      │
└───────────────────────────────┬──────────────────────────────┘
                                 │  Workspace Protocol
                                 │  (display, input, clipboard,
                                 │   files, audio, presence, control)
┌───────────────────────────────┴──────────────────────────────┐
│  PLATFORM  (host machine, OS-agnostic — never names an OS)     │
│                                                                │
│   Manager ── owns workspaces, members, roles, lifecycle        │
│      │                                                         │
│   Policy Engine ── deny-by-default, two-layer grants, audit    │
│      │                                                         │
│   Runtime ── drives one workspace's lifecycle & I/O            │
│      │                                                         │
│   ┌──┴───────────────  Adapter interface  ──────────────────┐  │  ← the boundary
│   │  create · destroy · pause · resume · save · restore      │  │    (ADR-005)
│   │  launch · share · revoke · attach · limit · display · io │  │
│   └──┬───────────────────────────────────────────────────┬──┘  │
│      │                                                    │     │
│   Mock Adapter          Windows Adapter (WSL2)      Linux/mac   │
│   (no OS, for tests)    provisions · isolates ·     (later)     │
│                         compositor · streams                   │
└──────────────────────────────────────────────────────────────┘
                                 │
                          OS-provided primitives
                       (rented, never built — ADR-001)
```

**The load-bearing rule:** nothing above the adapter boundary may name an OS or mechanism.
Manager, Policy, Runtime, and Protocol speak only *workspace, member, grant, policy,
application, display*. `#[cfg(windows)]` above the boundary is a review failure
([ADR-007](adr/0007-specification-before-platform-integration.md)).

---

## 2. Components

| # | Component | Owns | Spec | Status | Risk | Reuse |
|---|-----------|------|------|--------|------|-------|
| C1 | **Domain types** | Workspace, Member, Role, State, Grant, Policy, Capability | §2–6 | design | 🟢 | new |
| C2 | **Adapter interface** | the boundary trait + capability negotiation | §18, ADR-005 | design | 🟢 | new |
| C3 | **Mock adapter** | a fake workspace, no OS | §18.2 | design | 🟢 | new |
| C4 | **Manager** | workspaces, membership, roles, lifecycle, join flow | §3–5, §15.2 | design | 🟡 | new |
| C5 | **Policy Engine** | deny-by-default, grants, two-layer intersection, audit | §6, §17 | design | 🟡 | new |
| C6 | **Runtime** | one workspace's lifecycle, app launch, I/O wiring | §5, §10, §11 | design | 🟡 | new |
| C7 | **Protocol** | display, input, clipboard, files, audio, presence, control | §9, §13.3, §14 | design | 🟡 | **the pipe** |
| C8 | **Windows adapter (WSL2)** | provision, isolate, compositor, capture, inject | §14, ADR-006 | partial evidence | 🔴 | agent code |
| C9 | **Client** | member UI: connect, view, control, dashboard, chat/AV | §14–16 | design | 🔴 | console (rewrite) |
| C10 | **Cloud services** | identity, friends, invites, signaling, workspace registry | §4, §15 | design | 🟡 | backend |

Status legend: **design** = spec answers it, code doesn't exist · **partial evidence** =
some measured, most not · **risk** 🟢 low 🟡 medium 🔴 high.

---

## 3. The adapter boundary (C2) — derived from spec, not from WSL

Each method exists to satisfy a spec obligation, cited. If a method can't cite one, it is
WSL leaking upward and must be removed.

```
trait WorkspaceAdapter {
    fn capabilities(&self) -> CapabilitySet;                 // §18.2

    // lifecycle — §5
    fn create(&self, def: &WorkspaceDef) -> Result<WorkspaceHandle>;   // §3, §5.1
    fn destroy(&self, w: &WorkspaceHandle) -> Result<()>;              // §5.5
    fn pause(&self, w) -> Result<()>;   fn resume(&self, w) -> Result<()>;  // §5.3.1
    fn save(&self, w) -> Result<()>;    fn restore(&self, w) -> Result<()>; // §5.3.2
    fn state(&self, w) -> WorkspaceState;                             // §5.1

    // applications — §10
    fn launch(&self, w, app: &CatalogApp) -> Result<AppHandle>;      // §10.1, §10.2

    // grants (host authority) — §6, all revocable while running §6.3
    fn share_path(&self, w, grant: &PathGrant) -> Result<()>;        // §8.4
    fn revoke_path(&self, w, id: GrantId) -> Result<()>;             // §6.3
    fn attach_device(&self, w, grant: &DeviceGrant) -> Result<()>;   // §12.1
    fn set_limits(&self, w, limits: &ResourceLimits) -> Result<()>;  // §7.4

    // I/O — routed to the client via the Protocol, never to the host
    fn displays(&self, w) -> Vec<DisplayHandle>;                     // §14.1, §14.3
    fn input(&self, w, ev: InputEvent, from: Role) -> Result<()>;    // §14.2, §4.3
    fn clipboard(&self, w, dir: Direction, data) -> Result<()>;      // §9.2, §4.6

    // observability — §7.5, §17
    fn usage(&self, w) -> ResourceUsage;                             // §7.5
}
```

**Contract every adapter owes** (not optional, not a capability — §18.3): deny-by-default
isolation (§3.4, §6, §8, §11, §13.2/3, §14.2). An adapter that cannot enforce these
**refuses to start** rather than returning a workspace that only looks isolated.

**Capabilities that MAY differ** (§18.2, declared honestly, never faked): GPU
acceleration, live display resize/hot-plug (§14.4), USB passthrough (§12.1), live-memory
snapshot across Save (§5.3.3), audio (§12.3).

---

## 4. Keeping the pipe — what today's code becomes

We rebuild the brain, keep the transport ([earlier decision, still holds]).

| Existing (`arna-remote`) | Becomes | Fate |
|--------------------------|---------|------|
| `agent/` capture (scrap) + H.264 (openh264) + input (enigo) + WebRTC | **C7 Protocol** media path + the WSL adapter's capture/inject | **reused** — proven, painful to re-solve |
| `agent/bubble.rs`, `winmon.rs` | — | **deleted** — bubble is dead (ADR-006) |
| `backend/` accounts + signaling (axum, SQLite) | **C10** identity + signaling; DB → Postgres, model → workspaces | **evolved** |
| `console/` (Vue) | **C9** Client | **rewritten** (React/Tailwind/ShadCN) |
| `docs/PROTOCOL.md` (JSON channels) | **C7** Protocol (Protobuf/QUIC + WebRTC media) | **evolved** |

The reused pieces move *below* the adapter boundary (they are how an adapter streams) or
*into* the Protocol. Nothing reused gets to define a layer above the boundary.

---

## 5. Target repository structure

A single Cargo workspace; the client is Tauri+React on top.

```
platform/
  domain/        C1  types only — no I/O, no OS. The shared vocabulary.
  adapter/       C2  the trait + CapabilitySet. Depends on domain only.
  runtime/       C6
  manager/       C4
  policy/        C5
  protocol/      C7  (media path reuses the proven agent code)
adapters/
  mock/          C3  the first and most-tested adapter
  windows/       C8  WSL2 provisioning + compositor + capture/inject
  linux/             (later)
  macos/             (later)
services/
  identity/      C10 accounts, friends, invites
  signaling/     C10 workspace registry + connection introduction
client/          C9  Tauri + React + Tailwind + ShadCN
docs/            SPEC.md · ARCHITECTURE.md · adr/
```

`domain` and `adapter` depend on nothing platform-specific, so everything above the
boundary compiles and tests with **zero OS involvement** against the mock adapter.

---

## 6. Build sequence (ADR-007 order) — build one by one

Each step has a **definition of done** so "done" is a fact, not a feeling. A step starts
only when its dependencies are done.

| # | Step | Depends on | Done when |
|---|------|-----------|-----------|
| 001 | **Spec** | — | ✅ frozen v1.0 |
| 002 | **Architecture** | 001 | ◐ this document, reviewed |
| 003 | **Domain + Adapter interface** (C1, C2) | 002 | types compile; trait method-per-spec-section; capability set defined |
| 004 | **Mock adapter** (C3) | 003 | fake workspace: create→launch→pause→resume→destroy, all in memory, no OS |
| 005 | **Manager** (C4) | 004 | workspaces + members + roles + lifecycle + join flow, tested on mock |
| 006 | **Policy Engine** (C5) | 004 | deny-by-default, two-layer grants, revoke-while-running, audit log — tested on mock |
| 007 | **Runtime** (C6) | 005, 006 | drives a workspace via the adapter; enforces roles at the runtime (§4.3) |
| 008 | **Protocol** (C7) | 003 | display+input+clipboard+files+control over the wire; media path from reused pipe |
| 009 | **Windows adapter** (C8) | 007, 008 | mock swapped for WSL2; **passes the isolation validation suite** (V1–) |
| 010 | **Linux adapter** | 009 | same suite, native (no VM) |
| 011 | **macOS adapter** | 009 | same suite, Apple Virtualization |
| 012 | **Client** (C9) | 008 | connect, view, control, dashboard, chat — on real workspaces |

UI is 012 **by design**. The mock adapter (004) makes 005–008 real and testable long
before any OS work.

---

## 7. Validation track (runs alongside, feeds evidence into 009–011)

The spec is proven by construction above the boundary; adapters are proven by
**experiment**. These run in parallel with build steps, and an adapter is not "done"
(009+) until its suite passes.

| V | Question | Status |
|---|----------|--------|
| V1 | Do `interop`/`automount` off close host escape? (§3.4, §8.1) | **BEFORE measured: 4 leaks. AFTER: pending.** |
| V1b | Forbidden path → *not found*, not *denied*? (§6.5) | pending — **predicted FAIL**, the interesting one |
| V2 | Virtual filesystem semantics: share/hide/no-traversal (§8) | pending |
| V3 | Two concurrent workspaces coexist isolated? (§3.4) | pending |
| V4 | Per-workspace CPU/RAM/net limits enforce? (§7.4) | pending |
| V5 | App lifecycle: launch/crash-isolated/restart (§10.4) | pending |
| V6 | Performance baseline: start time, RAM, latency | pending |

**Falsification standing order** (keeps ADR-006 honest): the architecture's Windows
choice is reopened the day anyone shows **two independent cursors in one Windows session
without a VM**. Until then it is closed, not "reducible."

---

## 8. What is genuinely unknown (honest list)

Solved (product): vision, audience, hardware model, trust model, workspace model, security
philosophy. Design-needed (answered by spec, code pending): adapter interface, runtime,
policy, manager, protocol shapes. **Research (needs experiments):** filesystem
virtualization mechanism, multi-workspace behaviour, resource control, GPU sharing, audio
routing, performance, saved-workspace persistence, cross-workspace isolation strength.

The architecture choice ("native app isolation needs a rented primitive") is **closed by
structure** (one session, one input queue). The adapter's **fitness** (does WSL2 satisfy
every spec section) is **open by evidence** and is what the validation track exists to
answer.
