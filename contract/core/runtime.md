# Core: Runtime

**Normative.** Every workspace runs on exactly one **Runtime**: the versioned,
immutable execution environment *inside* the workspace. The adapter orchestrates
the platform; the runtime provides the execution. They are separate contract
boundaries.

```
Workspace  →  Runtime  →  Capabilities  →  Applications
```

## Why it's separate from the adapter

The adapter answers "how is this workspace launched and sealed on this platform?"
The runtime answers "what exists inside the workspace?" — Linux userspace, an X
server, a window manager, window discovery, catalog applications, libraries.
Collapsing the two hides the execution environment inside adapter setup code and
makes runs irreproducible. Kept separate:

- The **adapter** orchestrates (import, launch, stop, attest isolation).
- The **runtime** executes (provides the capabilities inside).
- The **Applications** capability never learns whether the runtime is a WSL2
  image, an OCI container, a Firecracker VM, or a remote host.

## The runtime is immutable and versioned

A runtime is an **image**, identified by a content `digest`, carrying a
`RuntimeVersion` (`major.minor.patch`: patch = same behaviour, minor = additive,
major = breaking).

- **Never mutate an image.** A change is a new version and a new digest, never an
  in-place patch. Tests must not install into a running workspace. This is the
  precondition for repeatable conformance ([conformance.md](conformance.md) C2/C3).
- A runtime **declares its capabilities** — deliberately mirroring the adapter
  capability model, because it is the same kind of boundary: a negotiated
  contract, not an assumption.

## Negotiation: adapter ∩ runtime

A workspace **usably provides** a capability only when *both* the adapter can
bridge it and the runtime provides it:

```
effective = adapter.capabilities  ∩  runtime.capabilities
```

Two different concerns, both required. A runtime with no display stack provides no
Applications even if the adapter could bridge it; an adapter that can't bridge a
capability withholds it even if the runtime offers it. This is why turning on
Windows Applications is a *runtime* change (ship the display image) plus an
*adapter* change (declare the bridge) — not either alone.

## Runtime attestation

When a workspace starts and its isolation is accepted, the engine records a
**`RuntimeAttestation`** — the exact environment that ran:

```
runtime id · name · version · digest · capabilities · start time
```

This makes every run and every bug report reproducible. "Applications fail on
wse-linux-x11 v1.3.2 (alpine-3.20+x11, sha256:…)" names the precise environment;
there is no ambiguity about *what* executed. The adapter supplies the runtime
descriptor (including its immutable digest); the engine stamps the time and pins
it to the workspace. It is absent until the workspace is first started.

## Identity

A runtime **definition** is a public, non-secret environment identifier, so its
`RuntimeId` is a stable, meaningful string (`wse-linux-x11`) — not the unguessable
random id used for runtime *instances* (workspaces, resources). The immutable
content identity is the `digest`.

## Conformance

Runtime is core; every adapter's `run_core` verifies it:

- `runtime/adapter_declares_a_runtime` — a named runtime with an immutable digest.
- `runtime/start_records_a_reproducible_attestation` — start pins id + version +
  digest; absent before start.
- `runtime/capabilities_bound_the_workspace` — a workspace never provides a
  capability its runtime doesn't offer.
- `identity_reflects_negotiated_capabilities` — capabilities are adapter ∩ runtime.

## Lifecycle & compatibility

A runtime has a lifecycle analogous to a capability's (the pattern is reused, not
reinvented) — see the full checklist in [../../runtimes/README.md](../../runtimes/README.md):
Intent · Identity · Capability declaration · Versioning · Build reproducibility ·
Attestation · Conformance expectations · Deprecation.

- **Versioning.** `major.minor.patch`. Adding a capability is *at least* a minor
  bump; removing or changing one is a major bump. Adapters pin the version they
  ship.
- **Deprecation is not mutation.** An image is never patched. Retiring a version
  means removing its artifact, never editing it — so every historical
  `RuntimeAttestation` (pinned by digest) stays meaningful and reproducible.
- **Conformance defines "working".** A runtime is only valid once some adapter
  passes `run_all` on it — `run_core` plus one suite per capability the runtime
  declares and the adapter bridges. Interchangeability is the test: the *same*
  adapter on two runtimes runs different suites, with no special cases.

## Runtimes in the repo

Runtime images are built by reproducible recipes under `runtimes/`, versioned and
content-addressed. The image artifact itself lives outside git (it is large and
immutable); the recipe and manifest are tracked. Two exist today —
[`wse-linux-x11`](../../runtimes/wse-linux-x11/) (Applications + Windows) and
[`wse-lite`](../../runtimes/wse-lite/) (nothing) — and the Windows adapter,
unchanged, conforms on both. See [../../runtimes/README.md](../../runtimes/README.md).
