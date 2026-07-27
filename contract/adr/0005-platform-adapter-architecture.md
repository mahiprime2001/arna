# ADR-005: Platform Adapter Architecture

**Status:** Accepted
**Date:** 2026-07-16

## Context

Each OS provides *radically different* primitives for "an isolated environment with its
own screen and input":

- **Linux** provides them **natively** — namespaces, cgroups, and a headless
  compositor. No VM required.
- **Windows Server** provides them natively via RDS sessions.
- **Windows Home/Pro** provides them only through a VM; the native primitive (an RDP
  session) exists but is license-limited. See
  [ADR-006](0006-windows-adapter-rendering-input.md).
- **macOS** has no multi-session GUI; it needs Apple Virtualization.

An early draft drew our boundary at *"everything above the VM."* That was wrong: it
hard-codes one implementation into the architecture, and it is simply false on Linux,
where the best implementation uses no VM at all.

## Decision

**The boundary is the platform integration layer, not the VM.**

```
        Workspace Client
                │
   Runtime · Manager · Policy · Protocol · Filesystem · Resources
                │
        ── Adapter interface ──         ← the boundary
                │
   Windows Adapter · macOS Adapter · Linux Adapter
                │
        Win32 / WSL / Hyper-V / bubblewrap / namespaces /
        Apple Virtualization / native compositor APIs
```

The Runtime says **"create a workspace."** The adapter decides *how*. The Runtime never
learns which mechanism was used, and must never branch on the host OS.

Adapters may legitimately use entirely different technology per platform. That is the
point, not a compromise.

### Capability negotiation

Adapters are not equal. Rather than degrade the interface to the weakest platform, each
adapter declares capabilities (e.g. GPU acceleration, USB passthrough, live resize,
snapshot, pause/resume). The Runtime queries capabilities and degrades features
explicitly. **An adapter must never silently fake a capability it lacks.**

Capability differences are an implementation reality, **not a product strategy**.
Parity across host platforms is the goal wherever practical: a capability flag records
**a gap to be closed, not a tier to be sold**. Implementations may differ in efficiency;
the experience must not differ in kind. Where a platform's primitives genuinely forbid a
feature, that surfaces as an explicit capability — never as a quietly different product.

### Required of every adapter

Deny-by-default isolation ([ADR-003](0003-hardware-ownership-model.md)) and the state
transitions in [ADR-004](0004-workspace-identity-model.md), or an explicit refusal via
capabilities. An adapter that cannot enforce deny-by-default does not ship.

## Alternatives Considered

**Single implementation across all platforms.** Rejected. There is no mechanism common
to Windows, macOS, and Linux; forcing one would mean a VM everywhere, penalizing Linux
for no reason.

**Boundary "above the VM".** Rejected — the refinement this ADR exists to record. It
bakes an implementation into the architecture and is untrue on Linux.

**Let the Runtime branch per OS internally (`#[cfg(windows)]` everywhere).** Rejected.
That is the adapter pattern with none of its benefits: untestable, and platform
assumptions leak into policy and lifecycle code.

## Consequences

- The adapter interface must be defined from **product needs**, not from what WSL2
  happens to expose. Designing it against a single adapter would leak that adapter's
  shape into the platform permanently.
- A **mock/fake adapter** is mandatory so Runtime, Manager, and Policy are testable with
  no OS involved. Expect the fake to be the most-used adapter in CI.
- The first real adapter is Windows/WSL2; `LinuxNative`, `WindowsRds`, and `MacVm` slot
  in later without changing any layer above.
- Feature work above the boundary is **not blocked** on adapter work — Manager, Policy,
  Protocol, and UI are all adapter-agnostic and can proceed in parallel.
- Cost: an indirection that will feel like overhead while exactly one adapter exists.
  We accept that; the second adapter is what it is paying for, and it will arrive.
