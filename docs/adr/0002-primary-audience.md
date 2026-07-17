# ADR-002: Primary Audience — Individual Developers

**Status:** Accepted
**Date:** 2026-07-16

## Context

"Workspace platform" can aim at very different buyers: enterprise IT (managed fleets,
compliance, SSO), consumers (family sharing, help-my-parents), or individual
developers. Each implies a different product.

The animating use case is concrete:

> *"I can lend my RTX workstation to another developer."*

That is a developer lending to a developer — peer to peer, no IT department, no
procurement.

## Decision

**Primary audience: individual developers.** Everything else is secondary for now.

This does not contradict the broader positioning that Arna is for everyone. It sets
the *order*: we build for developers first, then generalize. A future Personal /
Enterprise split remains open.

## Alternatives Considered

**Enterprise IT first.** Rejected for now. Demands SSO/directory integration, fleet
policy, audit/compliance, and a sales motion — all of which distort the architecture
before we have proven the core loop.

**General consumers first.** Rejected. The remote-help use case is served adequately by
existing remote-desktop tools; our differentiator (isolation + simultaneous
independent use) is worth most to technical users.

## Consequences

- **App catalog is developer-shaped**: editor, browser, terminal, git, package
  managers, language toolchains.
- **Network policy must allow developer traffic** — git, package registries, APIs —
  while still blocking LAN/NAS/printers/IoT.
- **Identity is social, not corporate**: friends lists and invite links, not Active
  Directory. No SSO/SCIM in the near term.
- **Persistence matters.** Developers have projects and long-lived state; a
  wipe-on-close-only model is insufficient. Drives the Saved workspace in
  [ADR-004](0004-workspace-identity-model.md).
- **SDK/CLI are product surface, not afterthoughts** — this audience automates.
- Developers are unusually good at noticing and reporting isolation leaks. Treat early
  users as adversarial testers of the boundary, in a good way.
