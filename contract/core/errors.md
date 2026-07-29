# Core: Errors

**Normative.** One closed error vocabulary spans every adapter and SDK.
**Adapters map platform failures into it; they never invent error kinds.** This
is what makes failures mean the same thing everywhere.

## The vocabulary (`WseError`)

| Kind | When |
|------|------|
| `NotFound(what)` | The thing does not exist — **or exists but is not granted**. A workspace must not distinguish the two (SPEC §6.5). Used for ungranted resources, unknown/deleted ids, unknown workspaces. |
| `InvalidTransition{from,to}` | The lifecycle state machine forbids this transition (§5.2). |
| `InvalidState{operation,state}` | The operation requires a different state (e.g. launch before running). |
| `CapabilityUnavailable(cap)` | The workspace does not provide this capability (§18.2). Declared-only. |
| `PermissionDenied{right,role}` | A role is refused an access right. A **visible** refusal — the resource's existence is not secret (contrast `NotFound`). |
| `ContractMismatch{adapter,engine}` | The adapter speaks an incompatible contract version (§18.4). |
| `ResourceUnavailable(what)` | A resource limit or dependency is unavailable (§7). |
| `IsolationRejected{workspace,details}` | The engine rejected the adapter's isolation attestation (§18.3). |
| `Internal(msg)` | A platform/adapter failure, mapped into the contract. |

## The one subtlety worth stating twice

`NotFound` and `PermissionDenied` are **not interchangeable**:

- `NotFound` is the §6.5 *undetectable* refusal — probing must not reveal that a
  thing exists. Use it for anything a workspace must not be able to discover.
- `PermissionDenied` is a *visible* refusal — the thing's existence is not a
  secret, the actor simply may not act on it (e.g. an Observer refused
  clipboard-read).

Choosing the wrong one is a spec violation, not a style choice.

## Invariants

- Adapters return only these kinds; a native failure with no better mapping
  becomes `Internal`.
- The set is closed: new kinds are added only when the contract genuinely needs
  one, and then they benefit every adapter at once.
