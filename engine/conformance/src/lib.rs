//! wse-conformance — the reusable Workspace Contract conformance suite.
//!
//! The point (per the WSE plan): **no adapter gets its own tests.** The Windows
//! adapter, the Linux adapter, and the mock all run this *same* suite, through
//! the engine. Passing it is the definition of "conforming to the contract".
//!
//! Usage (in any adapter's tests):
//! ```ignore
//! wse_conformance::run_core(MyAdapter::new).assert_ok();
//! ```
//!
//! This is the *mandatory core* suite (SPEC §18.3-adjacent behaviour that every
//! adapter must exhibit). Capability-specific suites (clipboard, devices, …)
//! will be gated by what an adapter declares, and added as those capabilities
//! land — never before.

use wse_common::{AppSpec, Persistence, WorkspaceState, WseError};
use wse_contract::{WorkspaceAdapter, CONTRACT_VERSION};
use wse_engine::{Engine, WorkspaceConfig};

/// Result of a single named conformance check.
pub struct CheckResult {
    pub name: &'static str,
    pub outcome: std::result::Result<(), String>,
}

/// The report from running a suite against an adapter.
#[derive(Default)]
pub struct ConformanceReport {
    pub results: Vec<CheckResult>,
}

impl ConformanceReport {
    fn check(&mut self, name: &'static str, f: impl FnOnce() -> std::result::Result<(), String>) {
        self.results.push(CheckResult {
            name,
            outcome: f(),
        });
    }

    pub fn passed(&self) -> usize {
        self.results.iter().filter(|r| r.outcome.is_ok()).count()
    }

    pub fn total(&self) -> usize {
        self.results.len()
    }

    pub fn is_conformant(&self) -> bool {
        self.results.iter().all(|r| r.outcome.is_ok())
    }

    pub fn summary(&self) -> String {
        format!("{}/{} conformance checks passed", self.passed(), self.total())
    }

    /// Panic (as a test would) on the first failing check.
    pub fn assert_ok(&self) {
        for r in &self.results {
            if let Err(why) = &r.outcome {
                panic!("conformance check '{}' FAILED: {why}", r.name);
            }
        }
    }
}

// helpers ---------------------------------------------------------------------
fn ok(cond: bool, msg: impl Into<String>) -> std::result::Result<(), String> {
    if cond {
        Ok(())
    } else {
        Err(msg.into())
    }
}

fn catalog() -> Vec<AppSpec> {
    vec![
        AppSpec::new("browser", "Browser"),
        AppSpec::new("editor", "Editor"),
    ]
}

fn cfg() -> WorkspaceConfig {
    WorkspaceConfig::new("conformance", Persistence::Temporary, catalog())
}

/// Run the mandatory-core conformance suite against any adapter. `make` builds a
/// fresh adapter for each check, so checks are independent.
pub fn run_core<A, F>(make: F) -> ConformanceReport
where
    A: WorkspaceAdapter,
    F: Fn() -> A,
{
    let mut r = ConformanceReport::default();

    r.check("declares_compatible_contract_version", || {
        let a = make();
        ok(
            a.contract_version().compatible_with(CONTRACT_VERSION),
            format!(
                "adapter speaks {}, engine speaks {}",
                a.contract_version(),
                CONTRACT_VERSION
            ),
        )
    });

    r.check("create_yields_created_state", || {
        let mut e = Engine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        ok(
            e.state(&ws) == Some(WorkspaceState::Created),
            "expected Created after create_workspace",
        )
    });

    r.check("start_yields_running_state", || {
        let mut e = Engine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.start(&ws).map_err(|e| e.to_string())?;
        ok(
            e.state(&ws) == Some(WorkspaceState::Running),
            "expected Running after a start whose attestation the engine accepts",
        )
    });

    r.check("launch_when_running_opens_a_window", || {
        let mut e = Engine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.start(&ws).map_err(|e| e.to_string())?;
        e.launch(&ws, "browser").map_err(|e| e.to_string())?;
        let n = e.list_windows(&ws).map_err(|e| e.to_string())?.len();
        ok(n == 1, format!("expected 1 window, got {n}"))
    });

    r.check("multiple_launches_are_registered", || {
        let mut e = Engine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.start(&ws).map_err(|e| e.to_string())?;
        e.launch(&ws, "browser").map_err(|e| e.to_string())?;
        e.launch(&ws, "editor").map_err(|e| e.to_string())?;
        let n = e.list_windows(&ws).map_err(|e| e.to_string())?.len();
        ok(n == 2, format!("expected 2 windows, got {n}"))
    });

    r.check("cannot_launch_before_running", || {
        let mut e = Engine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        match e.launch(&ws, "browser") {
            Err(WseError::NotRunning(_)) => Ok(()),
            other => Err(format!("expected NotRunning, got {other:?}")),
        }
    });

    r.check("ungranted_app_is_not_found_not_denied", || {
        // SPEC §6.5 undetectability — the single most important behavioural check.
        let mut e = Engine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.start(&ws).map_err(|e| e.to_string())?;
        match e.launch(&ws, "photoshop") {
            Err(WseError::NotFound(_)) => Ok(()),
            other => Err(format!("expected NotFound, got {other:?}")),
        }
    });

    r.check("illegal_transition_is_rejected", || {
        // SPEC §5.2 — Created -> Saved (stop before start) is not permitted.
        let mut e = Engine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        match e.stop(&ws) {
            Err(WseError::InvalidTransition { .. }) => Ok(()),
            other => Err(format!("expected InvalidTransition, got {other:?}")),
        }
    });

    r.check("destroy_is_irrecoverable", || {
        // SPEC §5.5 — after destroy the workspace does not exist (not merely unlisted).
        let mut e = Engine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.start(&ws).map_err(|e| e.to_string())?;
        e.destroy(&ws).map_err(|e| e.to_string())?;
        match e.list_windows(&ws) {
            Err(WseError::NotFound(_)) => Ok(()),
            other => Err(format!("expected NotFound after destroy, got {other:?}")),
        }
    });

    r
}
