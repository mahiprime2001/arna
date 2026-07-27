# WSE — Vision

**Status:** draft for review · supersedes the framing in earlier notes · answers to `SPEC.md`

---

## One sentence

> Docker changed servers by making the **execution environment** portable instead of the
> application. **WSE (Workspace Engine)** applies the same philosophy to desktop computing:
> each operating system provides its own implementation, but every application experiences
> the same **Workspace Runtime** — a secure, collaborative workspace it cannot see past.

WSE is **not** "Docker for desktops." It borrows Docker's *philosophy* (change what the
application sees, not the application) and takes the *identity* of a runtime contract:

| Platform | Defines |
|---|---|
| **OCI** | Container contract |
| **JVM** | Java runtime contract |
| **.NET CLR** | Managed runtime contract |
| **WSE** | **Workspace contract** |

VMs, WSL2, Hyper-V, namespaces, and future OS APIs are **implementation technologies**, not
the product. The product is the contract.

---

## The three pillars

Every engineering decision answers to these.

### Pillar 1 — The contract is the product

The engine isn't the product. The VM isn't the product. WSL2 isn't the product. The
**Workspace Capability Contract** is the product; everything else exists to satisfy it.

A contract is only real IP if it is **conformance-testable** — POSIX, OCI, and the JVM spec
are valuable because you can *prove* an implementation conforms. A capability *list* is a
table of contents. WSE's contract must ship an exact per-capability specification **and a
pass/fail conformance suite** that every adapter must pass. `SPEC.md` (116 MUSTs) is the
seed; `CONTRACT.md` is its machine-checkable distillation.

### Pillar 2 — Two application models

The apparent contradiction ("apps are unchanged" vs. "apps call `workspace.chat()`")
dissolves into two deliberate tiers:

- **Tier 1 — Legacy applications** (Chrome, VS Code, Office, Photoshop). Never written for
  WSE; they target Win32 / native APIs. The adapter **wraps** them — it fakes the workspace
  environment *around* an unmodified binary. This is the expensive, lossy path (a VM or
  sandbox). Analogy: Docker/VM.
- **Tier 2 — Workspace-native applications**. Written against the WSE SDK; they **speak** the
  contract directly (`workspace.files()`, `workspace.chat()`, `workspace.ai()`). Cheap,
  full-fidelity, portable across every adapter. Analogy: JVM (the app targets the runtime).

One contract, two ways to reach it: native apps *speak* it; legacy apps are *dragged into*
it by the adapter. This is also *why the SDK exists* — it is the cheap path to full
workspace fidelity and the incentive for developers to go native.

### Pillar 3 — The adapter is a strategy, not an identity

Each OS adapter fulfils the same contract using the **best native mechanism available at
that time**. The mechanism may change; the contract does not.

```
Contract  →  Adapter  →  Mechanism  →  Cost
```

"How the adapter does it is irrelevant" is true for **isolation** and false for everything
with a cost. So the contract has two layers (see Pillar-1 discipline):

- **Mandatory core** — isolation and the invariants of `SPEC.md §18.3`. Every adapter MUST
  provide these, identically. Not declarable as unavailable. No partial-isolation tier.
- **Declared capabilities** — GPU acceleration, live-memory snapshot, USB passthrough,
  latency class, native-app support (§18.2). These MAY differ per adapter and MUST be
  declared honestly, never faked.

An abstraction that hides cost is dishonest. WSE keeps a **versioned, dated cost table** (see
`ARCHITECTURE.md`) so "the plumbing can evolve" always has a receipt — and so no roadmap ever
lands on the critical path of an OS API that does not yet exist.

---

## What a user sees

Not Windows, not Linux, not macOS. A **workspace** — a blank canvas with the apps the owner
made available, arranged as windows. The operating system stays hidden. This is not remote
desktop (you don't connect to a desktop); you connect to a *place*.

```
Workspace
──────────────────────────
   VS Code     Chrome
   Terminal    Files
──────────────────────────
```

## What WSE is not

- **Not a VM product.** VMs are one adapter's mechanism, not the identity.
- **Not remote desktop.** No host desktop is exposed; apps live on a workspace canvas.
- **Not an app rewriter.** Tier-1 apps run unmodified. Tier-2 apps opt in.
- **Not a promise of native Windows `.exe` isolation without virtualization —** *today*.
  On Windows there is currently no practical way to provide Docker-style environment
  isolation for arbitrary native GUI apps without virtualization or deep OS integration.
  That is a statement about today's platforms, not a permanent law. WSE ships on the
  mechanism that exists now and inherits better ones as they arrive.

---

## The real IP

Not the streaming. Not the VM. Not the sandbox. Not the SDK in isolation. It is the
**Workspace Capability Contract**: define it well enough that a native Windows app, a Linux
app, a workspace-native app, an AI agent, and a plugin can all participate in the same
conceptual workspace despite completely different implementations underneath — and you have
built something genuinely new. The adapters and mechanisms evolve; the contract is the
invariant that gives WSE its identity and lets an ecosystem grow around it.
