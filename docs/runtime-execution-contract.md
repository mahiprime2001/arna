# The Runtime Execution Contract

**The output half of the runtime boundary.** [The WRM](workspace-resource-model.md)
defines the **input** — the `LaunchPlan`. This defines what a runtime *returns* and
the *lifecycle* it exposes, so every later feature — suspend/resume, snapshots,
monitoring, the UI, migration, cleanup — builds on one standard shape instead of
each runtime inventing its own.

```
LaunchPlan (input, WRM)  →  Runtime  →  ExecutionContext (output, this doc)
```

> A `LaunchPlan` is the recipe. An `ExecutionContext` is the running dish **and how
> to clean it up.** Everything after launch operates on the context, never the plan.

---

## 1. Why the plan isn't enough

After `execute`, a real running thing exists, with handles and allocations the plan
never mentioned:

```
processes/containers · windows/endpoints · mounted or staged resources
temp dirs · ports · volumes · the desktop · per-app profiles · overlay paths
```

`Suspend`, `Resume`, `Snapshot`, `Destroy`, `Migrate`, and "show me what's running"
all need *those*. They are the runtime's response, not its input.

**We already track this ad-hoc** — the native adapter's `WsState { desktop, home,
instances, windows, meta{pid, profile} }` and the Docker workspace's `container +
url` are proto-ExecutionContexts. This contract just names and standardises them.

---

## 2. ExecutionContext — the output object

```rust
struct ExecutionContext {
    instance_id: InstanceId,        // stable, unguessable, per running instance
    runtime: RuntimeId,             // which runtime realised it
    state: ExecState,               // Running | Suspended

    // the running thing — abstract handles, runtime-specific inside:
    processes: Vec<ProcessRef>,     // native: PIDs · docker: container id · cloud: VM id
    surfaces: Vec<Surface>,         // native: window handles · docker/cloud: endpoint URLs

    // what the plan's staged resources actually became:
    realized: Vec<RealizedResource>,// resource name -> concrete path / mount / volume

    // everything to release on teardown (the cleanup ledger):
    allocations: Allocations {      // temp dirs · ports · volumes · desktop · profiles
        temp_dirs, ports, volumes, desktop, profiles, overlays,
    },

    metadata: Map<String, String>,  // runtime-specific bag
}
```

Two design rules:
- **`surfaces`** unifies "a window" (native) and "a URL" (docker code-server, cloud
  stream) — the UI consumes surfaces without knowing the runtime.
- **`allocations`** is a *cleanup ledger*: `destroy` releases exactly what's listed,
  so teardown is complete and leak-free regardless of runtime. (This is why the
  native adapter leaves zero orphan desktops/processes today — the same idea,
  formalised.)

---

## 3. The Runtime lifecycle (the behaviour, not just capabilities)

`RuntimeDescriptor` (WRM §5c) is the *passive* capability declaration. A runtime is
also a *behaviour* — this trait:

```rust
trait Runtime {
    fn descriptor(&self) -> &RuntimeDescriptor;

    /// Negotiation (WRM): can I realise every mode this plan needs?
    fn can_satisfy(&self, plan: &LaunchPlan) -> Result<(), Vec<Mode>>;

    /// Realise the plan's resources (its "how"), launch, return the context.
    fn execute(&mut self, plan: LaunchPlan) -> Result<ExecutionContext>;

    fn suspend(&mut self, cx: &mut ExecutionContext) -> Result<()>;
    fn resume(&mut self, cx: &mut ExecutionContext) -> Result<()>;

    /// Optional — only if descriptor.supports(Snapshot).
    fn snapshot(&mut self, cx: &ExecutionContext) -> Result<SnapshotId>;
    fn restore(&mut self, snap: &SnapshotId) -> Result<ExecutionContext>;

    /// Release every allocation in the context. Consumes it.
    fn destroy(&mut self, cx: ExecutionContext) -> Result<()>;
}
```

`execute` is where the runtime does the **staging** the plan only described —
create the fresh dir, run the Overlay Engine copy, mount the volume — each its own
way. The projector stays pure; the runtime owns all fs/OS work. Errors are the core
contract's `WseError` (a runtime that can't satisfy a plan returns a capability
error, never a crash — WRM §5c).

---

## 4. The lifecycle

```
                 LaunchPlan
                     │  execute
                     ▼
              ExecutionContext ──────────────┐
                 │        ▲                   │ snapshot
        suspend  │        │  resume           ▼
                 ▼        │                SnapshotId
              (Suspended)─┘                   │ restore
                     │                        ▼
                     │ destroy          ExecutionContext
                     ▼
                  (gone — allocations released)
```

States: **Running ⇄ Suspended → Destroyed.** Snapshot/restore branch off a context
(if the runtime supports them).

---

## 5. How each runtime realises it (honest, per-runtime)

| Operation | Native Windows | Docker | Cloud VM |
|-----------|----------------|--------|----------|
| **execute** | stage dirs + env; `CreateProcess` on the desktop; track PIDs + window handles | stage volumes/mounts; `docker run`; container id + code-server URL | provision VM; boot; stream URL |
| **suspend** | *soft*: stop the apps, keep the home (profiles/overlay persist) | `docker stop` (container FS kept) | VM pause |
| **resume** | relaunch from the home (browser/VS Code restore via their profiles) | `docker start` | VM resume |
| **snapshot** | copy the workspace home (storage + overlay) | `docker commit` / volume snapshot | VM snapshot |
| **destroy** | kill PIDs, close desktop, remove temp/profiles | `docker rm -f` + `volume rm` | deprovision VM |

**Honesty (same spirit as the WRM `deny` note):** native **suspend is *soft*** —
Windows processes don't hibernate to disk cheaply, so native suspend stops the apps
and relies on their profiles in the home to restore state. Docker and Cloud suspend
are *real* (the whole environment freezes). `descriptor` declares which a runtime
offers, so the UI never promises a hard suspend a runtime can't do.

---

## 6. What builds on ExecutionContext (why this unlocks so much)

Every one of these becomes a small feature *because the context exists*:

- **Suspend / Resume** — operate on the context; already half-built (native stop,
  docker stop/start).
- **Snapshots / Restore** — the `allocations` + `realized` define exactly what to
  capture.
- **Live status / monitoring** — `surfaces` + `processes` + `state` are what the UI
  shows (running apps, the code-server URL, window count).
- **Migration** — snapshot on runtime A, restore on runtime B (Native→Cloud) — the
  same *workspace*, a different runtime, because the context is abstract.
- **Leak-free cleanup** — `destroy` walks `allocations`; nothing is missed.

None of these need the plan again. That's the point.

---

## 7. Fit with the core WSE contract

This is the same shape as the original adapter boundary, one level up:
- **Descriptor = capability declaration; Runtime trait = the mechanical interface.**
  The engine owns policy (which profile/role may request what); the runtime is
  mechanical (realise + execute + tear down).
- **`WseError` everywhere** — including the capability error from `can_satisfy`.
- **Identity** — `instance_id` obeys the identity rule (stable, unguessable,
  never reused).

---

## 8. Status

Design, not code — and mostly a *formalisation of what already runs*: the native
adapter and the Docker manager each already hold a proto-context and already do
leak-free teardown. Wiring the WRM projector into the runtime (the next step) is
exactly: `LaunchPlan → Runtime::execute → ExecutionContext`, replacing today's
app-specific `launch_native` with a generic plan executor that fills a context.

With this, WSE v2's boundary is complete on **both** sides: **LaunchPlan** in,
**ExecutionContext** out.
