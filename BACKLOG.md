# WSE Backlog

Not scheduled. These are pulled into [ROADMAP.md](ROADMAP.md) **only when real
daily use proves they're needed** — not on speculation. Each line notes the *wish*
that would justify it.

## WSE v2 — Resource Projection (the architecture)
- **[Resource Projection Model](docs/resource-projection-model.md)** — the generic
  engine: apps are *data* (manifests), resources have classes (executable /
  package / config / data / credential / cache), each projected by a mode (host /
  overlay / workspace / merge / temporary / deny). One `ResourceProjector` builds a
  Launch Plan; runtimes execute it at their own isolation strength. Subsumes the
  Overlay Engine (built) + Host Resource Projection. **First slice when ready:** the
  projector for VS Code with Clean/Development profiles.
- [Host Resource Projection](docs/host-resource-projection.md) — the motivation.
- **Workspace Overlay** — engine built + unit-tested (share → diff → merge/discard);
  needs Tauri commands + a "Changes" review panel + launch-on-overlay wiring.

## Capabilities
- **Devices** (external resources) — deferred; nobody can experience WSE yet, and
  Devices won't change that. A shell will.
- **Network** — build in layers (shared → firewall rules → virtual adapter →
  private network); don't jump to full virtualization.
- **Collaboration / multi-user** — invite with a projection profile that denies
  secrets (see Host Resource Projection).

## Experience (most likely to surface from daily use)
- **Snapshots** — "I wish opening a workspace restored my apps." Storage already
  owns all persistent state under one home, so a snapshot is a copy.
- **Workspace restore / sessions** — "I wish Chrome remembered where it was."
- **Controlled clipboard sync** — "I wish I could drag something between
  workspaces." (Mode already designed; needs a UI trigger.)
- **`workspace://` paths** — resolve `workspace://documents` → the home subdir.
  (Subdirs already exist.)
- **Workspace search / indexing** — "I wish I could search all workspaces."
- **Global switch hotkeys** — `Ctrl+Win+1..N` to jump between workspace desktops.

## Platform
- **Persistent daemon** — workspaces survive across shell restarts / reboot
  (today a workspace lives for the shell session; a daemon holds them longer).
- **Second platform adapter** (Linux / macOS) — once native Windows is
  battle-tested through daily use.
- **App catalog expansion** — VS Code, terminals, Git, dev tools, each with a
  SupportLevel (Certified / Compatible / Experimental).

---

The rule: a wish from real use → a backlog line → (maybe) the roadmap. Never the
other way around.
