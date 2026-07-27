# ADR-003: Hardware Ownership Model

**Status:** Accepted
**Date:** 2026-07-16

## Context

The platform's core promise is that a host lends compute **without giving away their
computer**. That only holds if it is unambiguous who owns what, and if the default for
anything unstated is "not shared."

Remote-desktop tools get this wrong by construction: they hand over the whole machine,
the real desktop, the real cursor, the real filesystem.

## Decision

Three categories, with a hard default.

**Host-owned** — lent to workspaces, never surrendered:
CPU, RAM, GPU, storage, **host-curated application catalog**, internet connection.

The catalog is the set of applications the host chooses to make available inside
workspaces (editor, browser, toolchains, and so on). It is **provided by the workspace
runtime, not shared from the host's own installations** — the host's installed software
is never directly exposed to a workspace. "The host lends apps" means the host *decides
the catalog*, not that host binaries are reachable.
See [ADR-006](0006-windows-adapter-rendering-input.md).

**User-owned** — belongs to the person in the workspace, never the host's:
keyboard, mouse, monitor(s), camera, microphone, speakers.
The host's camera and microphone are **never** reachable from a workspace.

**Host-controlled** — exists only via an explicit, revocable grant:
USB devices, printing, shared folders, resource limits.

**The default is deny.** A workspace sees nothing of the host unless the host granted
it. Absence of a rule means "no", never "yes".

**The host is the final authority.** The host can inspect, pause, resume, or terminate
any workspace at any time, and can override the Workspace Owner
([ADR-004](0004-workspace-identity-model.md)). This is not negotiable by policy.

**Threat model: invited guests.** Workspaces are shared with friends and people the
host invites — not anonymous strangers. We defend against accident, curiosity, and
ordinary malware. We do not currently defend against a determined attacker with a
kernel exploit; see [ADR-006](0006-windows-adapter-rendering-input.md) for what
today's boundary does and does not cover.

## Alternatives Considered

**Share-by-default with opt-out.** Rejected. Every default-share is a leak waiting for
someone to not-read a dialog, and it inverts the product promise.

**Full filesystem access with permission prompts.** Rejected. Prompt fatigue converts
into blanket approval, and the workspace would still be able to *enumerate* what
exists — path names alone leak plenty.

**Host as just another peer (no override).** Rejected. It is the host's physical
machine, electricity, and risk. Ultimate authority follows ownership.

## Consequences

- The Policy Engine is load-bearing, not a feature: every host grant is a policy object
  with a lifetime, and revocation must take effect on a live workspace.
- Grants are **dynamic** — folders can be shared and unshared while a workspace runs.
- Any adapter that cannot enforce deny-by-default is not shippable, regardless of how
  good its performance is.
- Host UI needs a always-available kill switch and a truthful view of what is currently
  granted. "What can they see right now?" must be answerable in one glance.
- Camera/microphone for *collaboration* (talking to each other) is a client-side
  concern and must never be implemented by reaching for host devices from the runtime.
