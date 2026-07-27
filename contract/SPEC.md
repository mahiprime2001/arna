# Workspace Platform Specification

**Version:** 1.0
**Status:** Frozen
**Date:** 2026-07-16

> **The specification defines observable behaviour, not implementation.**
> Any implementation that satisfies these semantics conforms to the platform, regardless
> of the underlying operating system, virtualization mechanism, or adapter technology.
>
> **The specification is the contract. Adapters, runtimes, and platform integrations are
> replaceable implementation details.**

Changes to a frozen specification are exceptional. Architecture adapts to this document;
this document does not adapt to architecture
([ADR-007](adr/0007-specification-before-platform-integration.md)).

## 1. Scope

This document defines what a **Workspace** is, how it behaves, and what an
implementation MUST guarantee.

It is deliberately free of implementation. No operating system, virtualization
technology, windowing system, wire encoding, or programming language appears in this
document. Per [ADR-007](adr/0007-specification-before-platform-integration.md), this
specification defines the interface; platform adapters implement it. An adapter that
cannot meet a requirement here declares a capability gap (§18) — it does not amend this
document by existing.

Where this document is silent, implementations MUST NOT infer permission (§6.1).

### 1.1 Conventions

**MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are used per RFC 2119.

### 1.2 Non-Goals

These are out of scope by decision, not by omission. They are recorded because a platform
without stated non-goals drifts.

| Non-Goal | Reason |
|----------|--------|
| **Run every desktop application** | Developer workflows first ([ADR-002](adr/0002-primary-audience.md)). Broad application support is not a measure of success. |
| **Replace the host's computer** | A workspace *borrows* compute. The host keeps their machine and keeps working (§14.2). |
| **Gaming** | Outside the product vision. Latency and input demands would distort the architecture for a use case we are not serving. |
| **Remote surveillance** | Forbidden by the trust model (§4.5). Any capability whose primary value is watching someone is not a feature here. |

A proposal that advances a non-goal MUST be rejected on those grounds alone, regardless
of technical merit.

## 2. Terminology

| Term | Meaning |
|------|---------|
| **Host Machine** | The physical machine whose compute is lent, and its attached devices. |
| **Host** (Host User) | The party owning the Host Machine. A *person*, who may connect from anywhere, including a different machine. |
| **Workspace Control Plane** | The single trusted path between a workspace and the platform. See §13.3. |
| **Workspace** | An isolated execution environment. A *place*, not a connection. |
| **Workspace Owner** | The party who owns a workspace and governs its collaboration. |
| **Member** | A party holding a role in a workspace. |
| **Collaborator** | A member who may provide input. |
| **Observer** | A member who may see output but provide no input. |
| **Application** | A program from the catalog, launchable in a workspace. |
| **Catalog** | The set of applications the host makes available to workspaces. |
| **Grant** | An explicit, revocable permission issued by the host. |
| **Policy** | The effective set of rules governing a workspace. |
| **Connection** | A transient attachment of a client to a workspace. Not the workspace. |

## 3. The Workspace

**3.1** A Workspace is an isolated execution environment possessing its own display(s),
its own input, its own clipboard, its own filesystem view, and its own processes.

**3.2** A workspace **is a place, not a connection.** It MUST exist independently of any
connection, MUST be able to run with no members connected (subject to §16.3), and
terminating every connection MUST NOT terminate the workspace.

**3.3** A workspace MUST have a stable identity. Identifiers are disclosed to humans
(invitations) and therefore MUST be unguessable and MUST NOT be sequential.

**3.4** A workspace MUST NOT observe or affect any of the following: the host's display,
input, clipboard, filesystem beyond its grants, processes, or devices; or any other
workspace by any means.

**3.5 Isolation is the defining property of a workspace.** Without isolation there is no
workspace — an environment that cannot enforce §3.4 is not a degraded workspace, it is
not a workspace at all (§18.3).

**3.6** Isolation MUST NOT depend on the cooperation of software running inside the
workspace.

## 4. Roles and Authority

| Role | Authority |
|------|-----------|
| **Host** | Owns the hardware. Final authority. |
| **Workspace Owner** | Governs the workspace and its membership. |
| **Collaborator** | Works in the workspace. Input permitted. |
| **Observer** | Sees the workspace. Input refused. |

**4.1** The Host MAY override any decision of any role at any time, including pausing or
terminating a workspace and revoking any grant. This authority MUST NOT be delegable,
disableable, or restrictable by policy.

**4.2** Host and Workspace Owner MAY be different parties. An implementation MUST NOT
assume they are the same.

**4.3** Role enforcement MUST occur where the workspace executes. An implementation MUST
NOT rely on a client to withhold input it is not entitled to send.

**4.4** Roles MAY change during a workspace's life. Changes MUST take effect on live
connections without requiring a restart or reconnection.

**4.5 Authority does not imply invisibility.** The Host MAY join a workspace as Observer
or Collaborator. When present, the Host's presence MUST be visible to members (§15.4).
An implementation MUST NOT provide covert observation.

The Host owns the hardware, the lifecycle, and the permissions. The Host does **not** own
silent surveillance. Authority is exercised over lifecycle and grants — never by watching
unseen.

**4.5.1** A deployment MAY define a policy permitting observation without the Host
appearing as a member. Such a policy MUST be disclosed to members before they join, and
every observation under it MUST be audited (§17.1). It MUST NOT be the default, and it
MUST NOT be silent.

**4.6 Capability matrix.** Role permissions are explicit per capability, never implied by
the role's name. Defaults:

| Capability | Workspace Owner | Collaborator | Observer |
|------------|:---------------:|:------------:|:--------:|
| View display | ✅ | ✅ | ✅ |
| Keyboard | ✅ | configurable | ❌ |
| Pointer | ✅ | configurable | ❌ |
| Clipboard read (out of workspace) | ✅ | configurable | ❌ |
| Clipboard write (into workspace) | ✅ | configurable | ❌ |
| File transfer | ✅ | configurable | ❌ |

**4.6.1** *Observing is not extracting.* An Observer MUST be refused every capability that
removes data from a workspace, not merely those that inject input. A role permitted to
see but not to touch MUST NOT be able to copy source, credentials, tokens, or environment
data out of the workspace.

**4.6.2** "configurable" means the Workspace Owner MAY grant or withhold the capability
within what host policy permits (§6.4). Where unspecified, the answer is no (§6.1).

## 5. States and Lifecycle

**5.1** A workspace is in exactly one state:

| State | Meaning |
|-------|---------|
| **Created** | Defined; not yet executing. |
| **Running** | Executing, with one or more members connected. |
| **Idle** | Executing, no members connected. |
| **Paused** | Execution suspended; state retained in memory. |
| **Resuming** | Transitioning from Paused or Saved to Running. |
| **Saved** | Not executing; contents persisted durably. |
| **Archived** | Saved, retained, excluded from normal listings. |
| **Deleted** | Contents destroyed; identity retired. |

**5.2** Permitted transitions:

```
Created   → Running | Deleted
Running   → Idle | Paused | Saved | Deleted
Idle      → Running | Paused | Saved | Deleted
Paused    → Resuming | Saved | Deleted
Resuming  → Running
Saved     → Resuming | Archived | Deleted
Archived  → Saved | Deleted
Deleted   → (terminal)
```

**5.3 Pause and Saved are different guarantees.** They MUST NOT be conflated.

**5.3.1 Pause → Resume.** Execution is suspended with memory retained. On resume,
processes MUST continue rather than restart, applications MUST resume exactly, and
connections SHOULD continue where the network permits. A member MUST perceive resumption
as continuation, not as a new session.

**5.3.2 Saved → Resume.** Durable contents are restored; memory is **not** guaranteed to
exist. Applications MUST be expected to restart, and workspace state MUST be restored
from durable contents. An implementation MUST NOT be required to checkpoint live memory
to conform.

**5.3.3** An implementation MAY additionally preserve live memory across Saved, and MAY
offer snapshots. Both are capabilities (§18.2), never requirements.

**5.4** A workspace is **Temporary** or **Saved**:
- **Temporary**: contents MUST be destroyed irrecoverably when it closes.
- **Saved**: contents MUST persist and be resumable.

Both are first-class. An implementation MUST NOT treat either as a degraded form of the
other.

**5.5** Deletion MUST be irrecoverable and MUST destroy workspace contents, not merely
unlist them.

## 6. Permissions and Grants

**6.1 Deny by default.** A workspace has access to nothing of the host's except what has
been granted. The absence of a rule means denial. An implementation MUST NOT treat
silence, ambiguity, or omission as permission.

**6.2** Every grant MUST be explicit, attributable to the host, scoped, and revocable.

**6.3** Grants MAY be issued and revoked while a workspace is running. Both MUST take
effect on the running workspace without requiring a restart.

**6.4 Two layers.** Effective permission is `host policy ∩ workspace-owner policy`. The
Workspace Owner MAY narrow what the host granted and MUST NOT widen it.

**6.5 Undetectability.** **The platform MUST NOT disclose the existence of non-granted
resources through any interface it provides.** A workspace MUST be unable to distinguish
between *"does not exist"* and *"exists but is not granted."*

This is not a courtesy — denial is itself information. A workspace that can probe:

```
Exists?  → Denied
Exists?  → Denied
Exists?  → Not found
```

has learned the shape of what it cannot reach. Therefore an implementation MUST report
non-granted resources as **not existing**, never as *denied*, *forbidden*, or
*unauthorized*.

**6.5.1 Scope of the guarantee.** This obligation binds the interfaces the platform
provides. It is **not** a claim of information-theoretic indistinguishability: timing,
cache, and microarchitectural side channels are outside what a platform of this kind can
enforce, and this specification does not pretend otherwise. Overpromising here would cost
the credibility of every other guarantee in this document.

## 7. Resources

**7.1 Host-provided:** compute, memory, graphics acceleration, storage, network access,
and the application catalog.

**7.2 Member-provided:** keyboard, pointer, display(s), camera, microphone, speakers.
These belong to the member, never to the host.

**7.3 Host Machine devices are never reachable.** The camera and microphone **physically
attached to the Host Machine** MUST NEVER be reachable from a workspace. This is not a
grant — it MUST NOT be grantable, by anyone, including the Host.

**7.3.1** This restricts the **Host Machine**, not the **Host User** (§2). A Host who
joins a workspace as a member connects through a client like any other member, and their
client's camera and microphone are then that member's own devices under §12.4 — even if
they happen to be connecting from the Host Machine itself. The prohibition is on the
runtime reaching hardware attached to the machine; it is not a prohibition on a person.

**7.4** The host MAY impose per-workspace limits on compute, memory, storage, and
bandwidth. Implementations MUST enforce them, and a workspace MUST NOT exceed them.

**7.5** Resource usage MUST be reportable per workspace.

## 8. Filesystem

**8.1** A workspace MUST see a virtual filesystem. It MUST NOT see the host's real
filesystem.

**8.2** Only granted paths appear. Non-granted paths MUST be **invisible**, not merely
unreadable. Path names are information; enumeration is a leak. Access to a non-granted
path MUST fail as *not found*, never as *denied* (§6.5).

**8.3** Traversal above a granted root MUST be impossible.

**8.4** Shares MAY be added and removed while running (§6.3).

**8.5** A grant MAY be read-only or read-write.

**8.6** A workspace MUST NOT determine whether a non-granted path exists (§6.5).

## 9. Clipboard and Transfer

**9.1** Each workspace MUST have its own clipboard, isolated from the host's and from
every other workspace's.

**9.2** Text and images MAY move between a member's client and the workspace, subject to
that member's role (§4.6). Clipboard direction MUST be governed independently: read (out
of the workspace) and write (into the workspace) are separate capabilities and MUST NOT
be granted as one.

**9.2.1** An Observer MUST be refused clipboard read by default (§4.6.1). Data extraction
is not observation.

**9.3** File movement between client and workspace is governed by host policy and by the
member's role (§4.6).

**9.4** Drag and drop MUST be implemented as a managed transfer subject to the same
policy as any other file transfer. It MUST NOT be a side channel around §9.3.

**9.5** Transfers SHOULD be resumable and MUST be auditable (§17).

## 10. Application Model

**10.1** Applications available to a workspace come from the host-curated catalog (§7.1).

**10.2** Applications launch inside the workspace. Every child process MUST inherit the
workspace and its policy. A child process MUST NOT escape its workspace.

**10.3** Multiple instances of an application MUST be permitted.

**10.4** An application crash MUST NOT terminate the workspace or other applications.

**10.5** Closing an application MUST NOT close the workspace.

**10.6 Privilege is scoped to the boundary, not to the operation.** Operations that
affect the Host Machine or cross the workspace boundary MUST require host approval.
Operations contained entirely within the workspace MUST NOT.

OS privilege and platform authority are different things and MUST NOT be conflated.
Elevation *inside* a workspace is not a platform concern: an isolated environment's root
is merely that workspace's root, and it reaches nothing (§3.4). Installing packages,
toolchains, or dependencies inside a workspace is ordinary work and MUST NOT require
approval — the host MUST NOT even be notified.

Approval is required only where an action leaves the workspace, for example: requesting a
new shared folder (§8.4), access to a device (§12.1), printing to a host printer (§12.2),
or modification of host policy (§6).

**10.7 No mandatory integration.** The platform MUST NOT require application-specific
integration for correctness. An application MUST NOT need to be modified, recompiled, or
workspace-aware in order to run correctly, and correctness MUST NOT depend on any
per-application support code.

**10.8** An application MAY *optionally* integrate with the platform for enhanced
behaviour. Optional enhancement is permitted; mandatory integration is not. If an
optional integration is absent, the application MUST still work correctly (§10.7).

## 11. Processes

**11.1** A workspace MUST see only its own processes. Host processes and other
workspaces' processes MUST be **invisible** — not merely protected.

## 12. Devices

**12.1** A workspace has no USB devices by default. The host MAY grant a specific device.
A workspace MUST NOT enumerate devices not granted to it (§6.5).

**12.2** Printing: a workspace MAY print to the member's own printer, or to a host
printer subject to host approval.

**12.3** Workspace audio output MUST be directed to the member's speakers by default.

**12.4** A member's own camera and microphone MAY be made available to their workspace
with that member's consent. The host's MUST NOT be, under any circumstances (§7.3).

## 13. Network

**13.1** Internet access is permitted by default, including APIs, source control, and
package registries.

**13.2** Local network access MUST be blocked: discovery, other hosts, network storage,
routers, printers, and other local devices.

**13.3 The Workspace Control Plane is the only reachable host service.** A workspace MUST
NOT reach services on the host's local network, nor services bound to the Host Machine,
with exactly one exemption: the **Workspace Control Plane** — the single trusted path
carrying clipboard, file transfer, policy requests, approvals, and liveness.

Every platform needs one trusted path; without it there is no clipboard, no transfer, no
approval, and no heartbeat. That path is therefore named, singular, and closed. **This is
the only exemption in this section, and no other MUST be added** — a security model with
one exemption is auditable; one with several is not.

**13.4** The host MAY impose bandwidth limits (§7.4).

**13.5** A workspace MUST NOT reach another workspace over the network unless explicitly
granted.

## 14. Display and Input

**14.1** Each workspace MUST have its own display(s) and its own input, independent of
the host's and of every other workspace's.

**14.2** Input directed at a workspace MUST NOT affect the host's input or any other
workspace's. Concurrent independent use is the core guarantee of this platform.

**14.3** A workspace MAY present multiple displays. The client maps them to the member's
monitors.

**14.4** Displays MAY be added, removed, or resized while running. An implementation MAY
decline this via capability (§18.2).

**14.5** Display resolution and pixel density SHOULD follow the member's client.

**14.6** An Observer MUST receive display output and MUST be refused input (§4.3).

## 15. Collaboration

**15.1** Presence, chat, voice, video, and the activity timeline attach to the
**workspace**, not to a connection, and MUST survive having zero connections.

**15.2** Join flow: a user requests, the Workspace Owner approves, and the Host MAY
override (§4.1).

**15.3 Status disclosure is role-based.** The same undetectability principle that governs
resources (§6.5) governs workspace discovery: a non-member MUST NOT learn which workspaces
exist by probing identifiers.

**15.3.1 To a non-member**, an unknown identifier, a workspace they may not join, and a
workspace that is offline MUST produce a **single indistinguishable response**. An
implementation MUST NOT reveal, by wording, error code, or timing of its own construction,
which of the three occurred.

**15.3.2 To a member**, status MUST be accurate and plainly reported — Running, Idle,
Paused, Saved, Offline, or Deleted. For members, ambiguous failure is a specification
violation: a member who cannot reach their own workspace MUST be told why.

**15.4** Members MUST be able to see who is present, including the Host (§4.5).

## 16. Power and Running Policy

**16.1** A workspace MUST pause when the host sleeps and MUST be resumable afterward.

**16.2** The host MAY define a battery policy that pauses workspaces.

**16.3** Behaviour when no members are connected — pause when empty, or keep running —
MUST be host-configurable.

**16.4** Pausing MUST preserve state; resuming MUST restore it (§5.3).

## 17. Audit

**17.1** The following MUST be recorded: grants, revocations, approvals, denials,
transfers, joins and departures, role changes, and host overrides.

**17.2** The host MUST be able to answer, at any moment, *"what can this workspace see
right now?"*

**17.3** Audit records MUST NOT be readable or alterable from within a workspace.

## 18. Conformance

**18.1** An implementation conforms if it satisfies every MUST in this document.

**18.2 Capabilities.** Some features are not available on every platform. An
implementation MUST declare which capabilities it provides. It MUST NOT silently
emulate, degrade, or fake a capability it lacks. Undeclared capabilities are absent.

**18.3 Isolation is not a capability.** The requirements of §3.4, §3.6, §6, §8, §11,
§12.1, §13.2, §13.3, and §14.2 MUST NOT be declared unavailable. An implementation that
cannot enforce them **does not conform** and MUST NOT run workspaces. There is no
partial-isolation tier.

Isolation is the defining property of a workspace (§3.5). "We disabled isolation because
this platform makes it difficult" is not a capability gap — it means the implementation
is not producing workspaces, and it MUST refuse to start rather than produce something
that resembles one.

**18.4 Platform Independence Principle.** Changing the implementation beneath a workspace
MUST NOT change that workspace's semantics. Across any implementation, and across a
change of implementation over time:

- workspace identity MUST be unchanged;
- policies and grants MUST retain identical meaning and effect;
- roles and authority MUST behave identically;
- state and lifecycle transitions MUST behave identically.

This is deeper than parity of features. Parity says the same things are *available*;
independence says the same things *mean the same*. A workspace whose semantics shift
because the layer beneath it was replaced was never a workspace — it was a view of an
implementation.

**18.5** Capability differences are gaps to be closed, not tiers to be sold.
