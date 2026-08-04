# WSE v2 — The Resource Projection Model

**Status: architecture design. No code yet.** This is the generic model the last
several iterations were converging toward. It subsumes
[Host Resource Projection](host-resource-projection.md) (the motivation) and the
Overlay Engine (already built) into one engine.

> The engine knows nothing about Python, Chrome, or VS Code. **Applications are
> consumers of resources. WSE virtualizes resources.** An application is *data* (a
> manifest), not code.

Once this holds, adding support for a new app isn't writing Rust — it's describing
resources. And every feature discussed (clean environments, shared folders,
merge/discard, selective host access, reusable runtimes) becomes a different
*configuration* of the same engine.

---

## 1. Resources — the taxonomy

Everything an app touches falls into a small set of **classes**. The class decides
which projection modes make sense and what the safe default is.

| Class | What it is | Examples |
|-------|-----------|----------|
| **Executable** | the binary + its bundled runtime | `python.exe`, `chrome.exe`, `Code.exe`, `git.exe` |
| **Package** | installable additions | pip site-packages, npm globals, VS Code / Chrome extensions |
| **Config** | non-secret settings | `.gitconfig`, `settings.json`, `.npmrc` |
| **Data** | user-created content | bookmarks, history, recent files, documents |
| **Credential** | secrets | SSH keys, cookies, saved passwords, tokens, cred stores |
| **Cache** | regenerable derived data | build caches, package caches, thumbnails |
| **Environment** | the *wiring* that connects the above | `PATH`, `HOME`, `PYTHONPATH`, `JAVA_HOME` |

A resource has: a **class**, a **host location** (path and/or env var), and
optionally **sub-layers** (Chrome's `data` splits into bookmarks / cookies /
passwords / extensions — each its own class).

Environment is special: it's *derived* (computed from the other resources'
projections), not stored. It's the output, not an input.

---

## 2. Projection modes — how a workspace sees a resource

| Mode | Meaning |
|------|---------|
| **host** | the workspace uses the host's real resource (shared; read-write or read-only) |
| **overlay** | starts as the host's; changes go to a workspace layer (copy-on-write); the host is untouched until **merged** — the Overlay Engine |
| **workspace** | a fresh, empty, workspace-owned resource — the "clean" default |
| **merge** | reads combine host + workspace; writes go to the workspace |
| **temporary** | workspace-private and wiped on close |
| **deny** | the resource is absent — not wired in at all |
| **managed** | WSE provisions a chosen version *(future)* |

**Honesty about strength (this matters):** a mode's *guarantee* depends on the
runtime.
- On the **native** runtime, `deny` means *"not projected"* — it isn't on the PATH,
  it isn't in the workspace's `HOME`, the app won't find it by default. But a
  determined process still runs as you and could reach the real host path. Native
  gives **workspace-scoping**, not a security wall.
- On a **sandbox** runtime (Docker / VM), `deny` is a true wall — the resource
  genuinely isn't reachable.

So the same policy means "convenience separation" on native and "security
isolation" on Docker. The model is the same; the runtime sets the strength. That's
the point of keeping runtimes pluggable.

---

## 3. Which modes fit which class (and the default)

This table is the "80% is the same" insight — sensible **defaults per class**, so a
policy is mostly defaults plus a few overrides.

| Class | host | overlay | workspace | merge | temporary | deny | **default** |
|-------|:--:|:--:|:--:|:--:|:--:|:--:|:--:|
| Executable | ✓ | – | – | – | – | ✓ | **host** |
| Package | ✓ | ✓ | ✓ | – | – | ✓ | **workspace** |
| Config | ✓ | ✓ | ✓ | ✓ | – | ✓ | **merge** |
| Data | ✓ | ✓ | ✓ | – | ✓ | ✓ | **workspace** |
| Credential | ✓ | – | – | – | ✓ | ✓ | **deny** |
| Cache | – | – | ✓ | – | ✓ | – | **temporary** |

Read the defaults row aloud and you get a sane workspace for free: *your tools
work (executable: host), but it starts clean (packages/data: workspace), keeps your
harmless config (merge), never sees your secrets (deny), and doesn't hoard cache.*

---

## 4. Expressing the rules — two data layers

### 4a. App manifest — *what* resources an app has, *where* they live (app-as-data)

Engine-agnostic. Ships with WSE; user-extendable. The engine never hard-codes an
app; it reads these.

```yaml
# chrome
id: chrome
executable:
  discover:
    - "%ProgramFiles%/Google/Chrome/Application/chrome.exe"
    - "%LOCALAPPDATA%/Google/Chrome/Application/chrome.exe"
  launch: '"{exe}" --user-data-dir="{data.dir}" --new-window {args}'
resources:
  data:                                  # the profile dir; sub-layers below
    host: "%LOCALAPPDATA%/Google/Chrome/User Data"
    layers:
      bookmarks:  { class: data,       files: ["Default/Bookmarks"] }
      history:    { class: data,       files: ["Default/History"] }
      cookies:    { class: credential, files: ["Default/Network/Cookies"] }
      passwords:  { class: credential, files: ["Default/Login Data"] }
      extensions: { class: package,    dirs:  ["Default/Extensions"] }
```

```yaml
# python
id: python
executable: { discover: ["%LOCALAPPDATA%/Programs/Python/**/python.exe"] }
resources:
  packages: { class: package, env: "PYTHONPATH", host: "{exe_dir}/Lib/site-packages" }
  cache:    { class: cache,   env: "PYTHONPYCACHEPREFIX" }
```

### 4b. Projection policy — the *mode* per resource (per workspace / profile)

Default-by-class, with per-app / per-layer overrides. **Profiles** are named
policies:

```yaml
# profile: development
defaults:            # by resource class
  executable: host
  package:    overlay
  config:     merge
  data:       workspace
  credential: deny
  cache:      temporary
overrides:
  vscode.extensions: host        # I want my editor extensions
  chrome.bookmarks:  host        # ...and my bookmarks
```

Profiles to ship: **Clean** (executables only), **Development** (runtimes +
packages + editor extensions, no secrets), **Personal** (full browser profile),
**Custom**.

---

## 5. Applying the rules — the pipeline

```
 App Manifest (data)  +  Projection Policy (data)  +  Workspace
                              │
                              ▼
                    ┌──────────────────┐
                    │ ResourceProjector │   generic; knows no app
                    └──────────────────┘
                              │  produces
                              ▼
        Launch Plan { environment block · staged resource dirs · command }
                              │
                              ▼
             Runtime (native / docker / cloud) executes the plan
```

The **ResourceProjector** walks the manifest's resources; for each, it looks up the
policy mode and **stages** it:

| Mode | Staging action |
|------|----------------|
| host | point the app at the host path (env/arg) |
| workspace | make a fresh empty dir under the workspace home |
| overlay | copy host → workspace overlay (the **Overlay Engine**) |
| merge | assemble a dir: host's allowed pieces + workspace writes |
| temporary | a temp dir, registered for wipe-on-close |
| deny | leave the path unset / empty; drop from `PATH` |

It emits a **Launch Plan**: the computed **environment block** (`PATH`, `HOME`,
`PYTHONPATH`, … — this is the *Workspace Runtime Context*), the staged directories,
and the filled-in **launch command**. The runtime just executes it (native:
`CreateProcess` with that env on the workspace desktop; Docker: the container's
mounts + entrypoint).

Nothing in the projector says "Python" or "Chrome." Add an app = add a manifest.

---

## 6. The four engines are one pipeline

- **Overlay Engine** — the mechanism behind the `overlay` mode (copy + diff +
  merge/discard). *Already built + unit-tested.*
- **Projection Engine** — the ResourceProjector: manifest + policy → Launch Plan.
- **Runtime Context (Environment)** — the environment-block portion of the plan.
- **Runtime Engine** — executes the plan. *Native + Docker already built.*

They're not four subsystems; they're stages of one flow.

---

## 7. Fit with the WSE contract

- **Capability-based, engine owns policy.** Which modes a role/profile may choose
  is engine policy (like the Authorizer). The **runtime applies** it (builds +
  executes the plan). Exactly the split the contract already uses.
- **App support is data.** New app = a manifest. Zero per-app engine code — the
  mistake to avoid is `PythonModule` / `ChromeModule`; instead one
  `ResourceProjector` + rules.
- **Runtimes realize modes at their own strength** (§2). The workspace *means* the
  same thing; the guarantee differs. That's why native and Docker coexist.

---

## 8. This model is already half-shipped (as special cases)

It isn't new work — it's the pattern under things that exist:

| Existing thing | Is really… |
|----------------|-----------|
| Overlay Engine | the `overlay` mode for **Data** |
| VS Code launched with a fresh `--extensions-dir` | `package: workspace` |
| the Chrome profile importer | `data: host` (projected) |
| Clipboard modes (isolated / shared) | the clipboard resource: `workspace` / `host` |
| the workspace **home** (storage) | `data: workspace` |

v2 is the realization that these are the *same engine* with different config.

---

## 9. Open decisions (to resolve before building)

1. **host = rw or ro?** Read-write host mode risks the workspace mutating your real
   config; read-only is safer but some apps need to write. Per-resource choice.
2. **Native `deny` honesty.** It's "not wired in," not "cannot access." Do we label
   modes with a strength badge in the UI (🔒 on Docker, ~ on native)?
3. **Where manifests live** — bundled set + a user manifest dir; format (YAML/JSON).
4. **merge semantics for config** — last-writer, or structured merge per filetype?
5. **Discovery vs manifest** — auto-detect installed apps, then attach a manifest.

---

## 10. Build order (when real use calls for it)

Design first, per the discipline. The first *implementable* slice, when ready:

> The **ResourceProjector for one app (VS Code)** with **Clean** vs **Development**
> profiles — reusing the Overlay Engine and an environment block. Prove the
> pipeline (manifest + policy → plan → launch) end-to-end on one app, then the rest
> is more manifests, not more engine.

That validates the whole model with the least code, and every subsequent app is
data.
