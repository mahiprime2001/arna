# Capability: Storage (Workspace Persistence)

**Status: Draft** · answers to SPEC §8, §9.3 · `Capability::Storage`

The capability answers one question:

> *How does a workspace persist data across time?*

Files are one implementation. An object store, a database, encrypted storage, an
in-memory test store, cloud storage are others. **The contract assumes none of
them.** It owns **Resources**, not files — so there is deliberately no `File`,
`Folder`, `Directory`, `Path`, `Drive`, `Mount`, or `Extension` anywhere in it.

---

## 1. Intent

Persistent storage of workspace-owned **resources**, independent of
implementation. A resource belongs to the workspace, is isolated from the host
and every other workspace, and survives suspend and (for a Saved workspace)
close and restore.

Scope: the workspace's *own* resources. A member moving data across the boundary
is gated by policy; an app *inside* the workspace using its own resources is
internal and ungoverned (§10.6). Mounting *host* folders (SPEC §8.2/§8.6) is a
separate future capability.

## 2. Contract

**Operations** — on resources, never on files:

```
create(ws, name, kind) -> ResourceId
write (ws, resource, bytes)
read  (ws, resource) -> bytes
delete(ws, resource) -> bool          // true if it existed
list  (ws) -> [ResourceMetadata]
```

`open`, `move`, streaming handles are deferred (see open questions); a
`ResourceId` is itself the stable reference, so v0.1 needs no separate handle.

## 3. Data model

Contract types only — no platform fields:

```
ResourceId                             // stable, unguessable, immutable
ResourceKind { Blob }                  // Collection (with children) is a future kind
ResourceMetadata { id, name, kind, size }
```

Bytes are opaque (`Vec<u8>`); as with Clipboard, formats are data, not shape.

## 4. State model

A resource's contract states (not OS states):

```
Created → Modified → (stable) → Deleted
```

A `ResourceId` names the resource through all of them. Deleted is terminal: the
id never resolves again and is never reused.

## 5. Invariants

- **I1 — Stable identity.** A `ResourceId` is unguessable and immutable, and
  names one resource for its whole life. *(Universal rule: every persistent
  object the engine exposes has a stable contract identity — WorkspaceId,
  ResourceId, WindowId, and so on.)*
- **I2 — One workspace.** A resource belongs to exactly one workspace and is
  invisible to any other and to the host beyond the adapter (SPEC §8.1).
- **I3 — Deletion is terminal.** After delete, `read`/`write`/`delete` on that
  id fail as `NotFound`; the id is never reopened or reused.
- **I4 — Identity survives metadata.** Renaming or otherwise changing metadata
  does not change the `ResourceId`.
- **I5 — Persistence.** Resources survive suspend; a Saved workspace keeps them
  across close/restore; destroying the workspace destroys them irrecoverably
  (§5.4, §5.5).
- **I6 — Boundary transfers are gated.** A member's create/write/read/delete
  crosses the boundary and requires the `FileTransfer` right — Owner holds it,
  Observer never (extraction is not observation, §4.6.1), Collaborator only if
  granted (§4.6.2). `list` is host introspection (§17.2), ungated.
- **I7 — Auditable, not logged.** Transfers emit an event with *who*,
  *operation*, and *resource id* — never the bytes (§17.1).
- **I8 — Absence of capability is typed.** A storage op on a workspace that
  doesn't declare Storage fails as `CapabilityUnavailable(Storage)`.

## 6. Error mapping

Reuses the existing vocabulary — nothing new:

| Situation | Error |
|---|---|
| Workspace doesn't provide Storage | `CapabilityUnavailable(Storage)` |
| Role lacks `FileTransfer` for a transfer | `PermissionDenied { FileTransfer, role }` |
| Unknown or deleted resource id | `NotFound` |
| Unknown workspace id | `NotFound` |
| Adapter/platform failure | `Internal` |

## 7. Conformance tests (`run_storage`)

Behaviour, not implementation. Runs only for adapters declaring `Storage`:

- `storage/resource_id_is_stable` — create returns an id; list/read see the same
  id (I1, I4).
- `storage/isolated_per_workspace` — a resource in one workspace is not readable
  from another and not in its list (I2).
- `storage/owner_roundtrips` — Owner create → write → read returns the bytes.
- `storage/deletion_is_terminal` — create, delete, then read → `NotFound`;
  deleting a missing id returns `false` (I3).
- `storage/list_reflects_resources` — two creates, list has two.
- `storage/observer_refused_transfer` — Observer create/read → `PermissionDenied`
  (I6).
- `storage/collaborator_needs_filetransfer_right` — refused without the right,
  allowed once granted (I6, exercises the Authorizer).
- `storage/persists_across_suspend` — written while Running, still readable after
  the workspace is stopped/Saved (I5, partial; full save→restore lands with
  Resume).

Plus, at the engine level against a no-storage adapter: `CapabilityUnavailable`.

## 8. Reference implementation

The mock adapter holds an in-memory `ResourceId -> (metadata, bytes)` map per
workspace and implements the mechanical `StorageCapability`. It declares
`Capability::Storage` and runs this suite via `run_all`. If the in-memory store
feels natural to write, the abstraction is sound.

---

## Deliberately left out (layer later, without changing the core)

quotas · synchronisation · version history · locking · sharing · encryption ·
mounting · replication · transactions · streaming handles · hierarchy
(Collection kind + children) · save→restore roundtrip (awaits Resume).
