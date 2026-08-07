# WRM manifest samples — "Hello World" for the platform

An application in WSE is **data**, not code. Supporting a new app means writing a
manifest like the ones here — never touching the engine. These five are the
reference examples every future manifest copies from.

> **Status (honest):** the engine's built-in manifests currently live as Rust data
> in [`engine/wrm/src/lib.rs`](../engine/wrm/src/lib.rs) (`mod manifests`). These
> YAML files are the **authoring format** — the shape a manifest takes and the
> canonical examples. A YAML loader that reads them at runtime is *product* work
> (a manifest problem, not an engine one); these mirror the built-ins exactly so
> the loader, when it lands, has a spec and a test corpus.

## The schema

```yaml
id: <string>                 # stable app id (used in policy overrides: "<id>.<resource>")
exe_candidates:              # first existing wins; %VARS% expanded
  - <path>
base_args:                   # appended last; "{workspace}" -> the workspace home
  - <arg>
resources:                   # what the app consumes — the projector decides HOW
  - name: <string>           # resource name (workspace path = home/<id>/<name>)
    class: Executable | Package | Config | Data | Credential | Cache
    host_path: <path|null>   # where it lives on the host (null = no host source)
    arg: "<flag>{path}"      # optional: hand the use-path to the app as an argument
    env: <VAR|null>          # optional: hand the use-path to the app as an env var
```

## How a manifest becomes a running app

```
manifest (this file)  +  profile (a Policy: mode per resource class)
        │
        ▼
   project()  ──►  LaunchPlan  ──►  Runtime.execute()  ──►  ExecutionContext
```

The **manifest** never says *where* a resource should live — only that it exists
and how the app is told about it. The **profile** decides the projection mode
(host / overlay / workspace / merge / temporary / deny) per resource class. Same
manifest + different profile = different isolation, zero app-specific code. See
[docs/workspace-resource-model.md](../docs/workspace-resource-model.md).

## The samples

| File | Shows off |
|------|-----------|
| [vscode.yaml](vscode.yaml) | arg-based projection (`--extensions-dir`, `--user-data-dir`); the canonical stress test |
| [chrome.yaml](chrome.yaml) | a browser profile as a `Data` resource; a `Cache` resource |
| [python.yaml](python.yaml) | env-based projection (`PYTHONPATH`) — no args at all |
| [git.yaml](git.yaml) | `Config` via env (`GIT_CONFIG_GLOBAL`) + a `Credential` resource (denied by default) |
| [node.yaml](node.yaml) | global packages via `NODE_PATH`; an npm cache |

## Try one against the projector

The `project()` function is proven generic (no app names) in
[`engine/wrm`](../engine/wrm/src/lib.rs) — its tests project VS Code and Python
from the built-in equivalents of these files and assert the plans differ only by
profile. That's the whole claim: **application support is authoring, not
engineering.**
