# Core: Identity

**Normative.**

> Every persistent object the Workspace Engine exposes has a **stable,
> unguessable, immutable contract identity.**

One pattern, everywhere: `WorkspaceId`, `ResourceId`, `WindowId`, `MemberId`,
`EventId` — and future `ApplicationId`, `DeviceId`.

## Invariants

- **ID1 — Stable.** An id names one object for its entire life.
- **ID2 — Immutable.** Changing an object's metadata (e.g. renaming a resource)
  never changes its id.
- **ID3 — Unguessable.** Ids are drawn from enough randomness that they cannot be
  probed for (SPEC §3.3). They are shown to humans in invitations, so they must
  not be sequential.
- **ID4 — Terminal.** A deleted id never resolves again and is never reused.
  Operations on a deleted id fail as `NotFound`.

## Why one rule

Because every object obeys the same identity contract, any object can be
referenced, audited (its id appears in events), and reasoned about the same way
across every capability. Identity is not a per-capability concern — it is core.

*(Implementation note: the current generator is std-only and merely
non-sequential; a real adapter MUST use a CSPRNG.)*
