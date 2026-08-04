# WSE Design Principles

Read this before any code. These are not APIs — they are the philosophy the
architecture is built to enforce. Every document, crate, and decision in WSE should
be traceable to one of them. When a change violates a principle, the change is wrong,
not the principle.

They are frozen. New ideas are welcome; they must find a home *inside* these, not
beside them.

---

### 1. Applications are data, not code.
An application is a **manifest** — a declaration of the resources it consumes. Adding
Chrome, Python, or VS Code is authoring data, never writing engine code. *Rules out:*
`if app == "chrome"` anywhere below the manifest layer.

### 2. Resources are virtualized, not processes.
WSE does not sandbox processes; it controls what a workspace *sees*. Every capability
question reduces to "which resources, projected which way?" *Rules out:* reaching for
a container or VM the moment isolation is mentioned — projection comes first, a heavier
runtime only when a mode genuinely demands it.

### 3. The OS owns applications; WSE owns workspaces.
We do not reimplement process management, windowing, or a filesystem. The OS runs the
app; WSE decides the *context* it runs in. *Rules out:* rewriting the host, its
registry, or its window manager.

### 4. Projection is declarative.
A workspace's view of the host is described (manifest + profile), then produced by one
generic projector. The description is data; the mechanism is shared. *Rules out:*
per-application imperative setup steps.

### 5. Runtimes execute plans; they do not interpret manifests.
The projector is the only thing that understands manifests and profiles. A runtime
receives a `LaunchPlan` and realises it. *Rules out:* a runtime that special-cases an
application — if it needs to, the plan was incomplete.

### 6. Every mutable resource belongs to a workspace.
State has an owner. Overlay dirs, profiles, volumes, temp dirs — each is listed in the
`ExecutionContext`'s allocation ledger so teardown is total. *Rules out:* orphaned
state; anything created outside a workspace's ownership is a leak.

### 7. Architecture before implementation.
A boundary is defined and proven (contract + conformance, or a projector test) before
it is wired into the product. *Rules out:* discovering the contract by accident, inside
the runtime.

### 8. Manifest problems are preferable to engine problems.
Success looks like "Chrome ignores this flag" — a data fix — not "the engine can't
express this." A steady stream of manifest work means the model is holding. *Rules out:*
treating an application quirk as a reason to change the architecture.

### 9. Capability honesty over false guarantees.
A runtime declares only what it can truly do. Native's soft suspend and shared
filesystem are documented as such; `deny` is a real wall only where a runtime enforces
it. *Rules out:* promising isolation or persistence the runtime cannot deliver.

### 10. Implementation should shrink `CapabilityUnavailable`, not change contracts.
The contract is the constant. Progress is measured by capabilities moving from
unavailable to live on more runtimes — never by bending the contract to fit an
implementation. *Rules out:* editing the spec to make a stubborn runtime pass.

---

## The pattern these produce

Together they describe an operating-system shape, one level up from the OS:

```
Policy      profiles decide what a workspace may request
   ↓
Planner     the projector turns manifest + policy into a LaunchPlan
   ↓
Executor    a runtime realises the plan into an ExecutionContext
   ↓
State       the workspace owns every resource, released on teardown
```

Policy → Planner → Executor → State. The technology under the executor — native
desktop, Docker, a cloud VM — is an interchangeable detail. That interchangeability is
the whole point.
