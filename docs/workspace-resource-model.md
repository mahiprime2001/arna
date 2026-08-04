# The Workspace Resource Model (WRM)

**The core model of WSE.** It defines the resource taxonomy, projection semantics,
launch planning, runtime negotiation, and workspace behaviour. It subsumes
[Host Resource Projection](host-resource-projection.md) (the motivation) and the
Overlay Engine (already built) into one engine.

**The keystone is the contract between the two halves** (see §5b/§5c): the
**LaunchPlan** — an OS-agnostic intermediate representation the projector emits and
the runtime executes — and the **Runtime Descriptor** — what a runtime declares it
can satisfy. The projector never learns an OS; the runtime never learns an app;
the LaunchPlan is their only shared language.

```
Workspace → Resource Projector → LaunchPlan (IR) → Runtime → Operating System
              (WHAT: policy)      the contract     (HOW)
```

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

## 5b. The LaunchPlan — the contract (IR)

The single most important object in WSE. The projector's only output; the runtime's
only input. OS-agnostic and app-agnostic — it describes *what to run and with which
projected resources*, never *how* to realise them.

```rust
struct LaunchPlan {
    executable: PathBuf,             // resolved from the manifest's candidates
    arguments: Vec<String>,          // filled from staged resources
    working_directory: PathBuf,
    environment: Vec<(String, String)>,   // the Workspace Runtime Context
    resources: Vec<StagedResource>,  // each resource + its chosen mode + paths
    requirements: RuntimeRequirements,     // the modes/features this plan needs
}

struct StagedResource {
    name: String,
    class: ResourceClass,
    mode: ProjectionMode,            // host | workspace | overlay | merge | temporary | deny
    workspace_path: Option<PathBuf>, // where the workspace copy lives (if any)
    host_path: Option<PathBuf>,      // the origin (for host/overlay/merge)
}
```

The plan is **declarative about resources**: it says "extensions: mode=workspace at
`<home>/vscode/ext`" — it does *not* copy anything. Realising a `StagedResource`
(create the dir, run the overlay copy, mount a volume) is the **runtime's** job,
done its own way (native: dirs + env; Docker: mounts). This keeps the projector
free of all fs/OS work.

## 5c. The Runtime Descriptor — capability negotiation

The mirror of the plan: what a runtime can do. The projector/policy negotiate
against it *before* execution, exactly like capability negotiation in the core
contract.

```yaml
# native-windows
runtime: native-windows
supports: [host, workspace, overlay, merge, temporary]   # deny is WEAK here
applications: native            # real Windows .exe
limitations:
  process_namespace: false
  kernel_isolation: false
  registry_projection: false    # can't project the registry (yet)
  deny_is_a_wall: false         # deny = "not wired in", not "cannot access"
```
```yaml
# docker
runtime: docker
supports: [workspace, overlay, deny, temporary, host]
applications: linux             # not native Windows apps
limitations:
  deny_is_a_wall: true          # true isolation
  native_windows_apps: false
```

**The flow becomes:**
```
Projection Policy → LaunchPlan → Runtime.can_satisfy(plan)? → execute | capability error
```
A runtime that can't satisfy a plan's requirements returns a capability error
(never a crash) — and the UI can suggest a runtime that can (e.g. "this needs a
real `deny` wall — use Docker"). Same `WseError` vocabulary as the core contract.

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

## 10. Validation — the pipeline is proven (`engine/wrm`)

The core claim is validated in code (`wse-wrm`, pure logic, no platform deps):

- **`project(manifest, policy, home) -> LaunchPlan`** — one generic function; grep
  it, there is no `"vscode"` or `"python"` in it.
- VS Code under **Clean** vs **Development** yields different plans from the policy
  alone (extensions: a fresh workspace dir vs the host's real extensions), same
  executable — *no app-specific code*.
- A **totally different app** (Python, env-based via `PYTHONPATH`) goes through the
  *same* `project` with zero changes — proving app support is data, not engine.
- **Runtime negotiation**: native + Docker both satisfy a normal plan; a plan that
  requires a real `deny` wall is *refused by native* (`missing() == [Deny]`, a
  capability result, not a crash) and *accepted by Docker*.

So application support is data-driven and runtimes execute/negotiate plans rather
than embedding application knowledge — the central claim of WSE v2. **Every
subsequent app is a manifest, not more engine.**

**Next (wiring, not architecture):** feed a `LaunchPlan` to the real native runtime
(`create_process_flags` already takes an env + command) so a workspace launches VS
Code *from the plan*, and stage `overlay`/`workspace` dirs via the Overlay Engine.
Then a profile picker in the create dialog. None of that changes the model.
