# Core: Conformance

**Normative.** Conformance is what makes an implementation a workspace at all.
`run_all(adapter)` is the mandatory-core suite plus one suite per declared
capability; passing it is the definition of "conforming".

The suite verifies two different things, and both matter:

- **Correctness** — the adapter behaves as the contract requires.
- **Repeatability** — the suite itself behaves the same every time it runs,
  against any adapter, including adapters with real persistent state.

## The rules of the standard test suite

These are properties of the *suite*, not of any adapter. They became first-class
the moment the suite met a real adapter (WSL2 distros are not in-memory):

- **C1 — Independent.** Each check builds its own fresh adapter and engine and
  shares nothing with another check.
- **C2 — Repeatable.** Running the suite twice produces identical results.
- **C3 — Self-cleaning.** Every check leaves the system in the **same observable
  state it found it in** — it destroys every workspace (and thus every distro,
  file, or handle) it created.
- **C4 — Order-independent.** No check depends on another having run first.
- **C5 — Interruption-safe.** A run may be interrupted without corrupting future
  runs. (Best-effort: an aborted process may leave state; a subsequent run's
  fresh adapters and cleanup absorb it.)
- **C6 — Deterministic verdict.** The pass/fail result is a function of the
  adapter's behaviour, not of timing or environment noise.

## How it's enforced

The harness wraps each check's engine in a **self-cleaning test engine** that
destroys every workspace it created when it drops (C3). Production `Engine` never
does this — a Saved workspace must survive a restart — so the teardown lives only
in the harness. This is why the live Windows suite creates and destroys real
distros and leaves none behind.

## Classifying a failure

When a real adapter fails a check, classify it before fixing (ADR-008 Rule 2):

```
implementation bug | adapter bug | specification ambiguity | impossible platform limitation
```

Only the last justifies changing the contract. The first live Windows run already
proved the discipline: a core check that called an Applications-capability op was
an *implementation bug* in the suite, fixed without touching the contract.
