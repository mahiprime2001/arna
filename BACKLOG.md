# WSE Backlog

Not scheduled. These are pulled into [ROADMAP.md](ROADMAP.md) **only when real
daily use proves they're needed** — not on speculation. Each line notes the *wish*
that would justify it.

## Capabilities
- **Devices** (external resources) — deferred; nobody can experience WSE yet, and
  Devices won't change that. A shell will.
- **Network**
- **Collaboration / multi-user**

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
