# ADR-0009: Isolation is a named model, not a single "sealed" flag

**Status:** Accepted. **Supersedes** the binary `sealed`/`require_sealed` shape.

## Context

Isolation is mandatory core (SPEC §18.3): a workspace must be isolated before it
runs, and there is no partial tier. The original contract expressed this as
`IsolationAttestation { sealed: bool }` evaluated by `IsolationPolicy {
require_sealed: true }`. That worked for the mock and the WSL2 adapter, both of
which isolate by *sealing* (no host filesystem, no host interop).

The native Windows adapter (`windows-native`) isolates differently: a workspace is
a **separate desktop** (own input + display) plus an **isolated per-app profile**
(own storage), with native apps on the host. This is real isolation — the owner
keeps working, workspaces don't share input/display or per-app storage — but it is
**not** a sealed VM: it shares the host filesystem. Setting `sealed: true` would be
a lie; setting `sealed: false` would (correctly, under the old policy) refuse to
run a legitimately-isolated workspace. The single boolean baked one platform's
mechanism in as the definition of isolation.

This is exactly the "genuine gap in the contract" our project rule reserves
architectural change for.

## Decision

An attestation **names its isolation model** and attests that model's guarantees
hold:

```
IsolationAttestation { model: IsolationModel, isolated: bool, details: [String] }
IsolationModel = SealedVm | DesktopProfile
IsolationPolicy { accepted_models: Option<Vec<IsolationModel>> }   // None = accept any satisfied model
```

- Each model has a full definition; `isolated: true` means *that model's*
  guarantees all hold. Models are not tiers on one scale.
- The default/conformance policy accepts any recognized, satisfied model, so one
  suite validates adapters of different models. A hardened deployment restricts
  to specific models (`IsolationPolicy::require([SealedVm])`).
- An unsatisfied attestation is always rejected (`IsolationRejected`). No
  workspace runs unisolated.

## Consequences

- The adapter-attests / engine-evaluates split is preserved; only the shape of the
  evidence changed.
- WSL and mock attest `SealedVm`; native attests `DesktopProfile`. All three
  conform under the default policy with no special cases.
- Deployments choose their security posture via policy, not by editing adapters.
- Honesty: `details` state exactly what a model does and does not seal. See
  [../core/isolation.md](../core/isolation.md).
