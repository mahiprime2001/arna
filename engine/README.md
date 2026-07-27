# WSE Engine

The Workspace Engine — a Rust workspace with **zero platform code above the
adapter boundary**. See `../contract/VISION.md` and `../contract/ARCHITECTURE.md`.

```
common/         wse-common        shared vocabulary (no OS)
contract/       wse-contract      the Workspace Contract as Rust traits (no OS)
core/           wse-engine        the orchestrator + lifecycle (knows the trait, never an OS)
adapters/mock/  wse-adapter-mock  the reference adapter: proves the engine with no OS
```

## First milestone (done)

```rust
let mut engine = Engine::new(MockAdapter::new());
let ws = engine.create_workspace(WorkspaceConfig::new("Design review", Persistence::Temporary, catalog))?;
engine.start(&ws)?;                 // only "running" once the adapter proves it's sealed (§18.3)
engine.launch(&ws, "browser")?;     // deny-by-default; unknown app is "not found", not "denied" (§6.5)
engine.launch(&ws, "editor")?;
engine.list_windows(&ws)?;          // -> 2 windows
```

Run: `cd engine && cargo test`. The tests are conformance checks — the lifecycle
state machine, deny-by-default undetectability, and the isolation core — that
every real adapter must also pass.

## Next

- Real Windows adapter (ports `../services/wsl`), passing the same core suite.
- Extract a formal conformance harness from `core/tests/milestone.rs`.
