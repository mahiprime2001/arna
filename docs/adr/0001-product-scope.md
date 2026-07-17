# ADR-001: Product Scope — a Workspace Platform, not a VM or OS

**Status:** Accepted
**Date:** 2026-07-16

## Context

A workspace needs an isolated execution environment with its own screen and its own
input. That requirement pulls hard toward writing low-level systems software, and we
seriously considered going all the way down — including a purpose-built micro-VM on
the Windows Hypervisor Platform (WHP), Firecracker-style, in Rust.

Two facts framed the decision:

1. **Only the OS hands out virtualization.** On Windows every isolation product —
   WSL2, Docker, VirtualBox, VMware, Windows Sandbox — calls the same WHP API. There
   is nothing underneath it to invent. We can write our *own* VM on WHP, but we cannot
   invent the layer WHP sits on.
2. **A hypervisor is not what anyone buys.** The product promise is *"I can lend my
   workstation to another developer while I keep working."* No user chooses a product
   for its custom hypervisor.

## Decision

**We build the Workspace Platform. We rent the primitives.**

We own, and will innovate on:

- Workspace Runtime
- Workspace Manager
- Policy Engine
- Workspace Protocol
- Workspace Filesystem (virtual, host-granted)
- Resource Manager
- Collaboration Layer
- Client / UI
- SDK, CLI, Developer APIs

We will **not** build:

- An operating system
- A hypervisor or VM monitor
- GPU drivers, kernel schedulers
- Display drivers (unless a specific platform leaves no alternative)

## Alternatives Considered

**Write our own micro-VM on WHP (rust-vmm / Firecracker-style).**
Rejected. Total control and ~100ms boots, but it is a multi-year effort that renders
zero pixels of product along the way, carries permanent OS/hardware/driver
compatibility burden, and differentiates nothing users care about.

**Native, no isolation layer — run apps directly on the host desktop.**
Rejected on measured evidence; see [ADR-006](0006-windows-adapter-rendering-input.md).
Windows structurally forbids it.

**Off-the-shelf full VM per workspace (VirtualBox/Hyper-V guest).**
Rejected as the default. Heavy RAM/CPU per workspace, guest OS licensing, weak
graphics. Remains available as an adapter where it fits ([ADR-005](0005-platform-adapter-architecture.md)).

## Consequences

- We depend on OS-provided primitives, and they differ per platform. This forces the
  adapter architecture in [ADR-005](0005-platform-adapter-architecture.md) — that is a
  consequence of this ADR, not an independent choice.
- Where a platform's primitives are weak, our product is weaker there. We accept this
  rather than paying to paper over it in kernel code.
- We ship working software in months rather than years.
- "Custom hypervisor" is permanently off the table as a differentiator. If a competitor
  ships one, we do not chase it — we compete on runtime, policy, and collaboration.
- Our engineering identity is *platform*, not *virtualization*. Hiring, docs, and
  roadmap should reflect that.
