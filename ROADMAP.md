# WSE Roadmap

**WSE Desktop** — a native Windows workspace platform. Run multiple independent
workspaces on one machine; each owns its applications, windows, clipboard,
storage, and browser profiles, using native Windows apps and standard APIs — no
VMs, no WSL.

## The three phases

The roadmap is one arc in three phases, not a flat list:

```
Specification  →  Implementation  →  Validation
  (the docs)       (generic wiring)    (live in it)
```

**We are leaving Specification.** The v1 contract and the v2 WRM (both boundary
halves — see [docs/](docs/README.md)) are frozen. The next milestone is not a
feature; it is a proof:

> **First LaunchPlan executed without application-specific runtime code.**

When one `LaunchPlan` flows `projector → Runtime::execute → ExecutionContext` and the
native runtime contains *no* `if app == …` branch, the architecture is proven and
every further application is manifest authoring, not engineering. VS Code is the
intended first stress test — it touches nearly every resource class (executable,
extensions, settings, workspace storage, environment, terminals, Git). If VS Code
goes fully manifest-driven, WRM is proven.

## Windows v1 — the Minimum Complete Workspace ✅

- [x] Engine (contract + conformance)
- [x] Native adapter (no WSL / no VM)
- [x] Runtime + isolation models (SealedVm, DesktopProfile)
- [x] Applications
- [x] Windows
- [x] Clipboard
- [x] Storage (the workspace home)

A native workspace now has its own desktop, apps, windows, clipboard, and a
persistent home. It is a place, not a launcher.

## Current goal

→ **Use WSE daily.**

Build the thin shell (see below), then *live in it* for 2–4 weeks and keep notes.
**No new capabilities until real usage proves they're needed.** The most valuable
information now comes from using the product, not from adding to it.

### WSE Shell (v0.1) ✅ built

- [x] list / create / destroy workspaces
- [x] launch apps into a workspace
- [x] **enter** a workspace (switch to its desktop) and return
- [x] suspend

Run it:

```
cargo run -p wse-shell        # or the `wse` binary
```

```
wse> create Work
wse> launch Work browser
wse> enter Work               # you're now ON the workspace desktop
                              #   Ctrl+Alt+Q returns you to your real desktop
wse> destroy Work
wse> quit
```

Workspaces live for the shell session (persisting them across restarts is a
daemon — a [backlog](BACKLOG.md) item). Everything else is in the backlog and is
pulled in **only** when daily use asks for it — not before.

## Now: use it

Live in it. Keep notes on what you reach for and what frustrates you. Those notes —
not speculation — decide what comes off the backlog next.
