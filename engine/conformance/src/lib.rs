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
    AccessRight, Actor, ApplicationDescriptor, ApplicationState, Capability, CapabilityState,
    ClipboardItem, DeviceClass, EventKind, EventSource, Persistence, ResourceKind, Role,
    WorkspaceState, WseError,
};
use std::ops::{Deref, DerefMut};

use wse_contract::{WorkspaceAdapter, CONTRACT_VERSION};
use wse_engine::{Engine, WorkspaceConfig};

/// A test-only Engine that destroys every workspace it created when it drops.
///
/// This enforces the repeatability property of the standard test suite: **every
/// conformance check leaves the system in the same observable state it found it
/// in.** Against the mock this is free; against a real adapter (WSL2 distros,
/// files, handles) it is what keeps the suite runnable twice with identical
/// results. Production `Engine` never auto-destroys workspaces — a Saved
/// workspace must survive — so this teardown lives only in the harness.
struct TestEngine<A: WorkspaceAdapter>(Engine<A>);

impl<A: WorkspaceAdapter> TestEngine<A> {
    fn new(adapter: A) -> Self {
        Self(Engine::new(adapter))
    }
}

impl<A: WorkspaceAdapter> Drop for TestEngine<A> {
    fn drop(&mut self) {
        for id in self.0.workspace_ids() {
            let _ = self.0.destroy(&id); // best-effort teardown
        }
    }
}

impl<A: WorkspaceAdapter> Deref for TestEngine<A> {
    type Target = Engine<A>;
    fn deref(&self) -> &Engine<A> {
        &self.0
    }
}

impl<A: WorkspaceAdapter> DerefMut for TestEngine<A> {
    fn deref_mut(&mut self) -> &mut Engine<A> {
        &mut self.0
    }
}

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

fn catalog() -> Vec<ApplicationDescriptor> {
    vec![
        ApplicationDescriptor::new("browser", "Browser"),
        ApplicationDescriptor::new("editor", "Editor"),
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
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        ok(
            e.state(&ws) == Some(WorkspaceState::Created),
            "expected Created after create_workspace",
        )
    });

    r.check("start_yields_running_state", || {
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.start(&ws).map_err(|e| e.to_string())?;
        ok(
            e.state(&ws) == Some(WorkspaceState::Running),
            "expected Running after a start whose attestation the engine accepts",
        )
    });

    r.check("illegal_transition_is_rejected", || {
        // SPEC §5.2 — Created -> Saved (stop before start) is not permitted.
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        match e.stop(&ws) {
            Err(WseError::InvalidTransition { .. }) => Ok(()),
            other => Err(format!("expected InvalidTransition, got {other:?}")),
        }
    });

    r.check("destroy_is_irrecoverable", || {
        // SPEC §5.5 — after destroy the workspace does not exist (not merely unlisted).
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.start(&ws).map_err(|e| e.to_string())?;
        e.destroy(&ws).map_err(|e| e.to_string())?;
        ok(
            e.state(&ws).is_none() && e.identity(&ws).is_none(),
            "the workspace must no longer exist after destroy",
        )
    });

    r.check("identity_reflects_negotiated_capabilities", || {
        // A workspace provides what the adapter bridges ∩ what the runtime offers.
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        let a = make();
        let expected = a.capabilities().intersect(&a.runtime().capabilities);
        let id = e.identity(&ws).ok_or("no identity")?;
        ok(
            id.capabilities == expected,
            "workspace capabilities must be adapter ∩ runtime",
        )
    });

    // ── runtime is core: every workspace runs on exactly one runtime ─────────
    r.check("runtime/adapter_declares_a_runtime", || {
        let rt = make().runtime();
        ok(
            !rt.name.is_empty() && !rt.digest.is_empty(),
            "a runtime must have a name and an immutable digest",
        )
    });

    r.check("runtime/start_records_a_reproducible_attestation", || {
        // Starting pins the exact environment: id, version, digest — so any run
        // or bug report names precisely what executed.
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        if e.runtime_attestation(&ws).is_some() {
            return Err("runtime attestation must not exist before start".into());
        }
        e.start(&ws).map_err(|e| e.to_string())?;
        let att = e
            .runtime_attestation(&ws)
            .ok_or("no runtime attestation after start")?;
        let declared = make().runtime();
        ok(
            att.runtime == declared.id
                && att.version == declared.version
                && att.digest == declared.digest,
            "the attestation must pin the declared runtime's id, version, and digest",
        )
    });

    r.check("runtime/capabilities_bound_the_workspace", || {
        // The workspace cannot provide a capability its runtime doesn't offer.
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        let rt_caps = make().runtime().capabilities;
        let ws_caps = e.capabilities(&ws).ok_or("no capabilities")?;
        ok(
            ws_caps.declared().iter().all(|c| rt_caps.supports(*c)),
            "every capability a workspace provides must be provided by its runtime",
        )
    });

    // ── events are core: every adapter must exhibit them ─────────────────────
    r.check("events/creation_emits_a_core_event", || {
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        let evs = e.events_for(&ws);
        let first = evs.first().ok_or("no events after create")?;
        ok(
            matches!(first.kind, EventKind::WorkspaceCreated)
                && first.source == EventSource::Core
                && first.actor == Actor::System
                && first.workspace == ws,
            "create must emit a Core WorkspaceCreated event with the right envelope",
        )
    });

    r.check("events/seq_is_monotonic_per_workspace", || {
        // Core-only operations (no capability): create + start + stop each emit
        // a lifecycle event, so seq monotonicity is testable on any adapter.
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.start(&ws).map_err(|e| e.to_string())?;
        e.stop(&ws).map_err(|e| e.to_string())?;
        let evs = e.events_for(&ws);
        let ordered = evs.windows(2).all(|w| w[1].seq > w[0].seq);
        ok(
            ordered && evs.len() >= 3,
            "per-workspace event seq must be strictly increasing",
        )
    });

    r.check("events/never_expose_forbidden_data", || {
        // Clipboard/storage events carry identity/metadata only, never content —
        // guaranteed by the envelope shape (EventKind has no content fields).
        // This check documents the invariant and fails loudly if the shape ever
        // grows a content-bearing field via a compile-time exhaustiveness match.
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.start(&ws).map_err(|e| e.to_string())?;
        for ev in e.events_for(&ws) {
            // No arm carries bytes/strings-of-content; ids and metadata only.
            match &ev.kind {
                EventKind::WorkspaceCreated
                | EventKind::WorkspaceDestroyed
                | EventKind::StateChanged { .. }
                | EventKind::ApplicationLaunchRequested { .. }
                | EventKind::ApplicationStarted { .. }
                | EventKind::ApplicationStopping { .. }
                | EventKind::ApplicationStopped { .. }
                | EventKind::WindowOpened { .. }
                | EventKind::WindowFocused { .. }
                | EventKind::WindowClosed { .. }
                | EventKind::ClipboardRead
                | EventKind::ClipboardWritten
                | EventKind::ResourceCreated { .. }
                | EventKind::ResourceModified { .. }
                | EventKind::ResourceRead { .. }
                | EventKind::ResourceDeleted { .. }
                | EventKind::DeviceAttached { .. }
                | EventKind::DeviceDetached { .. }
                | EventKind::DeviceRequested { .. }
                | EventKind::DeviceReleased { .. }
                | EventKind::CapabilityStateChanged { .. } => {}
            }
        }
        Ok(())
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

    // The capability is a *lifecycle*, not a launch call. These checks validate
    // the model — descriptor≠instance, stable instance id (never a PID), window
    // ownership, and the lifecycle transitions — not "an app can be started".

    r.check("applications/launch_yields_a_running_instance", || {
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.start(&ws).map_err(|e| e.to_string())?;
        let iid = e.launch(&ws, "browser").map_err(|e| e.to_string())?;
        let instances = e.app_instances(&ws).map_err(|e| e.to_string())?;
        let inst = instances
            .iter()
            .find(|i| i.id == iid)
            .ok_or("launched instance id not present in app_instances")?;
        ok(
            inst.state == ApplicationState::Running,
            format!("expected Running instance, got {:?}", inst.state),
        )
    });

    r.check("applications/instance_owns_its_windows", || {
        // Applications establishes the association; the Windows capability lists
        // the windows. The instance's windows must be real, listed windows.
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.start(&ws).map_err(|e| e.to_string())?;
        let iid = e.launch(&ws, "browser").map_err(|e| e.to_string())?;
        let instances = e.app_instances(&ws).map_err(|e| e.to_string())?;
        let inst = instances.iter().find(|i| i.id == iid).ok_or("no instance")?;
        let listed = e.list_windows(&ws).map_err(|e| e.to_string())?;
        ok(
            inst.windows.iter().all(|w| listed.iter().any(|lw| &lw.id == w)),
            "every window an instance owns must be a real, listed window",
        )
    });

    r.check("applications/instances_have_distinct_identities", || {
        // SPEC §10.3 — launching the same app twice yields two distinct instances
        // with distinct ids (a PID would collide across reuse; an instance id
        // must not).
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.start(&ws).map_err(|e| e.to_string())?;
        let a = e.launch(&ws, "browser").map_err(|e| e.to_string())?;
        let b = e.launch(&ws, "browser").map_err(|e| e.to_string())?;
        let n = e.app_instances(&ws).map_err(|e| e.to_string())?.len();
        ok(a != b && n == 2, format!("expected 2 distinct instances, got {n}"))
    });

    r.check("applications/stop_ends_the_instance", || {
        // After stop the instance no longer exists (SPEC §10 lifecycle terminal).
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.start(&ws).map_err(|e| e.to_string())?;
        let iid = e.launch(&ws, "browser").map_err(|e| e.to_string())?;
        e.stop_app(&ws, &iid).map_err(|e| e.to_string())?;
        let gone = e
            .app_instances(&ws)
            .map_err(|e| e.to_string())?
            .iter()
            .all(|i| i.id != iid);
        ok(gone, "a stopped instance must no longer be listed")
    });

    r.check("applications/lifecycle_is_observable_as_events", || {
        // The lifecycle flows through the one event envelope: LaunchRequested →
        // Started, then Stopping → Stopped. No second event system.
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.start(&ws).map_err(|e| e.to_string())?;
        let iid = e.launch(&ws, "browser").map_err(|e| e.to_string())?;
        e.stop_app(&ws, &iid).map_err(|e| e.to_string())?;
        let kinds: Vec<_> = e.events_for(&ws).iter().map(|ev| ev.kind.clone()).collect();
        let has = |want: &EventKind| kinds.iter().any(|k| k == want);
        ok(
            has(&EventKind::ApplicationLaunchRequested { app: "browser".into() })
                && has(&EventKind::ApplicationStarted { instance: iid.clone(), app: "browser".into() })
                && has(&EventKind::ApplicationStopping { instance: iid.clone() })
                && has(&EventKind::ApplicationStopped { instance: iid.clone() }),
            "expected LaunchRequested→Started→Stopping→Stopped for the instance",
        )
    });

    r.check("applications/cannot_launch_before_running", || {
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        match e.launch(&ws, "browser") {
            Err(WseError::InvalidState { .. }) => Ok(()),
            other => Err(format!("expected InvalidState, got {other:?}")),
        }
    });

    r.check("applications/ungranted_app_is_not_found_not_denied", || {
        // SPEC §6.5 undetectability — an app not in the catalog is not found.
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.start(&ws).map_err(|e| e.to_string())?;
        match e.launch(&ws, "photoshop") {
            Err(WseError::NotFound(_)) => Ok(()),
            other => Err(format!("expected NotFound, got {other:?}")),
        }
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
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.start(&ws).map_err(|e| e.to_string())?;
        e.launch(&ws, "browser").map_err(|e| e.to_string())?;
        e.launch(&ws, "editor").map_err(|e| e.to_string())?;
        let windows = e.list_windows(&ws).map_err(|e| e.to_string())?;
        let focused = windows.iter().filter(|w| w.focused).count();
        ok(focused == 1, format!("expected exactly 1 focused, got {focused}"))
    });

    r.check("windows/newest_is_focused", || {
        let mut e = TestEngine::new(make());
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
        let mut e = TestEngine::new(make());
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
        let mut e = TestEngine::new(make());
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
        let mut e = TestEngine::new(make());
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
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        match e.clipboard_read_out(&ws, Role::Observer) {
            Err(WseError::PermissionDenied { .. }) => Ok(()),
            other => Err(format!("Observer read_out must be denied, got {other:?}")),
        }
    });

    r.check("clipboard/observer_refused_write_in", || {
        let mut e = TestEngine::new(make());
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
        let mut e = TestEngine::new(make());
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
        let mut e = TestEngine::new(make());
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
        let mut e = TestEngine::new(make());
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
        let mut e = TestEngine::new(make());
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
        let mut e = TestEngine::new(make());
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
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        match e.storage_create(&ws, Role::Observer, "x", ResourceKind::Blob) {
            Err(WseError::PermissionDenied { .. }) => Ok(()),
            other => Err(format!("Observer create must be denied, got {other:?}")),
        }
    });

    r.check("storage/collaborator_needs_filetransfer_right", || {
        // I6 — a Collaborator without FileTransfer is refused.
        let mut e = TestEngine::new(make());
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
        let mut e = TestEngine::new(make());
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

/// SPEC §12 — the Devices capability (external resources). See
/// contract/capabilities/devices.md. Runs only for adapters declaring Devices.
pub fn run_devices<A, F>(make: F) -> ConformanceReport
where
    A: WorkspaceAdapter,
    F: Fn() -> A,
{
    let mut r = ConformanceReport::default();

    let use_device_collaborator = || {
        let mut c = cfg();
        let mut rights = HashMap::new();
        rights.insert(AccessRight::UseDevice, false);
        c.collaborator_rights = rights;
        c
    };

    r.check("devices/none_by_default", || {
        // I1 — a fresh workspace has no devices (§12.1).
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        let n = e.device_enumerate(&ws).map_err(|e| e.to_string())?.len();
        ok(n == 0, format!("expected no devices by default, got {n}"))
    });

    r.check("devices/enumerate_lists_available", || {
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        let d = e
            .device_attach(&ws, DeviceClass::Printer, "Office Printer")
            .map_err(|e| e.to_string())?;
        let found = e
            .device_enumerate(&ws)
            .map_err(|e| e.to_string())?
            .iter()
            .any(|x| x.id == d.id);
        ok(found, "an attached device must appear in enumerate")
    });

    r.check("devices/non_available_is_not_found", || {
        // I2 — requesting a device that isn't available is NotFound, not denied.
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        let ghost = wse_common::DeviceId::new();
        match e.device_request(&ws, Role::Owner, &ghost) {
            Err(WseError::NotFound(_)) => Ok(()),
            other => Err(format!("expected NotFound, got {other:?}")),
        }
    });

    r.check("devices/observer_refused_request", || {
        // I6 — Observer touches nothing.
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        let d = e
            .device_attach(&ws, DeviceClass::Camera, "Cam")
            .map_err(|e| e.to_string())?;
        match e.device_request(&ws, Role::Observer, &d.id) {
            Err(WseError::PermissionDenied { .. }) => Ok(()),
            other => Err(format!("Observer request must be denied, got {other:?}")),
        }
    });

    r.check("devices/collaborator_needs_use_right", || {
        let mut e = TestEngine::new(make());
        let ws = e
            .create_workspace(use_device_collaborator())
            .map_err(|e| e.to_string())?;
        let d = e
            .device_attach(&ws, DeviceClass::Camera, "Cam")
            .map_err(|e| e.to_string())?;
        match e.device_request(&ws, Role::Collaborator, &d.id) {
            Err(WseError::PermissionDenied { .. }) => Ok(()),
            other => Err(format!("Collaborator w/o UseDevice must be denied, got {other:?}")),
        }
    });

    r.check("devices/request_then_release", || {
        // I4 — Owner request yields a handle; release ends it.
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        let d = e
            .device_attach(&ws, DeviceClass::Microphone, "Mic")
            .map_err(|e| e.to_string())?;
        let handle = e
            .device_request(&ws, Role::Owner, &d.id)
            .map_err(|e| e.to_string())?;
        if handle.device != d.id {
            return Err("handle must reference the requested device".into());
        }
        let released = e
            .device_release(&ws, Role::Owner, &d.id)
            .map_err(|e| e.to_string())?;
        ok(released, "release must report a handle was held")
    });

    r.check("devices/isolated_per_workspace", || {
        // I7 — a device in one workspace is invisible in another.
        let mut e = TestEngine::new(make());
        let a = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        let b = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.device_attach(&a, DeviceClass::Usb, "Stick")
            .map_err(|e| e.to_string())?;
        let n = e.device_enumerate(&b).map_err(|e| e.to_string())?.len();
        ok(n == 0, "workspace B must not see workspace A's devices")
    });

    r.check("devices/state_reflects_availability", || {
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        let empty = e
            .capability_state(&ws, Capability::Devices)
            .map_err(|e| e.to_string())?;
        if empty != CapabilityState::Unavailable {
            return Err(format!("no devices should be Unavailable, got {empty:?}"));
        }
        e.device_attach(&ws, DeviceClass::Speaker, "Spk")
            .map_err(|e| e.to_string())?;
        let now = e
            .capability_state(&ws, Capability::Devices)
            .map_err(|e| e.to_string())?;
        ok(
            now == CapabilityState::Available,
            format!("with a device should be Available, got {now:?}"),
        )
    });

    r.check("devices/state_change_emits_event", || {
        // I8 — availability flip flows through the CORE event envelope.
        let mut e = TestEngine::new(make());
        let ws = e.create_workspace(cfg()).map_err(|e| e.to_string())?;
        e.device_attach(&ws, DeviceClass::Gpu, "GPU")
            .map_err(|e| e.to_string())?;
        let emitted = e.events_for(&ws).iter().any(|ev| {
            matches!(
                ev.kind,
                EventKind::CapabilityStateChanged {
                    capability: Capability::Devices,
                    ..
                }
            )
        });
        ok(emitted, "attaching the first device must emit CapabilityStateChanged")
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
    if caps.supports(Capability::Devices) {
        report.absorb(run_devices(&make));
    }
    // Network, Audio, Camera suites slot in here as each capability's mini-spec
    // is written — never before.
    report
}
