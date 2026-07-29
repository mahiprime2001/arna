# Core: Permissions

**Normative.** Authorization — which role may exercise which access right on a
capability — lives in the **engine**, behind an `Authorizer` interface. It is
**not** inside any capability. Capabilities are mechanical; the engine asks, the
policy system answers.

## Roles and access rights (SPEC §4)

| Role | Authority |
|------|-----------|
| **Owner** | Governs the workspace. Holds every access right. |
| **Collaborator** | Works in the workspace. Rights are granted, deny-by-default. |
| **Observer** | Sees the workspace. May only `ViewDisplay` — never extract or inject. |

Access rights: `ViewDisplay`, `Keyboard`, `Pointer`, `ClipboardRead`,
`ClipboardWrite`, `FileTransfer`. Read and write directions are **separate**
rights and must not be granted as one (§9.2). An access right is distinct from a
capability: a capability is what the workspace *provides*; an access right is what
a member may *do* with it.

## The Authorizer interface

```
authorizer.allows(ctx, grants, right) -> bool
```

- `ctx` — the **authorization context** (`AuthContext`). Today just a `Role`; a
  `MemberId` that *resolves* to a role arrives with collaboration. Capability
  operations never see more than this — they know nothing of identity, sessions,
  or networking.
- `grants` — the workspace's collaborator grants (§4.6.2).
- returns the decision.

The default policy is `RoleMatrixAuthorizer`, implementing the SPEC §4.6 matrix:
Owner all; Observer `ViewDisplay` only (observing is not extracting, §4.6.1);
Collaborator deny-by-default (§6.1). It can grow into a full Permission Manager
(role → right → capability → decision) **without any capability changing** —
because the engine depends on the interface, not the concrete policy.

## Invariants

- Policy lives in the engine, never in an adapter or a capability.
- Deny-by-default: an unspecified right is denied (§6.1).
- A capability-gated operation checks (a) the capability is declared and (b) the
  authorizer allows it — in that order — before the adapter is touched.
