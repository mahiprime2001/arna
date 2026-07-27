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
| Storage      | Planned | — |
| Devices      | Planned | — |
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
