# ADR-004: Workspace Identity Model

**Status:** Accepted
**Date:** 2026-07-16

## Context

Remote-desktop products model a **connection**: ephemeral, one viewer, one machine,
gone when you hang up. Our product statement is different:

> **A workspace is a place, not a connection.**

A place has an identity, an owner, members, contents that persist, and a life
independent of whoever is currently looking at it. The prior model — a device list you
dial into — cannot express that.

## Decision

**The Workspace is a first-class, persistent entity**, not a session. It has: an id,
an owner, members with roles, a state, a policy, and (if saved) durable contents.

### Roles

| Role | Authority |
|------|-----------|
| **Host** | Owns the hardware. Final authority; may override anyone, always. |
| **Workspace Owner** | Runs the workspace and its collaboration. Approves joins. |
| **Collaborator** | Works in the workspace. Full input. |
| **Observer** | Sees the workspace. No input. |

**Host and Workspace Owner are deliberately separable.** The host lends the machine;
someone else may own the workspace on it. This split is what makes the product a
platform rather than a remote-desktop tool. (*Moderator is deferred, not rejected.*)

### Two-layer policy

Effective permissions are **host policy ∩ workspace-owner policy**. The owner can only
narrow what the host granted, never widen it.

### Join flow

`User requests → Workspace Owner approves → Host may override.`
If the workspace is offline/unreachable, say so plainly — never fail ambiguously.

### States

`Created → Running → Idle → Paused → Resuming → Saved → Archived → Deleted`

**Temporary** workspaces are destroyed with their data on close. **Saved** workspaces
persist and resume. Both are first-class; neither is a special case of the other.

## Alternatives Considered

**Workspace = session (status quo).** Rejected. Cannot express ownership, membership,
persistence, or a workspace that exists while nobody is connected.

**Workspace = device.** Rejected. One machine hosts many workspaces; conflating them
makes multi-workspace incoherent and leaks host identity into every share.

**Single role (owner + guests).** Rejected. Cannot express Observer, and cannot express
a host lending to an owner who is not the host.

## Consequences

- **Backend data model changes shape**: workspaces + memberships + roles + policies +
  audit, not accounts + devices. This is the main reason the "brain" is being rebuilt
  while the transport pipe is kept.
- Presence, chat, and activity timeline attach to the **workspace**, not the connection
  — they must survive nobody being connected.
- The Runtime must implement real state transitions (pause/resume/save/archive), which
  every adapter must support or explicitly decline via capability flags
  ([ADR-005](0005-platform-adapter-architecture.md)).
- Multiple viewers per workspace become normal, so the protocol needs per-viewer
  identity and per-viewer input authority (Collaborator vs Observer) — enforced at the
  runtime, never only in the UI.
- Workspace ids are shared with humans (invite links); they need to be unguessable, not
  sequential.
