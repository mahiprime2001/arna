# ADR-0011: Workspace restore + resume (architecture v3)

**Status:** Accepted. Architecture version: **v3** (first change past the ADR-0010
freeze). Milestone: persistence — *close WSE → reopen → resume where you stopped.*

## Context

ADR-0010 froze the architecture and reserved v3 for a change proven necessary by a
real implementation. Building **workspace persistence** surfaced exactly that: two
gaps the frozen engine cannot express.

1. **No restore.** `create_workspace` *mints* a fresh `WorkspaceId` and there is no
   way to bring a workspace back under its **existing** id after a process restart.
   Native workspace state on disk (home, profiles, overlay) is keyed by that id, so
   "reopen → the same workspace, same files" is unreachable by driving the engine.
2. **No resume transition.** The lifecycle (`WorkspaceState::can_transition`) defines
   `Saved → Resuming → Running`, but **no method performs `Saved → Resuming`**.
   `start` only accepts `Created`/`Resuming`/`Idle → Running`, so a Saved workspace
   can never return to Running. `start` is even documented as "Start (or resume)" —
   the intent was always there; the mechanism was missing.

This is the genuine-gap case ADR-0010 reserves engine change for: a real product
need demonstrating the architecture cannot express a required behavior. Docker
workspaces (state lives in the container/volume, managed outside the engine) did
**not** need this — only the engine-tracked runtimes do.

## Decision

Two minimal, additive changes to the engine lifecycle. Neither alters the contract's
shape, the state machine (SPEC §5.2 unchanged), or any existing behavior.

1. **`Engine::restore(id, cfg)`** — the counterpart to `create_workspace`. It adopts
   a **caller-provided** id instead of minting one, reconstructing the engine record
   and the adapter's runtime state for the workspace's on-disk home. Comes back in
   `Created`; Resume is `start`. `create_workspace` now delegates to the same
   internal `instantiate`, so the two share one path and differ only in the id.

2. **`start` resumes from `Saved`** — when a workspace is `Saved`, `start` first
   performs the `Saved → Resuming` transition the state machine already defines, then
   proceeds `Resuming → Running`. This completes the documented "Start (or resume)"
   behavior without adding a state or a rule.

The product layer (the Tauri app) persists the metadata needed to reconstruct a
workspace (id, name, selected apps, browser choice) and, on reopen, calls `restore`
then `start` — reconstructing the runtime rather than pretending old processes
survived, exactly as intended.

## Consequences

- **Ids are stable across restarts.** No id-churn, no home-rename hacks; the on-disk
  home matches because `restore` adopts the persisted id.
- **The lifecycle is now complete.** Every state the machine defines is reachable by
  a method. This is also the foundation for **snapshots** and **cloud runtimes**
  (restore-into-a-different-runtime is the same shape).
- **Scope discipline held.** The change is two additive lifecycle operations proven
  by a real need — not a redesign. Everything else (persisting metadata, the resume
  UX) stayed in the product layer. Docker persistence needed no engine change at all.
- v1 (contract) and v2 (WRM) remain frozen; v3 = v2 + these two lifecycle
  completions. The freeze rule stands: the next engine change again needs an ADR.
