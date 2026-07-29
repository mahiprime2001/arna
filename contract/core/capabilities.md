# Core: Capabilities

**Normative.** A workspace is defined by the capabilities it provides. This file
defines the capability *model*; each capability's behaviour is normative in its
own spec under [../capabilities/](../capabilities/).

## The model

- A **capability** is something a workspace provides: `Applications`, `Windows`,
  `Clipboard`, `Storage`, `Devices`, `Network`, `Audio`, `Camera`.
- An adapter **declares** a `CapabilitySet` — what it provides. Undeclared means
  absent (SPEC §18.2). Never faked.
- The engine **negotiates** on capabilities: it asks "does this workspace provide
  X?" (`engine.supports(id, X)`), never "am I on platform Y?".
- **Isolation is not on this list.** It is the mandatory core (§18.3); it cannot
  be declared unavailable, and there is no partial-isolation tier.

## Each capability is its own interface

A capability is a mechanical trait the adapter implements, exposed through a
negotiation hook (`clipboard()`, `storage()`, …) that returns `None` unless
declared. Policy for the capability lives in the engine (see
[permissions.md](permissions.md)), never in the trait.

## Conformance is capability-gated

`run_all(adapter)` runs the mandatory core suite plus **one suite per declared
capability** — and nothing for a capability the adapter didn't declare. This is
capability negotiation applied to conformance itself.

## The per-capability lifecycle

Spec stays in front of implementation. Every capability follows:

1. **Intent** — the problem it solves.
2. **Contract** — interface + data model.
3. **State model** — the state it owns.
4. **Invariants** — rules that always hold.
5. **Error mapping** — which `WseError`s may arise.
6. **Conformance** — the `run_<capability>` suite.
7. **Reference implementation** — the mock.
8. **Platform implementations** — adapters, later, as consumers.

## Maturity

Each capability carries a status (`Capability::maturity()` in code, table in
[../capabilities/README.md](../capabilities/README.md)):

- **Stable** — shape settled; changes additive. (Applications, Windows)
- **Draft** — specified and conformance-tested; shape may still change.
  (Clipboard, Storage)
- **Planned** — named; not yet specified. (Devices, Network, Audio, Camera)

## Versioning

Today the whole contract shares one version (`CONTRACT_VERSION = v0.1`); adapters
declare it and the engine refuses an incompatible major. **Forward-looking**
(captured, not built): a capability may later carry its own id + version so a
future contract can hold `Clipboard v2` beside `Storage v1` without versioning
everything at once. Contract-level **capability states** (Available / ReadOnly /
Degraded / Offline, mapped from platform reality like errors are) land per
capability when one needs them.
