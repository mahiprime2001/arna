# WSE Documentation

**WSE is a Workspace Operating Layer** — the OS owns applications, WSE owns
workspaces, and a workspace is a bundle of *virtualized resources*. Applications
are data (manifests); runtimes execute plans. This index is the map.

> **Status:** the architecture (v1 contract + v2 WRM) is **complete and stable**.
> Treat it like a specification — changes should be rare, deliberate, and driven by
> concrete evidence, not speculation. From here the work is *implementation*
> (wiring + manifests), not architecture.

## Start here (reading path for a new engineer)

0. **[design-principles.md](design-principles.md)** — the ten principles. Read these
   *before* any code; every decision traces back to one.
1. **[contract/VISION.md](../contract/VISION.md)** — what WSE is and why.
2. **[contract/CONTRACT.md](../contract/CONTRACT.md)** — the v1 contract, the map of
   the core concepts and the rules that bind them.
3. **[workspace-resource-model.md](workspace-resource-model.md)** — WSE v2: how
   applications became *data*.
4. **[runtime-execution-contract.md](runtime-execution-contract.md)** — the runtime
   boundary's output half.
5. **[../IMPLEMENTATION_GUIDE.md](../IMPLEMENTATION_GUIDE.md)** — how to write an
   adapter; **[../contract/STATUS.md](../contract/STATUS.md)** — what's built.

## The architecture in one picture

```
Application Registry → Manifest + Workspace Profile → Projector → LaunchPlan
                                                                     │
                                                                     ▼
                                                       Runtime.execute() → ExecutionContext
                                                                     │
                                                          suspend · resume · snapshot · destroy
```

Every boundary has both halves — the symmetry that makes it a spec:

| Boundary  | Input                | Output             |
|-----------|----------------------|--------------------|
| Projector | Manifest + Profile   | LaunchPlan         |
| Runtime   | LaunchPlan           | ExecutionContext   |

---

## 0. Philosophy
- [design-principles.md](design-principles.md) — the ten frozen principles + the
  Policy → Planner → Executor → State pattern. The philosophy every other doc obeys.

## 1. Vision & product
- [contract/VISION.md](../contract/VISION.md) — three pillars, principles.
- [../ROADMAP.md](../ROADMAP.md) — Windows-first, "use it daily".
- [../BACKLOG.md](../BACKLOG.md) — pulled in only when real use proves the need.

## 2. The Contract (v1 — the frozen core)
- [contract/CONTRACT.md](../contract/CONTRACT.md) — the canonical map + binding rules.
- [contract/SPEC.md](../contract/SPEC.md) — the frozen constitution (the MUSTs).
- [contract/ARCHITECTURE.md](../contract/ARCHITECTURE.md) — engine / capability / adapter.
- **core/** — [workspace](../contract/core/workspace.md) ·
  [identity](../contract/core/identity.md) ·
  [capabilities](../contract/core/capabilities.md) ·
  [permissions](../contract/core/permissions.md) ·
  [errors](../contract/core/errors.md) · [events](../contract/core/events.md) ·
  [isolation](../contract/core/isolation.md) · [runtime](../contract/core/runtime.md) ·
  [conformance](../contract/core/conformance.md)
- **capabilities/** — [applications](../contract/capabilities/applications.md) ·
  [clipboard](../contract/capabilities/clipboard.md) ·
  [storage](../contract/capabilities/storage.md) ·
  [devices](../contract/capabilities/devices.md)
  ([maturity table](../contract/capabilities/README.md))
- [contract/STATUS.md](../contract/STATUS.md) — the conformance dashboard (Mock /
  Windows / …).

## 3. WSE v2 — the Workspace Resource Model
- [workspace-resource-model.md](workspace-resource-model.md) — **WRM**: resource
  taxonomy, projection modes, manifests (apps as data), the LaunchPlan (input IR).
- [runtime-execution-contract.md](runtime-execution-contract.md) — the
  **ExecutionContext** (output IR) + the Runtime lifecycle trait.
- [host-resource-projection.md](host-resource-projection.md) — the motivation
  ("what should this workspace see of the host?").

## 4. Runtimes & implementation
- [runtimes/README.md](../runtimes/README.md) — the runtime lifecycle + the runtime
  index (wse-linux-x11, wse-lite).
- [../IMPLEMENTATION_GUIDE.md](../IMPLEMENTATION_GUIDE.md) — writing an adapter.
- **Code:** `engine/` — `common` (vocabulary) · `contract` · `core` (engine) ·
  `wrm` (the projector proof) · `adapters/{mock,windows,windows-native}` ·
  `conformance`. The unified app: `client/` (React + Tauri + embedded engine).

## 5. Decisions (ADRs)
- [contract/adr/README.md](../contract/adr/README.md) — index. 0001 scope · 0002
  audience · 0003 hardware ownership · 0004 identity · 0005 adapter architecture ·
  0006 rendering/input · 0007 spec-before-platform · 0008 adapter discipline ·
  0009 isolation models.

## 6. Where the risk is now
Not architecture. The next problems are **manifest problems** — "Chrome ignores
this flag", "VS Code caches that dir", "this app stores state in the registry."
When problems shift from *"the engine can't express this"* to *"this app needs
another manifest rule"*, WRM is doing its job: application support is configuration,
not architectural change.
