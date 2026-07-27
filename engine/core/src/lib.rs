//! wse-engine — the orchestrator. It owns workspace lifecycle, the window
//! registry, permissions, and the event stream, and it drives whatever adapter
//! it is handed. It knows the *contract*; it never knows an OS.
//!
//! Design choices (deliberately small for the first milestone):
//!   - Direct, typed calls between concerns; clear ownership; easy to test.
//!   - Events are for OBSERVATION (things the outside subscribes to), not the
//!     control mechanism. More managers get split out as behaviour demands them.

use std::collections::HashMap;

use wse_common::*;
use wse_contract::{IsolationReport, WorkspaceAdapter, WorkspaceDef};

/// What the caller asks for when creating a workspace.
pub struct WorkspaceConfig {
    pub name: String,
    pub persistence: Persistence,
    /// The catalog the workspace may launch from (SPEC §7.1 / §10.1). Anything
    /// not here is *not found*, never *denied* (SPEC §6.5).
    pub apps: Vec<AppSpec>,
    pub limits: ResourceLimits,
    /// Collaborator grants, within what host policy allows (SPEC §4.6.2).
    /// Defaults are deny-by-default except view (SPEC §6.1, §4.6).
    pub collaborator_grants: HashMap<Capability, bool>,
}

impl WorkspaceConfig {
    /// Spec-faithful defaults: deny-by-default, except viewing the display.
    pub fn new(name: impl Into<String>, persistence: Persistence, apps: Vec<AppSpec>) -> Self {
        let mut grants = HashMap::new();
        grants.insert(Capability::ViewDisplay, true);
        grants.insert(Capability::Keyboard, true);
        grants.insert(Capability::Pointer, true);
        grants.insert(Capability::ClipboardRead, false);
        grants.insert(Capability::ClipboardWrite, false);
        grants.insert(Capability::FileTransfer, false);
        Self {
            name: name.into(),
            persistence,
            apps,
            limits: ResourceLimits::default(),
            collaborator_grants: grants,
        }
    }
}

/// Things the outside world can subscribe to. Never used as control flow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    WorkspaceCreated(WorkspaceId),
    StateChanged {
        workspace: WorkspaceId,
        from: WorkspaceState,
        to: WorkspaceState,
    },
    AppLaunched {
        workspace: WorkspaceId,
        app: String,
        window: WindowId,
    },
    WorkspaceDestroyed(WorkspaceId),
}

/// The engine's own record of a workspace (its policy + state). The adapter
/// holds the running reality; this holds the meaning.
struct Record {
    name: String,
    state: WorkspaceState,
    persistence: Persistence,
    catalog: Vec<AppSpec>,
    #[allow(dead_code)]
    collaborator_grants: HashMap<Capability, bool>,
}

pub struct Engine<A: WorkspaceAdapter> {
    adapter: A,
    workspaces: HashMap<WorkspaceId, Record>,
    events: Vec<Event>,
}

impl<A: WorkspaceAdapter> Engine<A> {
    pub fn new(adapter: A) -> Self {
        Self {
            adapter,
            workspaces: HashMap::new(),
            events: Vec::new(),
        }
    }

    /// The declared capabilities of the underlying adapter (SPEC §18.2).
    pub fn capabilities(&self) -> wse_contract::CapabilitySet {
        self.adapter.capabilities()
    }

    /// Everything the outside world may observe, in order.
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    pub fn state(&self, id: &WorkspaceId) -> Option<WorkspaceState> {
        self.workspaces.get(id).map(|r| r.state)
    }

    /// Create a workspace. It exists (Created) but does not execute yet.
    pub fn create_workspace(&mut self, cfg: WorkspaceConfig) -> Result<WorkspaceId> {
        let id = WorkspaceId::new();
        let def = WorkspaceDef {
            id: id.clone(),
            name: cfg.name.clone(),
            persistence: cfg.persistence,
            limits: cfg.limits,
        };
        self.adapter.create(&def)?;
        self.workspaces.insert(
            id.clone(),
            Record {
                name: cfg.name,
                state: WorkspaceState::Created,
                persistence: cfg.persistence,
                catalog: cfg.apps,
                collaborator_grants: cfg.collaborator_grants,
            },
        );
        self.events.push(Event::WorkspaceCreated(id.clone()));
        Ok(id)
    }

    /// Start (or resume) a workspace. The engine ONLY marks it running once the
    /// adapter proves it is sealed (SPEC §18.3). If not, it stops it and errors.
    pub fn start(&mut self, id: &WorkspaceId) -> Result<()> {
        let from = self.require_state(id)?;
        self.check_transition(from, WorkspaceState::Running)?;

        let report: IsolationReport = self.adapter.start(id)?;
        if !report.sealed {
            // Refuse to present an unsealed workspace as running. Best-effort stop.
            let _ = self.adapter.stop(id);
            return Err(WseError::NotIsolated {
                workspace: id.clone(),
                details: report.details,
            });
        }
        self.set_state(id, WorkspaceState::Running);
        Ok(())
    }

    /// Suspend execution. The workspace record survives (SPEC §3.2).
    pub fn stop(&mut self, id: &WorkspaceId) -> Result<()> {
        let from = self.require_state(id)?;
        self.check_transition(from, WorkspaceState::Saved)?;
        self.adapter.stop(id)?;
        self.set_state(id, WorkspaceState::Saved);
        Ok(())
    }

    /// Launch a catalog app. It must be running, and the app must be in the
    /// catalog — otherwise it is *not found*, never *denied* (SPEC §6.5).
    pub fn launch(&mut self, id: &WorkspaceId, app_id: &str) -> Result<WindowId> {
        let rec = self.workspaces.get(id).ok_or(WseError::NotFound(format!(
            "workspace {id}"
        )))?;
        if rec.state != WorkspaceState::Running {
            return Err(WseError::NotRunning(id.clone()));
        }
        let app = rec
            .catalog
            .iter()
            .find(|a| a.id == app_id)
            .cloned()
            // §6.5: an un-granted app is indistinguishable from a nonexistent one.
            .ok_or_else(|| WseError::NotFound(format!("app {app_id}")))?;

        let window = self.adapter.launch(id, &app)?;
        let wid = window.id.clone();
        self.events.push(Event::AppLaunched {
            workspace: id.clone(),
            app: app.id,
            window: wid.clone(),
        });
        Ok(wid)
    }

    /// The windows open in the workspace (SPEC §14, metadata only).
    pub fn list_windows(&self, id: &WorkspaceId) -> Result<Vec<Window>> {
        if !self.workspaces.contains_key(id) {
            return Err(WseError::NotFound(format!("workspace {id}")));
        }
        self.adapter.list_windows(id)
    }

    /// SPEC §5.5 — destroy irrecoverably.
    pub fn destroy(&mut self, id: &WorkspaceId) -> Result<()> {
        let from = self.require_state(id)?;
        self.check_transition(from, WorkspaceState::Deleted)?;
        self.adapter.destroy(id)?;
        self.workspaces.remove(id);
        self.events.push(Event::WorkspaceDestroyed(id.clone()));
        Ok(())
    }

    // ── internals ───────────────────────────────────────────────────────────
    fn require_state(&self, id: &WorkspaceId) -> Result<WorkspaceState> {
        self.workspaces
            .get(id)
            .map(|r| r.state)
            .ok_or_else(|| WseError::NotFound(format!("workspace {id}")))
    }

    fn check_transition(&self, from: WorkspaceState, to: WorkspaceState) -> Result<()> {
        if from.can_transition(to) {
            Ok(())
        } else {
            Err(WseError::InvalidTransition { from, to })
        }
    }

    fn set_state(&mut self, id: &WorkspaceId, to: WorkspaceState) {
        if let Some(rec) = self.workspaces.get_mut(id) {
            let from = rec.state;
            rec.state = to;
            self.events.push(Event::StateChanged {
                workspace: id.clone(),
                from,
                to,
            });
        }
    }

    /// Read-only peek at a workspace's name + persistence (for a manager/UI).
    pub fn describe(&self, id: &WorkspaceId) -> Option<(String, Persistence, WorkspaceState)> {
        self.workspaces
            .get(id)
            .map(|r| (r.name.clone(), r.persistence, r.state))
    }
}
