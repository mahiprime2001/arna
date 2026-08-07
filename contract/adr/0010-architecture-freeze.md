# ADR-0010: The architecture is frozen (WRM runtime complete)

**Status:** Accepted. Milestone tag: `v2-runtime-complete`.

## Context

WSE has reached a state with no missing conceptual pieces. The chain is complete
and each boundary has both halves:

```
Vision → Contract → Capabilities → WRM → Projector → LaunchPlan
      → Runtime.execute() → ExecutionContext → Workspace
```

Throughout the project, progress kept surfacing a new "we need…": identities,
events, runtimes, capability negotiation, LaunchPlan, ExecutionContext. There is
no such gap left. The round trip — `manifest → project → LaunchPlan → execute →
ExecutionContext → destroy → zero leaks` — is proven in running code (native
runtime, Job-Object ownership, leak-free teardown). **The engine has stopped
moving.**

That is the signal that WSE has changed category: from an *engine* that grows by
code to a *platform* that grows by data. Supporting IntelliJ, Blender, or Unity is
now "write a manifest / projection rules," not "redesign the engine." Left
unprotected, that property erodes — every "this feels cleaner" edit to the engine
is a small step back toward code-driven growth.

## Decision

**The engine architecture is frozen.** Two architecture versions are frozen; a
third is reserved for any future, deliberate evolution:

```
v1  — the Workspace Contract           FROZEN
v2  — WRM + the runtime execution model FROZEN   (this milestone)
v3  — reserved
```

### The Engine Freeze Rule

> The engine may change **only** if a real implementation demonstrates that the
> architecture *cannot express* a required behavior. Not "this might be useful,"
> not "this feels cleaner" — only "the current architecture literally cannot
> express this."

Frozen surface = the contract (`contract/`), the WRM types and projector
(`engine/wrm`), the runtime execution contract (`ExecutionContext`, the
`execute → destroy` lifecycle), and the projection modes + guarantee model.

### Change process

A change to the frozen surface is treated like a public API break, not a commit:

```
proof the architecture can't express it  →  ADR  →  discussion  →  new architecture version (v3…)
```

An ordinary `git commit` to the engine that is not backed by such an ADR is out of
process. (ADR-0009 is the template: it changed the contract only because a real
adapter exposed a genuine gap the model could not express.)

### Where change belongs instead

Everything that is *not* an inexpressible-architecture problem goes to one of:

- **manifests** — a new application (`samples/` are the reference examples),
- **profiles / projection rules** — new isolation policy,
- **runtime implementations** — a new runtime behind the same `execute` contract,
- **the product** — UI, collaboration, remote control, networking.

## Consequences

- The engineering question changes from *"how should WSE work?"* (finished) to
  *"how should applications describe themselves?"* — a manifest problem.
- Compatibility work moves from `engine/` to `manifests/` and `profiles/`. If the
  next problems read "Chrome needs another manifest field," the freeze is working.
- The three decisions that made this possible are the ones the rule now protects:
  (1) contract separated from implementation, (2) runtimes separated from the
  engine, (3) applications separated from the engine via WRM.
- New contributors read this ADR and the [Design Principles](../../docs/design-principles.md)
  before touching `engine/`.

> *From this point forward, changes to the engine require proof that the
> architecture cannot express the required behavior. Everything else belongs in
> manifests, profiles, runtime implementations, or the product.*
