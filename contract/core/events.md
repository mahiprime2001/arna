# Core: Events

**Normative.** Events are **core**, not a capability. They are the language the
whole engine speaks: every capability emits them, every adapter forwards them,
every SDK subscribes, every audit log records them.

Mirrors the error model: the **envelope is defined by the core contract**, and
the set of event **kinds is closed** — so, exactly as with errors, an adapter can
**never invent an event, only populate one**.

## The envelope

```
Event {
    id:        EventId          // stable, unguessable identity
    workspace: WorkspaceId      // which workspace
    seq:       u64              // per-workspace monotonic ordering authority
    at:        u128             // wall-clock nanos, best-effort/informational
    actor:     Actor            // System | Member(Role)
    source:    EventSource      // Core | Capability(Capability)
    kind:      EventKind        // the type + payload (closed set)
}
```

The envelope is fixed. `kind` payloads carry **identity and metadata only** —
never content or bytes.

## Kinds (closed, grouped by source)

- **Core lifecycle** — `WorkspaceCreated`, `StateChanged{from,to}`,
  `WorkspaceDestroyed`.
- **Applications** — `ApplicationStarted{app, window}`.
- **Windows** — `WindowOpened{window}`, `WindowFocused{window}`,
  `WindowClosed{window}`.
- **Clipboard** — `ClipboardRead`, `ClipboardWritten` (who + direction, never
  content).
- **Storage** — `ResourceCreated/Modified/Read/Deleted{resource}` (who + which
  resource, never bytes).

New capabilities add their kinds here as their specs land — never before, and
never outside this set.

## Invariants

- **E1 — Immutable.** An event is never mutated after it is created.
- **E2 — Append-only.** The event log only grows; events are never removed or
  reordered.
- **E3 — Ordered per workspace.** `seq` is a per-workspace, strictly increasing
  ordering authority. Ordering *across* workspaces is not defined.
- **E4 — No forbidden data.** A payload never exposes data the contract forbids
  elsewhere: clipboard/storage events carry ids and direction, never content or
  bytes (audit vs. privacy, SPEC §17.1). Enforced by the envelope shape.
- **E5 — Envelope core-defined, payload capability-defined.** A capability owns
  which kinds it emits; it never changes the envelope.
- **E6 — Populated, not invented.** Adapters and capabilities populate this
  envelope; they never define a new event shape (E-analogue of the error rule).

## Conformance

Because events are core, their checks live in `run_core` — every adapter is
tested: creation emits a `Core`/`System`/`WorkspaceCreated` event with the right
envelope; per-workspace `seq` is strictly increasing; kinds carry no
content-bearing fields (a compile-time exhaustive match guards E4).

## Why this matters

Events are where capabilities **compose**. That Applications, Windows, Clipboard,
and Storage all emit through this one envelope with no capability-specific
exceptions is the evidence that the *core* of the platform is coherent — not just
the individual capabilities. Later capabilities (Devices, Network, Permissions,
Collaboration) join the same language for free.
