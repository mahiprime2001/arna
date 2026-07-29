# Workspace Capabilities

A workspace is defined by the capabilities it provides. Each capability is its
own mini-specification with its own conformance suite. An adapter declares which
it provides (SPEC §18.2); the engine negotiates on capabilities, never on
platform names; and conformance is capability-gated — an adapter is tested only
for what it declares.

## Maturity

Not every capability is complete at once. Maturity is a signal — to us now, to
contributors later — of which parts of the contract are expected to still move.

| Capability   | Status  | Spec |
|--------------|---------|------|
| Applications | Stable  | (in engine + core conformance) |
| Windows      | Stable  | (in engine + core conformance) |
| **Clipboard**| **Draft** | [clipboard.md](clipboard.md) |
| **Storage**  | **Draft** | [storage.md](storage.md) |
| **Devices**  | **Draft** | [devices.md](devices.md) |
| Network      | Planned | — |
| Audio        | Planned | — |
| Camera       | Planned | — |

- **Stable** — the shape is settled; changes are additive.
- **Draft** — specified and conformance-tested, but the shape may still change.
- **Planned** — named in the capability model; not yet specified.

The status is also machine-readable: `Capability::maturity()` in `wse-common`.

## The lifecycle every capability follows

Spec stays in front of implementation:

1. **Intent** — the problem this capability solves.
2. **Contract** — the public interface and data model.
3. **State model** — what state the capability owns.
4. **Invariants** — rules that must always hold.
5. **Error mapping** — which `WseError` values are allowed, and when.
6. **Conformance tests** — executable requirements (the `run_<capability>` suite).
7. **Reference implementation** — the mock adapter.
8. **Platform implementations** — Windows, Linux, macOS (later, as consumers).

A capability reaches **Draft** at step 7 and **Stable** once a real adapter has
passed its suite and the shape has held.

## Forward-looking (captured, not yet built)

Two ideas to introduce when they earn their place — deliberately *not* built yet:

- **Capability states, at the contract level.** *(Now real, introduced with
  Devices.)* `CapabilityState` (`Unavailable`/`Available`/`ReadOnly`/`Degraded`/
  `Offline`) is a **contract** state, never a platform state — the adapter maps
  its reality into it (a crashed driver → Degraded), same as it maps errors.
  `engine.capability_state(ws, cap)` reports it, and a change emits
  `CapabilityStateChanged` through the core event envelope. Devices uses it
  today; other capabilities adopt it when they need richer states.

- **Independent capability versioning.** A capability may carry its own
  `CapabilityId` + version + maturity, independent of the Workspace Contract
  version, so a future contract can hold `Clipboard v2` alongside `Storage v1`
  without versioning everything at once. For now, maturity (Stable/Draft/Planned)
  is the only per-capability signal; explicit per-capability versions come when
  the first capability needs a breaking change.

## Universal rule: stable contract identity

> **Every persistent object the Workspace Engine exposes has a stable,
> unguessable, immutable contract identity.**

`WorkspaceId`, `ResourceId`, `WindowId`, `MemberId` — and future `ApplicationId`,
`DeviceId` — all follow the one pattern. An id names one object for its whole
life; changing the object's metadata never changes its id; a deleted id never
resolves again and is never reused. This is what lets any object be referenced,
audited, and reasoned about the same way across every capability.

## Policy note

Authorization (which role may exercise which access right on a capability) lives
in the **engine**, behind an `Authorizer` interface — not inside any capability.
Capabilities are mechanical; the engine asks the policy system, the policy system
answers. Today that's a simple role matrix (`RoleMatrixAuthorizer`); it can grow
into a full Permission Manager without any capability changing.
