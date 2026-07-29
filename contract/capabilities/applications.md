# Capability: Applications

**Stable.** SPEC §10. The capability to run applications inside a workspace over
time. It is a **lifecycle**, not a launch call.

## 1. Intent

A workspace can run applications. Every platform — Windows, Linux, macOS,
Android, a cloud host — has *some* notion of an application existing over time, so
the contract is centred on that existence, not on "start a process". The adapter
maps its platform's reality onto the contract; the engine never learns whether an
instance is a process, a container, a VM, or a remote session.

## 2. Contract (the two shapes)

Applications follows the same identity/handle split as Storage (resource ≠ handle)
and Devices (descriptor ≠ handle):

```
ApplicationDescriptor   →   launch   →   ApplicationInstance
   (immutable defn)                        (runtime state)
```

- **`ApplicationDescriptor`** — a host-curated catalog *definition*: `id`, a
  platform-neutral `entry` key (e.g. `"browser"`), `name`, `metadata`. Immutable.
  The adapter maps `entry` onto whatever the platform launches.
- **`ApplicationInstance`** — a *running* application: a stable, unguessable
  `ApplicationInstanceId`, the `application` it came from, its `state`, and the
  `windows` it owns.

Operations (mechanical; the engine has already checked policy and catalog):
`app_launch(descriptor) → instance`, `app_stop(instance)`, `app_instances()`.

## 3. State model

```
Declared → Launching → Running → Suspended ⇄ Resuming
                          │
                          ▼
                       Stopping → Stopped
```

These are **contract** states. The adapter maps its platform's own
process/job/session states onto them; the platform's state names never surface.

## 4. Invariants

- **An instance id is not a PID.** It is stable and unguessable and lives in the
  contract; the adapter may map it internally onto a PID, container id, or remote
  session, but that mapping never escapes. (A PID is reused by the OS; an instance
  id is never reused.)
- **Descriptor ≠ instance.** The definition is immutable and shared; each launch
  is a new instance with its own identity. Two launches of one descriptor are two
  distinct instances.
- **Applications owns associations, not windows.** An instance *owns* zero or more
  `WindowId`s; the **Windows** capability owns window behaviour (focus, bounds,
  listing). A headless service is a valid instance with zero windows.
- **Stop is terminal.** After `app_stop` the instance no longer exists.
- **Undetectability.** An `entry` not in the catalog is `NotFound`, never
  `PermissionDenied` (SPEC §6.5) — a workspace cannot probe what it wasn't granted.

## 5. Error mapping

| Situation | Error |
|-----------|-------|
| workspace not running | `InvalidState` |
| `entry` not in catalog | `NotFound` (never `PermissionDenied`) |
| capability not declared | `CapabilityUnavailable(Applications)` |
| platform launch/stop failure | `Internal` (adapter maps; never invents) |

## 6. Events

The lifecycle is observable through the **one** event envelope — no second event
system. The engine emits:

```
ApplicationLaunchRequested → ApplicationStarted → … → ApplicationStopping → ApplicationStopped
```

Payloads carry the instance id and the catalog `entry` only — never a PID, never
bytes (SPEC §17.1).

## 7. Reference implementation

`engine/adapters/mock` — a launch opens one window the instance owns; stop closes
it. This is the executable definition of conforming; `run_applications` (7 checks)
validates the model: running instance, window ownership, distinct identities,
terminal stop, the observable lifecycle, pre-running rejection, undetectability.

## 8. Platform

`engine/adapters/windows` — maps `entry` onto a real application inside the sealed
WSL2 workspace, tracks the `ApplicationInstanceId → PID` mapping privately, and
associates the windows it opens. Declaring this capability turns on the
`run_applications` suite for the adapter (see [../STATUS.md](../STATUS.md)).
