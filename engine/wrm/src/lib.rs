//! The Workspace Resource Model (WRM) — the generic engine.
//!
//! Applications are **data** (an `AppManifest`). A **`Policy`** says, per resource
//! class, how to project it. The **`project`** function turns (manifest + policy +
//! workspace home) into a **`LaunchPlan`** — the OS-agnostic IR a runtime executes.
//!
//! The whole claim of WSE v2 is validated here: `project` contains **no
//! application-specific code** (grep it — no "vscode", no "python"). Supporting a
//! new app is authoring a manifest, not extending the engine. See
//! docs/workspace-resource-model.md.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

// ── the resource taxonomy + projection modes ─────────────────────────────────
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ResourceClass {
    Executable,
    Package,
    Config,
    Data,
    Credential,
    Cache,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Mode {
    Host,
    Overlay,
    Workspace,
    Merge,
    Temporary,
    Deny,
}

// ── application manifest (an app expressed purely as data) ───────────────────
/// One resource an application consumes. `arg`/`env` are how the *use path* is
/// handed to the app: an arg template like `--extensions-dir={path}` and/or an
/// environment variable like `PYTHONPATH`.
pub struct Resource {
    pub name: &'static str,
    pub class: ResourceClass,
    pub host_path: Option<&'static str>, // where it lives on the host (%VARS% ok)
    pub arg: Option<&'static str>,
    pub env: Option<&'static str>,
}

pub struct AppManifest {
    pub id: &'static str,
    pub exe_candidates: &'static [&'static str],
    /// Base args; `{workspace}` is replaced with the workspace home.
    pub base_args: &'static [&'static str],
    pub resources: &'static [Resource],
}

// ── projection policy (mode per class, with per-"app.resource" overrides) ────
pub struct Policy {
    pub name: &'static str,
    pub defaults: &'static [(ResourceClass, Mode)],
    pub overrides: &'static [(&'static str, Mode)],
}

impl Policy {
    fn mode_for(&self, app: &str, r: &Resource) -> Mode {
        let key = format!("{app}.{}", r.name);
        if let Some((_, m)) = self.overrides.iter().find(|(k, _)| *k == key) {
            return *m;
        }
        self.defaults
            .iter()
            .find(|(c, _)| *c == r.class)
            .map(|(_, m)| *m)
            .unwrap_or(Mode::Deny)
    }
}

// ── the contract: LaunchPlan (IR) ────────────────────────────────────────────
pub struct StagedResource {
    pub name: String,
    pub class: ResourceClass,
    pub mode: Mode,
    pub workspace_path: Option<PathBuf>,
    pub host_path: Option<PathBuf>,
}

pub struct LaunchPlan {
    pub executable: PathBuf,
    pub arguments: Vec<String>,
    pub working_directory: PathBuf,
    pub environment: Vec<(String, String)>,
    pub resources: Vec<StagedResource>,
    /// Every projection mode this plan uses — what a runtime must satisfy.
    pub requirements: HashSet<Mode>,
}

impl LaunchPlan {
    /// Find the value passed to an argument like `--extensions-dir=` (helper).
    pub fn arg_value(&self, flag: &str) -> Option<&str> {
        self.arguments
            .iter()
            .find_map(|a| a.strip_prefix(flag))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum WrmError {
    NotInstalled(&'static str),
}

// ── projection strength: what a runtime HONESTLY enforces ────────────────────
/// How strongly a runtime enforces a projection. The policy is identical across
/// runtimes; only the strength differs. **Rule:** a runtime must never claim a
/// stronger guarantee than the OS can actually enforce with supported mechanisms.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Guarantee {
    /// Fully enforced by the runtime (e.g. env vars, a Job Object process tree).
    Strong,
    /// A real wall — own filesystem / network / registry (a sandbox).
    Isolated,
    /// Shared with the host; not isolated at all.
    Shared,
    /// Best-effort / incomplete.
    Partial,
    /// The runtime cannot provide this at all.
    Unsupported,
}

/// A runtime's honest guarantees, per resource aspect. Carried on the descriptor
/// AND copied into every `ExecutionContext`, so the UI can render exactly what a
/// running workspace enforces without ever re-asking the runtime.
#[derive(Clone, Copy, Debug)]
pub struct Guarantees {
    pub environment: Guarantee,
    pub working_directory: Guarantee,
    pub overlay: Guarantee,
    pub process_tree: Guarantee,
    pub clipboard: Guarantee,
    pub registry: Guarantee,
    pub network: Guarantee,
}

// ── runtime descriptor (capability negotiation, mirror of the plan) ──────────
pub struct RuntimeDescriptor {
    pub name: &'static str,
    pub supports: &'static [Mode],
    /// What this runtime honestly enforces (see the non-negotiable rule above).
    pub guarantees: Guarantees,
}

impl RuntimeDescriptor {
    /// Can this runtime realise every mode the plan needs? (No -> capability error.)
    pub fn can_satisfy(&self, plan: &LaunchPlan) -> bool {
        plan.requirements.iter().all(|m| self.supports.contains(m))
    }

    pub fn missing(&self, plan: &LaunchPlan) -> Vec<Mode> {
        plan.requirements
            .iter()
            .copied()
            .filter(|m| !self.supports.contains(m))
            .collect()
    }
}

// ── the projector: the ONE generic function; knows no application ────────────
/// Turn a manifest + policy + workspace home into a LaunchPlan. Deliberately
/// contains no app-specific logic — everything comes from the manifest data.
pub fn project(m: &AppManifest, p: &Policy, home: &Path) -> Result<LaunchPlan, WrmError> {
    let executable = resolve_exe(m.exe_candidates).ok_or(WrmError::NotInstalled(m.id))?;

    let mut arguments = Vec::new();
    let mut environment = Vec::new();
    let mut resources = Vec::new();

    for r in m.resources {
        let mode = p.mode_for(m.id, r);
        let host = r.host_path.map(resolve_path);
        let workspace = matches!(mode, Mode::Workspace | Mode::Temporary | Mode::Overlay)
            .then(|| home.join(m.id).join(r.name));

        // The path the app should actually use for this resource.
        let use_path = match mode {
            Mode::Host | Mode::Merge => host.clone(),
            Mode::Workspace | Mode::Temporary | Mode::Overlay => workspace.clone(),
            Mode::Deny => None,
        };
        if let Some(path) = &use_path {
            let s = path.to_string_lossy();
            if let Some(a) = r.arg {
                arguments.push(a.replace("{path}", &s));
            }
            if let Some(e) = r.env {
                environment.push((e.to_string(), s.into_owned()));
            }
        }

        resources.push(StagedResource {
            name: r.name.to_string(),
            class: r.class,
            mode,
            workspace_path: workspace,
            host_path: host,
        });
    }

    // Base args last (e.g. the folder to open).
    for a in m.base_args {
        arguments.push(a.replace("{workspace}", &home.to_string_lossy()));
    }

    let requirements = resources.iter().map(|s| s.mode).collect();
    Ok(LaunchPlan {
        executable,
        arguments,
        working_directory: home.to_path_buf(),
        environment,
        resources,
        requirements,
    })
}

fn resolve_path(p: &str) -> PathBuf {
    let mut out = p.to_string();
    for var in ["USERPROFILE", "LOCALAPPDATA", "APPDATA", "ProgramFiles"] {
        if let Ok(v) = std::env::var(var) {
            out = out.replace(&format!("%{var}%"), &v);
        }
    }
    PathBuf::from(out)
}

fn resolve_exe(candidates: &[&str]) -> Option<PathBuf> {
    let resolved: Vec<PathBuf> = candidates.iter().map(|c| resolve_path(c)).collect();
    // Prefer an installed one; fall back to the first candidate so a plan can be
    // formed (and validated) even where the app isn't installed.
    resolved
        .iter()
        .find(|p| p.exists())
        .cloned()
        .or_else(|| resolved.into_iter().next())
}

// ── the data: manifests, policies, runtimes (no engine code) ─────────────────
pub mod manifests {
    use super::*;

    pub static VSCODE: AppManifest = AppManifest {
        id: "vscode",
        exe_candidates: &[
            r"%LOCALAPPDATA%\Programs\Microsoft VS Code\Code.exe",
            r"C:\Program Files\Microsoft VS Code\Code.exe",
        ],
        base_args: &["--new-window", "{workspace}"],
        resources: &[
            Resource {
                name: "extensions",
                class: ResourceClass::Package,
                host_path: Some(r"%USERPROFILE%\.vscode\extensions"),
                arg: Some("--extensions-dir={path}"),
                env: None,
            },
            Resource {
                name: "userdata",
                class: ResourceClass::Data,
                host_path: Some(r"%APPDATA%\Code"),
                arg: Some("--user-data-dir={path}"),
                env: None,
            },
        ],
    };

    pub static PYTHON: AppManifest = AppManifest {
        id: "python",
        exe_candidates: &[r"%LOCALAPPDATA%\Programs\Python\Python313\python.exe"],
        base_args: &[],
        resources: &[
            Resource {
                name: "packages",
                class: ResourceClass::Package,
                host_path: Some(r"%LOCALAPPDATA%\Programs\Python\Python313\Lib\site-packages"),
                arg: None,
                env: Some("PYTHONPATH"),
            },
            Resource {
                name: "cache",
                class: ResourceClass::Cache,
                host_path: None,
                arg: None,
                env: Some("PYTHONPYCACHEPREFIX"),
            },
        ],
    };
}

pub mod policies {
    use super::*;

    /// Executables only; everything else fresh/denied. A fresh machine.
    pub static CLEAN: Policy = Policy {
        name: "clean",
        defaults: &[
            (ResourceClass::Executable, Mode::Host),
            (ResourceClass::Package, Mode::Workspace),
            (ResourceClass::Config, Mode::Workspace),
            (ResourceClass::Data, Mode::Workspace),
            (ResourceClass::Credential, Mode::Deny),
            (ResourceClass::Cache, Mode::Temporary),
        ],
        overrides: &[],
    };

    /// Runtimes + your packages/extensions; still no credentials.
    pub static DEVELOPMENT: Policy = Policy {
        name: "development",
        defaults: &[
            (ResourceClass::Executable, Mode::Host),
            (ResourceClass::Package, Mode::Host),
            (ResourceClass::Config, Mode::Merge),
            (ResourceClass::Data, Mode::Workspace),
            (ResourceClass::Credential, Mode::Deny),
            (ResourceClass::Cache, Mode::Temporary),
        ],
        overrides: &[],
    };
}

pub mod runtimes {
    use super::*;

    /// Native Windows: everything but a real `deny` wall. Honest guarantees —
    /// registry can't be virtualised in the app layer, network is the host's.
    pub static NATIVE_WINDOWS: RuntimeDescriptor = RuntimeDescriptor {
        name: "native-windows",
        supports: &[
            Mode::Host,
            Mode::Workspace,
            Mode::Overlay,
            Mode::Merge,
            Mode::Temporary,
        ],
        guarantees: Guarantees {
            environment: Guarantee::Strong,
            working_directory: Guarantee::Strong,
            overlay: Guarantee::Strong,
            process_tree: Guarantee::Strong, // Job Object
            clipboard: Guarantee::Strong,
            registry: Guarantee::Unsupported, // app layer can't virtualise HKCU
            network: Guarantee::Shared,       // shares the host network stack
        },
    };

    /// Docker: a real sandbox — including a true `deny`, private registry + network.
    pub static DOCKER: RuntimeDescriptor = RuntimeDescriptor {
        name: "docker",
        supports: &[
            Mode::Workspace,
            Mode::Overlay,
            Mode::Temporary,
            Mode::Deny,
            Mode::Host,
        ],
        guarantees: Guarantees {
            environment: Guarantee::Strong,
            working_directory: Guarantee::Strong,
            overlay: Guarantee::Strong,
            process_tree: Guarantee::Strong,
            clipboard: Guarantee::Strong,
            registry: Guarantee::Isolated, // own OS, own registry
            network: Guarantee::Isolated,  // own IP / network namespace
        },
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_manifest_different_policy_yields_different_plans() {
        let home = Path::new(r"C:\ws\home");

        let clean = project(&manifests::VSCODE, &policies::CLEAN, home).unwrap();
        let dev = project(&manifests::VSCODE, &policies::DEVELOPMENT, home).unwrap();

        // Same app, same executable — only the projection changed.
        assert_eq!(clean.executable, dev.executable);

        // Extensions (a Package): CLEAN -> a fresh workspace dir; DEVELOPMENT ->
        // the host's real extensions. The projector did this from the policy, not
        // from any VS Code knowledge.
        let clean_ext = clean.arg_value("--extensions-dir=").unwrap();
        let dev_ext = dev.arg_value("--extensions-dir=").unwrap();
        assert!(clean_ext.contains(r"ws\home"), "clean extensions are fresh: {clean_ext}");
        assert!(dev_ext.contains(".vscode"), "dev extensions are the host's: {dev_ext}");
        assert_ne!(clean_ext, dev_ext);

        // The folder-to-open comes last, and is the workspace home.
        assert_eq!(clean.arguments.last().unwrap(), r"C:\ws\home");
    }

    #[test]
    fn projector_is_generic_across_apps() {
        // A totally different app (env-based, not arg-based) goes through the same
        // `project` with zero app-specific code.
        let home = Path::new(r"C:\ws\home");
        let clean = project(&manifests::PYTHON, &policies::CLEAN, home).unwrap();
        let dev = project(&manifests::PYTHON, &policies::DEVELOPMENT, home).unwrap();

        let pp = |plan: &LaunchPlan| {
            plan.environment.iter().find(|(k, _)| k == "PYTHONPATH").map(|(_, v)| v.clone())
        };
        // CLEAN -> PYTHONPATH points at a fresh (empty) workspace dir; DEVELOPMENT
        // -> the host's site-packages.
        assert!(pp(&clean).unwrap().contains(r"ws\home"));
        assert!(pp(&dev).unwrap().contains("site-packages"));
    }

    #[test]
    fn runtime_negotiates_the_plan() {
        let home = Path::new(r"C:\ws\home");
        let plan = project(&manifests::VSCODE, &policies::DEVELOPMENT, home).unwrap();
        // Native + Docker can both satisfy this plan (no `deny` needed here).
        assert!(runtimes::NATIVE_WINDOWS.can_satisfy(&plan));
        assert!(runtimes::DOCKER.can_satisfy(&plan));

        // A plan that REQUIRES a real wall (deny) is refused by native, accepted by
        // Docker — a capability error, never a crash.
        let mut walled = plan;
        walled.requirements.insert(Mode::Deny);
        assert!(!runtimes::NATIVE_WINDOWS.can_satisfy(&walled));
        assert_eq!(runtimes::NATIVE_WINDOWS.missing(&walled), vec![Mode::Deny]);
        assert!(runtimes::DOCKER.can_satisfy(&walled));
    }
}
