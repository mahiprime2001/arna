# Writing a WSE Adapter

Practical, not normative. If you can build an adapter from this guide without
asking architectural questions, the standard is successfully separated from its
implementations. The normative rules are in [contract/](contract/CONTRACT.md);
the discipline is [ADR-008](contract/adr/0008-adapter-discipline.md).

An adapter is the **only** place platform code lives. It is a **translation
layer**: platform → contract → engine, never the reverse.

---

## 1. What you must implement

Implement `WorkspaceAdapter` (in `wse-contract`). Only four methods are
mandatory — lifecycle + isolation:

```rust
fn create(&mut self, def: &WorkspaceDef) -> Result<()>;
fn start(&mut self, id: &WorkspaceId) -> Result<IsolationAttestation>;
fn stop(&mut self, id: &WorkspaceId) -> Result<()>;
fn destroy(&mut self, id: &WorkspaceId) -> Result<()>;
```

Plus `capabilities()` (what you declare) and `contract_version()` (defaulted).
Everything else is a **capability hook** that returns `None` until you provide it.

## 2. Start minimal and truthful

Declare **no capabilities** first:

```rust
fn capabilities(&self) -> CapabilitySet { CapabilitySet::none() }
```

Now `run_all` == `run_core`: you pass lifecycle + isolation and every capability
is honestly `CapabilityUnavailable`. A minimal honest adapter beats a partial
pretending one. This is the whole point of capability negotiation.

## 3. Grow by shrinking the unsupported surface

The roadmap writes itself: every capability is `CapabilityUnavailable` until you
declare it and implement its hook, which turns on its `run_<capability>` suite.

```
CapabilityUnavailable  →  declare + implement hook  →  run_<capability>() passes
```

Recommended order (matches how the capabilities stabilised): Applications →
Windows → Clipboard → Storage → Devices.

To add a capability: (a) `.with(Capability::X)` in `capabilities()`, (b)
implement the mechanical trait `XCapability`, (c) return `Some(self)` from the
`x()` hook. The engine handles policy, events, and gating; you provide mechanics.

## 4. Isolation: attest, don't assert

`start` returns an `IsolationAttestation` — **evidence**, not a claim. Measure
and check (e.g. read `/proc/mounts`, probe interop), report `sealed` plus
human-readable `details`. The **engine** evaluates it against policy and decides
whether to run. If you can't prove the seal, return `sealed: false`; the engine
will refuse and the workspace never appears running. There is no
partial-isolation tier.

## 5. Map failures; never invent

- **Errors:** return only `WseError` variants. Map a native failure with no
  better fit to `Internal`. Never invent an error kind. Note the sharp line:
  something a workspace must not detect → `NotFound`; a visible role refusal →
  `PermissionDenied`.
- **Events:** you don't emit events — the engine does, through the fixed
  envelope. You never define an event shape.
- **Identity:** mint stable, unguessable, immutable ids (a CSPRNG). A deleted id
  never resolves or is reused.

## 6. What must never happen inside an adapter

- Never leak a platform concept upward (`HWND`, `HANDLE`, registry, paths…).
  They stay inside the crate.
- Never change the contract to fit the platform. Map it, or declare it
  unavailable, or record a spec-ambiguity note (ADR-008 Rule 2).
- Never make the engine branch on your platform. If the engine would need to
  know it's you, the boundary is wrong.

## 7. Isolate platform APIs

Keep all platform calls behind private helpers in the adapter crate; only
dependencies the adapter needs (not `common`/`contract`/`engine`, which stay
dependency-free). The Windows adapter, for example, shells to `wsl.exe` and keeps
UTF-16 decoding, `/proc/mounts` parsing, and hardening entirely private.

## 8. Prove it — one line

Your conformance test is identical to every other adapter's:

```rust
#[test]
#[ignore = "requires the platform; creates/destroys real workspaces"]
fn adapter_is_conformant() {
    wse_conformance::run_all(MyAdapter::new).assert_ok();
}
```

Mark it `#[ignore]` if it touches real resources; run with `--ignored`. The
suite is **self-cleaning** — it destroys every workspace it creates — so it is
repeatable (see [contract/core/conformance.md](contract/core/conformance.md)).

## 9. Debugging a failing check

1. Read the failing check name; it maps to a spec invariant.
2. **Classify** it (ADR-008 Rule 2): implementation bug · adapter bug · spec
   ambiguity · impossible platform limitation.
3. Only the last justifies a contract change — and that's a discussion, with a
   design note, not a quiet edit. Everything else is your adapter's work.

## Reference implementations

- `engine/adapters/mock` — the reference; the executable definition of
  conforming. Read this first.
- `engine/adapters/windows` — the first real platform adapter (WSL2). Read this
  to see the translation-layer pattern against a real OS.
