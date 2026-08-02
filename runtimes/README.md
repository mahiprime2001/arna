# WSE Runtimes

A **runtime** is the immutable, versioned execution environment *inside* a
workspace. The adapter orchestrates the platform; the runtime provides the
execution. Normative contract: [contract/core/runtime.md](../contract/core/runtime.md).

Runtimes are the second extension point of WSE (adapters are the first). The same
adapter runs on any runtime; a workspace's usable capabilities are
`adapter ∩ runtime`.

## Runtimes in this repo

| Runtime | Version | Provides inside | Purpose |
|---------|---------|-----------------|---------|
| [wse-linux-x11](wse-linux-x11/) | 1.0.0 | Applications, Windows | the display-stack runtime (Xvfb + openbox + xterm + xdotool + launcher) |
| [wse-lite](wse-lite/) | 1.0.0 | (none) | deliberately minimal/headless — proves runtime interchangeability |

**Interchangeability is proven live:** the Windows adapter, unchanged, passes
`run_all` = 21/21 on wse-linux-x11 and 12/12 (core only) on wse-lite. Same code,
different runtime, different effective capability set — no special cases. See
`engine/adapters/windows/tests/conformance.rs`.

## The runtime lifecycle (what makes a valid WSE runtime)

Deliberately analogous to the capability lifecycle — reusing the pattern, not
inventing a new one. A runtime is valid when:

1. **Intent** — it states what execution environment it provides, and for whom.
2. **Identity** — a stable, public `RuntimeId` (a meaningful name), and an
   immutable content `digest`. Definitions are public; instances are unguessable.
3. **Capability declaration** — it declares exactly the capabilities it provides
   inside the workspace, honestly. Undeclared means absent. These are negotiated
   against the adapter (`adapter ∩ runtime`), never assumed.
4. **Versioning** — `major.minor.patch`: patch = same behaviour, minor = additive,
   major = breaking. A capability change is at least a minor bump.
5. **Build reproducibility** — produced by a tracked `build.sh` with no manual
   steps: fixed manifest structure, a unique digest, builder torn down. The image
   artifact is content-addressed and lives outside git; the recipe is tracked.
6. **Attestation** — starting a workspace records a `RuntimeAttestation` pinning
   id + version + digest + capabilities + time, so every run is reproducible.
7. **Conformance expectations** — a runtime is only "valid" once some adapter
   passes `run_all` on it: `run_core` (12) always, plus one suite per capability
   the runtime declares *and* the adapter bridges. Passing is the definition of a
   working runtime, exactly as it is for adapters.
8. **Deprecation / compatibility** — an image is **immutable**: never patched. A
   change is a new version + digest. Old versions remain valid and runnable by
   digest; adapters pin the version they ship. Retiring a version is removing its
   artifact, never mutating it — so historical attestations stay meaningful.

## Runtime services (internal building blocks)

A runtime provides each capability through a small internal **service** — a script
or program inside the image that the adapter calls. These are **not contract
concepts** and are never exposed publicly; they are the reusable internal
structure of a runtime, documented here so every runtime doesn't reinvent it.

```
Runtime image
├── display service    (start-display.sh)  — brings up Xvfb + WM; needed by the below
├── launcher service   (launch.sh + apps.conf) — Applications: map entry -> command, return window
├── clipboard service  (clip.sh)           — Clipboard: X11 CLIPBOARD selection, durable ownership
├── storage service    (planned)           — Storage: workspace-owned persistent resources
└── device service     (planned)           — Devices: external resources
```

Rules for a service:

- **It knows nothing about WSE.** No roles, no policy, no events, no error
  vocabulary. It performs a mechanical operation and reports success/failure and
  data. The adapter translates its result into the contract (and `WseError`); the
  engine owns policy and events.
- **It is addressed by a stable in-image path** (`/opt/wse/<service>.sh`) so the
  adapter needs no per-runtime knowledge beyond "call the clipboard service".
- **It backs the capability with a real OS mechanism** where one exists (the
  clipboard service uses the X11 CLIPBOARD selection via `xclip`), with durable
  fallback so ownership survives the adapter's separate invocations.

`wse-lite` provides *no* services — that is exactly why it provides no
capabilities. Adding a service to a runtime (plus the matching adapter bridge) is
how a capability turns on: `effective = adapter ∩ runtime`.

## Adding a runtime

Create `runtimes/<name>/` with a `build.sh`, a `manifest.v<major>.json`, any
tracked in-image scripts under `image/`, and a `.gitignore` for `dist/`. Declare
its capabilities in the manifest; build it; point an adapter at it (the Windows
adapter takes a `RuntimeSpec`). Then prove it with `run_all`.
