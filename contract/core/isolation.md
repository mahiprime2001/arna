# Core: Isolation

**Normative.** SPEC §18.3. A workspace must be isolated before it runs. Isolation
is mandatory core — there is no "run it unisolated" option. But *how* a workspace
is isolated differs by platform, so the contract lets an adapter **name its
isolation model** and attest that model's guarantees hold; the engine's policy
decides which models a deployment accepts.

## Attestation names a model

`start` returns an `IsolationAttestation`:

```
model: IsolationModel     # which kind of isolation this workspace provides
isolated: bool            # the adapter attests THAT model's guarantees all hold
details: [String]         # human-readable evidence
```

The adapter provides evidence; it never decides whether it's acceptable — that is
the engine's policy (the same adapter-provides / engine-evaluates split as before).

## The models

| Model | Guarantees | Does NOT | Used by |
|-------|-----------|----------|---------|
| `SealedVm` | no host filesystem, no host interop | — | WSL2, VMs (the strict default) |
| `DesktopProfile` | separate desktop (own input + display), isolated per-app profile (own storage) | **shares the host filesystem** | native Windows app-layer |

A model is **not a tier on one scale.** Each has its own full definition, and
`isolated: true` means *that model's* guarantees all hold. `DesktopProfile` is not
"weaker SealedVm" — it is a different, honestly-labelled boundary. The `details`
state plainly what it does and does not seal.

## Policy accepts models

```
IsolationPolicy { accepted_models: Option<Vec<IsolationModel>> }
```

- `None` (default, and the conformance posture) — accept **any** recognized model
  whose guarantees are satisfied. This is what lets one conformance suite validate
  adapters of different isolation models with no special cases.
- `Some(list)` — a hardened deployment that accepts only those models (e.g.
  `require([SealedVm])` refuses anything but a sealed VM).

Either way, an attestation whose model is **not satisfied** (`isolated: false`) is
always rejected: the engine stops the workspace and returns `IsolationRejected`.
No workspace is ever presented as running unisolated.

## Why this shape (the genuine gap)

Originally the attestation was a single `sealed: bool` and the policy was
`require_sealed`. That baked in one platform's isolation mechanism (a sealed VM)
as *the* definition of isolation. The native Windows adapter exposed the gap: a
separate desktop + isolated profile is real isolation, but it is not a sealed VM,
and faking `sealed: true` would be dishonest. So the binary flag became a named
**model** + **satisfied** flag, evaluated by policy. This is the one contract
change the project rule ("no new architectural work unless the implementation
exposes a genuine gap in the contract") was reserved for — see
[../adr/0009-isolation-models.md](../adr/0009-isolation-models.md). Adapters,
conformance, and the engine's own split (adapter attests, engine evaluates) are
otherwise unchanged.
