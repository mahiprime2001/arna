# ADR 0008 — Adapter discipline

**Status:** accepted · 2026-07-28

## Context

The contract is complete enough for v1: core (identity, errors, events,
capability negotiation, capability states, policy) plus five capabilities, each
with a conformance suite, a reference engine, and a mock adapter. The next
question is not "is the contract complete?" but **"can a real platform satisfy it
without forcing us to change it?"** — the test that turns an internal design into
a genuine specification, once *multiple independent implementations* agree on it.

We are starting the first such implementation (the Windows adapter). Before any
platform code, we fix the rules that keep the boundary honest.

## Decision

Four rules bind every adapter:

1. **An adapter may never change the contract.** If a platform cannot satisfy
   something, the adapter maps it, reports it, or declares the capability
   unavailable — it does not rewrite the specification.

2. **Every failing conformance check is classified before it is fixed**, as one
   of: *implementation bug* · *adapter bug* · *specification ambiguity* ·
   *impossible platform limitation*. **Only the last** justifies evolving the
   contract. Everything else is implementation work.

3. **No platform concepts in the core.** No `HWND`, `HANDLE`, Win32, COM,
   registry, or NT object names in `wse-common`/`wse-contract`/`wse-engine`.
   Those live exclusively inside the adapter.

4. **The adapter is a translation layer**, mapping *platform → contract →
   engine*, never *engine → platform*. The engine never branches on a platform.

## The first milestone

Not "it works" — a **minimal, truthful** adapter:

```
run_core        ✓   (real lifecycle + isolation attestation)
run_applications  CapabilityUnavailable ✓
run_windows       CapabilityUnavailable ✓
run_clipboard     CapabilityUnavailable ✓
run_storage       CapabilityUnavailable ✓
run_devices       CapabilityUnavailable ✓
```

A minimal adapter that truthfully reports what it supports beats a partial one
pretending to support everything. Capabilities are then added in stabilisation
order (Applications, Windows, Clipboard, Storage, Devices), each completed
capability enabling its conformance suite.

## Ambiguities are data

Every point where mapping a platform to the contract is genuinely unclear is
recorded as a design note. If "the platform is simply different", the adapter
absorbs it. Only if "the contract cannot express this for *any* platform" does
the specification evolve (Rule 2, category four).

## Consequence

When the Windows adapter passes every suite for the capabilities it declares —
with no platform branches in the engine and no reshaping of the contract — WSE
has *two independent implementations conforming to one standard*. That is the
threshold at which the contract graduates from a design document into a platform
specification, and only then do we design distributed capabilities (Network,
Collaboration).
