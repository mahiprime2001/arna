# Host Resource Projection (design)

**Status: design captured, not yet built.** The next major subsystem for the
native runtime. Its detailed shape (which resources, what granularity) should be
driven by real use — see "When to build" below.

> **This is the motivation. The full, generic model is now in
> [resource-projection-model.md](resource-projection-model.md)** (WSE v2) — where
> apps become data, resources have classes + projection modes, and one
> `ResourceProjector` replaces per-app code. Read that for the architecture; this
> doc for the "why".

## The idea

Turn the user's question from:

> "Should I create a virtual environment / how do I virtualize Python?"

into:

> **"What parts of the host should this workspace be allowed to see?"**

A workspace doesn't *copy* or *install* anything. It receives a **projection** of
the host — a host-controlled *view*. Whatever isn't projected simply doesn't exist
inside the workspace. This is the resource-virtualization reframe applied to the
host environment itself.

## Resources are layered (like a real OS)

The host's tools are not one thing. Each splits into layers, and each layer is
projected independently:

| Resource | Executable | User data | Config | Credentials/Secrets |
|----------|:---:|:---:|:---:|:---:|
| **Python** | python.exe | site-packages | — | — |
| **Node** | node/npm | global packages | .npmrc | npm tokens |
| **Git** | git.exe | — | .gitconfig | SSH keys, cred store |
| **VS Code** | Code.exe | extensions | settings.json | — |
| **Chrome** | chrome.exe | bookmarks, history | — | cookies, passwords |

Example — the host allows Python's executable but not its packages:
```
python           works
pip list         (almost empty)
```
Allow packages too, and `pip list` shows numpy/flask/pandas — the host's.

## Profiles, not 100 checkboxes

The default UX is a profile, with Custom for full control:

- **Clean** — executables only; nothing else. A fresh machine.
- **Development** — runtimes + packages + VS Code extensions; **no** credentials
  (no SSH keys, no git creds, no browser cookies).
- **Personal** — full browser profile (bookmarks, passwords, extensions).
- **Custom** — the full per-resource matrix.

This is also the security model: invite a collaborator with **Development**, and
they can work — but never see your SSH keys, git credentials, or cookies.

## How it maps to native Windows (concretely — this is buildable)

Projection is **not** filesystem magic. On the native runtime it's three
mechanisms, all things the launcher already touches:

1. **A projected environment at launch.** Launch each app with a custom
   environment block (CreateProcess): `PATH` (which executables resolve),
   `PYTHONPATH` / a fresh `site-packages`, `HOME`/`USERPROFILE` pointed at the
   workspace home (so apps look there for config), `GIT_CONFIG`, etc. Deny a layer
   by simply not putting it on the path/env.
2. **Selective projection of user-data dirs.** Point an app at the host's real
   data dir (projected) or a fresh one (denied), per layer:
   - VS Code: `--extensions-dir` → host's `~/.vscode/extensions` **or** a fresh dir.
   - Chrome: a `--user-data-dir` seeded with only the allowed profile pieces
     (Bookmarks yes, Cookies no).
   - Git: copy `.gitconfig` into the workspace home; leave `.ssh` out.
3. **Executable discovery.** Scan the host for python/node/git/VS Code/Chrome and
   offer them; the workspace's PATH exposes only the projected ones.

**We already ship primitives of this**: VS Code launches with an isolated
`--extensions-dir` (that's "extensions: denied"); the browser profile importer is
"Chrome user-data: projected". Host Resource Projection generalises those into one
permission-based subsystem.

## Where it sits in the architecture

```
Host: applications · runtimes · user data · config · credentials · secrets
                          │
              Host Resource Projection   ← the host decides what is projected
                          │
Workspace: applications · environment · clipboard · storage · overlay
```

It's a new **capability family**: per-resource, permission-based. Consistent with
the contract — the **engine owns the policy** (what each role/profile may project),
the **adapter applies** the projection at launch. Different runtimes realise it
differently: native via env + selective data dirs; Docker inherently (the container
*is* a clean projection, and you opt-in to mounting host bits).

## When to build

Not yet. The value is in the projection *matrix*, and the right matrix comes from
real use — you'll know what to project the first time you think "damn, I wish this
workspace had my X" or "I don't want it to see my Y." Today native + Docker +
overlay are all unvalidated by daily work.

**First proof slice (small, when ready):** a `Clean` vs `Development` profile on
the native create dialog that controls the one layer we can already toggle for
free — VS Code extensions (host's vs fresh) — plus a projected `PATH`. That
demonstrates the whole concept with mechanisms that already exist, before building
the full per-resource matrix.
