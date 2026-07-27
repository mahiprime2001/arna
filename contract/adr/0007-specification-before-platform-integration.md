# ADR-007: Product Specification Before Platform Integration

**Status:** Accepted
**Date:** 2026-07-16

## Context

Infrastructure exerts constant gravitational pull on product design. It is not a
hypothetical risk — it has already happened twice in this project's short history:

1. The isolation boundary was first drawn as *"everything above the VM."* That is a
   statement about one implementation, and it is **false on platforms that need no VM**.
   It was corrected to *"above the platform integration layer"*
   ([ADR-005](0005-platform-adapter-architecture.md)).
2. `Host Resources: Installed Apps` described what a *host OS* looks like, not what the
   *product* offers, and was unimplementable as written
   ([ADR-003](0003-hardware-ownership-model.md), [ADR-006](0006-windows-adapter-rendering-input.md)).

Both were caught. The next ones may not be. Once infrastructure vocabulary enters the
product layer it never leaves: a single `CreateWSLWorkspace()` makes the entire platform
Windows-shaped forever, and every later platform pays for it.

The failure mode is invisible in the moment. Building the first adapter first *feels*
efficient — it produces a demo soonest — and the platform silently inherits that
adapter's shape as its definition of reality.

## Decision

> **The product defines the interface. Platform adapters implement the interface.
> Adapters must never define the product.**

This principle is binding on all layers above the adapter boundary. Concretely:

1. **The Platform Specification is written first**, and defines a workspace without
   naming any operating system, virtualization technology, windowing system, or
   implementation language.
2. **The adapter interface is derived from the specification**, never from the
   capabilities of any particular adapter — not even a convenient one.
3. **The mock adapter is the first implementation.** Manager, Policy Engine, Runtime,
   and Protocol are developed and tested against it, with no OS involvement.
4. **No layer above the adapter boundary may name a platform or mechanism.** No
   `CreateWSLWorkspace()`. No platform conditionals in Runtime, Manager, or Policy. The
   vocabulary above the boundary is *workspace, application, grant, policy* — never
   *wsl, desktop, session, container, hypervisor*.

### Required build order

```
001 Platform Specification    007 Runtime
002 Architecture              008 Protocol
003 Adapter Interface         009 Windows Adapter
004 Mock Adapter              010 Linux Adapter
005 Manager                   011 macOS Adapter
006 Policy Engine             012 UI
```

UI is near-last by design. This is deliberately the opposite of the usual order.

## Alternatives Considered

**Build the Windows adapter first, generalize later.** Rejected. Fastest to a demo, and
the surest way to make the platform permanently Windows-shaped. "Generalize later" is
where architectures go to die: by then the adapter's assumptions are load-bearing in
code nobody wants to touch.

**Derive the interface from the union of all platforms' capabilities.** Rejected. That
leaks *every* platform into the product instead of one, and is unbounded — we cannot
enumerate the capabilities of platforms we have not targeted yet.

**Specify informally; let the interface emerge from code.** Rejected. What emerges is
the shape of whatever was written first, which returns us to the first alternative
wearing a disguise.

## Consequences

- **Slower to first pixel.** We accept this explicitly. The first runnable thing is a
  fake workspace, not a real one.
- **The mock adapter will be the most-exercised adapter in the project**, and the
  primary target in CI. It must be maintained as a first-class implementation, not a
  test stub that rots.
- **Review rule:** a change that puts a platform name, mechanism, or conditional above
  the adapter boundary is a review failure regardless of correctness or urgency.
- **Spec bugs surface early and cheaply.** If the specification requires something no
  adapter can deliver, we learn it while writing adapters and decide explicitly — a
  capability gap ([ADR-005](0005-platform-adapter-architecture.md)) or a spec revision.
  Both beat discovering it after the product depends on it.
- The specification becomes the contract that outlives every implementation in this
  repository, including ones not yet written.
