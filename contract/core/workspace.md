# Core: Workspace

**Normative.** A workspace is an isolated execution environment — a *place*, not
a connection (SPEC §3). It exists independently of any connection and can run
with no members connected.

## Identity

Each workspace has a `WorkspaceId` (see [identity.md](identity.md)) and a
`WorkspaceIdentity` view: id, name, state, persistence, owner, members, declared
capabilities, contract version, last isolation attestation, metadata. This is
everything the engine knows about *what a workspace is*, independent of the
adapter running it.

## Lifecycle (SPEC §5.1–§5.2)

Exactly one state at a time:

```
Created → Running | Deleted
Running → Idle | Paused | Saved | Deleted
Idle    → Running | Paused | Saved | Deleted
Paused  → Resuming | Saved | Deleted
Resuming→ Running
Saved   → Resuming | Archived | Deleted
Archived→ Saved | Deleted
Deleted → (terminal)
```

Anything not listed is forbidden — the whole rule. Enforced by the engine and
checked exhaustively (all 64 pairs) in conformance.

## Starting is gated by isolation

`Created → Running` is not just a transition. The adapter **attests** isolation;
the engine **evaluates** the attestation against its `IsolationPolicy` (§18.3).
If the evidence is rejected, the workspace is stopped and never presented as
running. There is no partial-isolation tier. See
[../SPEC.md](../SPEC.md) §18.3 and the isolation attestation in the contract.

## Persistence (SPEC §5.4)

A workspace is **Temporary** (contents destroyed irrecoverably on close) or
**Saved** (contents persist and are resumable). Both are first-class; neither is
a degraded form of the other. Deletion is irrecoverable (§5.5) and destroys
contents, not merely unlists them.
