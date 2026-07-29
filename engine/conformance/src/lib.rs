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

use std::collections::HashMap;

use wse_common::{
    AccessRight, AppSpec, Capability, ClipboardItem, Persistence, ResourceKind, Role,
    WorkspaceState, WseError,
};
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

    fn absorb(&mut self, other: ConformanceReport) {
        self.results.extend(other.results);
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
            Err(WseError::InvalidState { .. }) => Ok(()),
            other => Err(format!("expected InvalidState, got {other:?}")),
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

    r.check("identity_reflects_declared_capabilities", || {
        let mut e = Engine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        let adapter_caps = make().capabilities();
        let id = e.identity(&ws).ok_or("no identity")?;
        ok(
            id.capabilities == adapter_caps,
            "workspace identity must reflect the adapter's declared capabilities",
        )
    });

    r
}

// ── capability-gated suites ─────────────────────────────────────────────────
// Each capability has its own suite. An adapter only runs the suites for the
// capabilities it declares. `run_all` wires it together: no adapter is tested
// for something it never claimed to provide.

/// SPEC §10 — the Applications capability. Deeper than core: multiple instances
/// of an app are permitted (§10.3).
pub fn run_applications<A, F>(make: F) -> ConformanceReport
where
    A: WorkspaceAdapter,
    F: Fn() -> A,
{
    let mut r = ConformanceReport::default();

    r.check("applications/multiple_instances_permitted", || {
        // SPEC §10.3 — launching the same app twice yields two windows.
        let mut e = Engine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.start(&ws).map_err(|e| e.to_string())?;
        e.launch(&ws, "browser").map_err(|e| e.to_string())?;
        e.launch(&ws, "browser").map_err(|e| e.to_string())?;
        let n = e.list_windows(&ws).map_err(|e| e.to_string())?.len();
        ok(n == 2, format!("expected 2 instances, got {n}"))
    });

    r
}

/// SPEC §14 — the Windows capability. Focus semantics: at most one window is
/// focused, and it is the most recently launched.
pub fn run_windows<A, F>(make: F) -> ConformanceReport
where
    A: WorkspaceAdapter,
    F: Fn() -> A,
{
    let mut r = ConformanceReport::default();

    r.check("windows/at_most_one_focused", || {
        let mut e = Engine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.start(&ws).map_err(|e| e.to_string())?;
        e.launch(&ws, "browser").map_err(|e| e.to_string())?;
        e.launch(&ws, "editor").map_err(|e| e.to_string())?;
        let windows = e.list_windows(&ws).map_err(|e| e.to_string())?;
        let focused = windows.iter().filter(|w| w.focused).count();
        ok(focused == 1, format!("expected exactly 1 focused, got {focused}"))
    });

    r.check("windows/newest_is_focused", || {
        let mut e = Engine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.start(&ws).map_err(|e| e.to_string())?;
        e.launch(&ws, "browser").map_err(|e| e.to_string())?;
        e.launch(&ws, "editor").map_err(|e| e.to_string())?;
        let windows = e.list_windows(&ws).map_err(|e| e.to_string())?;
        ok(
            windows.last().map(|w| w.focused).unwrap_or(false),
            "the most recently launched window must be focused",
        )
    });

    r
}

/// SPEC §9 — the Clipboard capability. See contract/capabilities/clipboard.md.
/// Runs only for adapters that declare Capability::Clipboard.
pub fn run_clipboard<A, F>(make: F) -> ConformanceReport
where
    A: WorkspaceAdapter,
    F: Fn() -> A,
{
    let mut r = ConformanceReport::default();

    // A config where the Collaborator may write into the workspace but not read
    // out of it — to prove the two directions are independent (I2).
    let write_only_collaborator = || {
        let mut c = cfg();
        let mut rights = HashMap::new();
        rights.insert(AccessRight::ClipboardWrite, true);
        rights.insert(AccessRight::ClipboardRead, false);
        c.collaborator_rights = rights;
        c
    };

    r.check("clipboard/isolated_per_workspace", || {
        // I1 — one workspace's clipboard is invisible to another.
        let mut e = Engine::new(make());
        let a = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        let b = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.clipboard_write_in(&a, Role::Owner, ClipboardItem::text("secret"))
            .map_err(|e| e.to_string())?;
        let seen = e
            .clipboard_read_out(&b, Role::Owner)
            .map_err(|e| e.to_string())?;
        ok(seen.is_none(), "workspace B must not see workspace A's clipboard")
    });

    r.check("clipboard/owner_roundtrips", || {
        // I4 — Owner may write then read the same payload back.
        let mut e = Engine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.clipboard_write_in(&ws, Role::Owner, ClipboardItem::text("hello"))
            .map_err(|e| e.to_string())?;
        let got = e
            .clipboard_read_out(&ws, Role::Owner)
            .map_err(|e| e.to_string())?;
        ok(
            got == Some(ClipboardItem::text("hello")),
            "Owner read_out must return what was written_in",
        )
    });

    r.check("clipboard/read_and_write_are_separate_rights", || {
        // I2 — a Collaborator with write-but-not-read may write, is refused read.
        let mut e = Engine::new(make());
        let ws = e
            .create_workspace(write_only_collaborator())
            .map_err(|e| e.to_string())?;
        e.clipboard_write_in(&ws, Role::Collaborator, ClipboardItem::text("x"))
            .map_err(|e| format!("write_in should be allowed: {e}"))?;
        match e.clipboard_read_out(&ws, Role::Collaborator) {
            Err(WseError::PermissionDenied { .. }) => Ok(()),
            other => Err(format!("read_out should be denied, got {other:?}")),
        }
    });

    r.check("clipboard/observer_refused_read_out", || {
        // I3 — observing is not extracting.
        let mut e = Engine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        match e.clipboard_read_out(&ws, Role::Observer) {
            Err(WseError::PermissionDenied { .. }) => Ok(()),
            other => Err(format!("Observer read_out must be denied, got {other:?}")),
        }
    });

    r.check("clipboard/observer_refused_write_in", || {
        let mut e = Engine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        match e.clipboard_write_in(&ws, Role::Observer, ClipboardItem::text("x")) {
            Err(WseError::PermissionDenied { .. }) => Ok(()),
            other => Err(format!("Observer write_in must be denied, got {other:?}")),
        }
    });

    r
}

/// SPEC §8 — the Storage capability (workspace persistence). See
/// contract/capabilities/storage.md. Runs only for adapters declaring Storage.
pub fn run_storage<A, F>(make: F) -> ConformanceReport
where
    A: WorkspaceAdapter,
    F: Fn() -> A,
{
    let mut r = ConformanceReport::default();

    let write_only_collaborator = || {
        let mut c = cfg();
        let mut rights = HashMap::new();
        rights.insert(AccessRight::FileTransfer, false);
        c.collaborator_rights = rights;
        c
    };

    r.check("storage/owner_roundtrips", || {
        let mut e = Engine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        let meta = e
            .storage_create(&ws, Role::Owner, "notes", ResourceKind::Blob)
            .map_err(|e| e.to_string())?;
        e.storage_write(&ws, Role::Owner, &meta.id, b"hello".to_vec())
            .map_err(|e| e.to_string())?;
        let got = e
            .storage_read(&ws, Role::Owner, &meta.id)
            .map_err(|e| e.to_string())?;
        ok(got == b"hello".to_vec(), "read must return what was written")
    });

    r.check("storage/resource_id_is_stable", || {
        let mut e = Engine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        let meta = e
            .storage_create(&ws, Role::Owner, "a", ResourceKind::Blob)
            .map_err(|e| e.to_string())?;
        let listed = e.storage_list(&ws).map_err(|e| e.to_string())?;
        ok(
            listed.iter().any(|m| m.id == meta.id),
            "the created resource id must appear in the list",
        )
    });

    r.check("storage/isolated_per_workspace", || {
        // I2 — a resource in one workspace is not readable from another.
        let mut e = Engine::new(make());
        let a = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        let b = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        let meta = e
            .storage_create(&a, Role::Owner, "secret", ResourceKind::Blob)
            .map_err(|e| e.to_string())?;
        match e.storage_read(&b, Role::Owner, &meta.id) {
            Err(WseError::NotFound(_)) => Ok(()),
            other => Err(format!("B must not read A's resource, got {other:?}")),
        }
    });

    r.check("storage/deletion_is_terminal", || {
        // I3 — after delete, the id never resolves again.
        let mut e = Engine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        let meta = e
            .storage_create(&ws, Role::Owner, "tmp", ResourceKind::Blob)
            .map_err(|e| e.to_string())?;
        let existed = e
            .storage_delete(&ws, Role::Owner, &meta.id)
            .map_err(|e| e.to_string())?;
        if !existed {
            return Err("delete of an existing resource must return true".into());
        }
        match e.storage_read(&ws, Role::Owner, &meta.id) {
            Err(WseError::NotFound(_)) => {}
            other => return Err(format!("read after delete must be NotFound, got {other:?}")),
        }
        let again = e
            .storage_delete(&ws, Role::Owner, &meta.id)
            .map_err(|e| e.to_string())?;
        ok(!again, "deleting a missing resource must return false")
    });

    r.check("storage/list_reflects_resources", || {
        let mut e = Engine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.storage_create(&ws, Role::Owner, "a", ResourceKind::Blob)
            .map_err(|e| e.to_string())?;
        e.storage_create(&ws, Role::Owner, "b", ResourceKind::Blob)
            .map_err(|e| e.to_string())?;
        let n = e.storage_list(&ws).map_err(|e| e.to_string())?.len();
        ok(n == 2, format!("expected 2 resources, got {n}"))
    });

    r.check("storage/observer_refused_transfer", || {
        // I6 — extraction is not observation.
        let mut e = Engine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        match e.storage_create(&ws, Role::Observer, "x", ResourceKind::Blob) {
            Err(WseError::PermissionDenied { .. }) => Ok(()),
            other => Err(format!("Observer create must be denied, got {other:?}")),
        }
    });

    r.check("storage/collaborator_needs_filetransfer_right", || {
        // I6 — a Collaborator without FileTransfer is refused.
        let mut e = Engine::new(make());
        let ws = e
            .create_workspace(write_only_collaborator())
            .map_err(|e| e.to_string())?;
        match e.storage_create(&ws, Role::Collaborator, "x", ResourceKind::Blob) {
            Err(WseError::PermissionDenied { .. }) => Ok(()),
            other => Err(format!("Collaborator w/o FileTransfer must be denied, got {other:?}")),
        }
    });

    r.check("storage/persists_across_suspend", || {
        // I5 (partial) — data written while Running survives a stop -> Saved.
        let mut e = Engine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.start(&ws).map_err(|e| e.to_string())?;
        let meta = e
            .storage_create(&ws, Role::Owner, "keep", ResourceKind::Blob)
            .map_err(|e| e.to_string())?;
        e.storage_write(&ws, Role::Owner, &meta.id, b"persist".to_vec())
            .map_err(|e| e.to_string())?;
        e.stop(&ws).map_err(|e| e.to_string())?; // -> Saved
        let got = e
            .storage_read(&ws, Role::Owner, &meta.id)
            .map_err(|e| format!("resource must survive suspend: {e}"))?;
        ok(got == b"persist".to_vec(), "resource bytes must survive suspend")
    });

    r
}

/// Run the full conformance suite an adapter is *eligible* for: the mandatory
/// core, plus one suite per declared capability. An adapter is never tested for
/// a capability it didn't declare (SPEC §18.2). This is capability negotiation
/// applied to conformance itself.
pub fn run_all<A, F>(make: F) -> ConformanceReport
where
    A: WorkspaceAdapter,
    F: Fn() -> A,
{
    let caps = make().capabilities();
    let mut report = run_core(&make);
    if caps.supports(Capability::Applications) {
        report.absorb(run_applications(&make));
    }
    if caps.supports(Capability::Windows) {
        report.absorb(run_windows(&make));
    }
    if caps.supports(Capability::Clipboard) {
        report.absorb(run_clipboard(&make));
    }
    if caps.supports(Capability::Storage) {
        report.absorb(run_storage(&make));
    }
    // Devices, Network, Audio, Camera suites slot in here as each capability's
    // mini-spec is written — never before.
    report
}
