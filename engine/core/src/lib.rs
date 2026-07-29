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
use wse_contract::{
    ContractVersion, IsolationAttestation, WorkspaceAdapter, WorkspaceDef, CONTRACT_VERSION,
};

/// The engine's isolation policy — what it *requires* of an adapter's
/// attestation before it will run a workspace. The adapter provides evidence;
/// this policy is how the engine evaluates it (SPEC §18.3). Policy lives in the
/// engine, never in the adapter.
#[derive(Clone, Copy, Debug)]
pub struct IsolationPolicy {
    /// The mandatory core. There is no partial-isolation tier, so this is true
    /// by default and cannot be relaxed away in a conforming deployment.
    pub require_sealed: bool,
}

impl Default for IsolationPolicy {
    fn default() -> Self {
        Self { require_sealed: true }
    }
}

impl IsolationPolicy {
    /// Evaluate an adapter's attestation. Returns Ok, or the reasons it fails.
    pub fn evaluate(&self, att: &IsolationAttestation) -> std::result::Result<(), Vec<String>> {
        if self.require_sealed && !att.sealed {
            let mut why = att.details.clone();
            if why.is_empty() {
                why.push("adapter did not attest the workspace is sealed".into());
            }
            Err(why)
        } else {
            Ok(())
        }
    }
}

/// What the caller asks for when creating a workspace.
pub struct WorkspaceConfig {
    pub name: String,
    pub persistence: Persistence,
    /// The catalog the workspace may launch from (SPEC §7.1 / §10.1). Anything
    /// not here is *not found*, never *denied* (SPEC §6.5).
    pub apps: Vec<AppSpec>,
    pub limits: ResourceLimits,
    /// Collaborator access rights, within what host policy allows (§4.6.2).
    /// Defaults are deny-by-default except view/input (SPEC §6.1, §4.6).
    pub collaborator_rights: HashMap<AccessRight, bool>,
    /// The workspace owner (SPEC §4). Optional at this stage; a fresh one is
    /// minted if absent.
    pub owner: Option<MemberId>,
    /// Free-form metadata the owner attaches (labels, project id, …).
    pub metadata: HashMap<String, String>,
}

impl WorkspaceConfig {
    /// Spec-faithful defaults: deny-by-default, except viewing + input.
    pub fn new(name: impl Into<String>, persistence: Persistence, apps: Vec<AppSpec>) -> Self {
        let mut rights = HashMap::new();
        rights.insert(AccessRight::ViewDisplay, true);
        rights.insert(AccessRight::Keyboard, true);
        rights.insert(AccessRight::Pointer, true);
        rights.insert(AccessRight::ClipboardRead, false);
        rights.insert(AccessRight::ClipboardWrite, false);
        rights.insert(AccessRight::FileTransfer, false);
        Self {
            name: name.into(),
            persistence,
            apps,
            limits: ResourceLimits::default(),
            collaborator_rights: rights,
            owner: None,
            metadata: HashMap::new(),
        }
    }
}

/// The identity of a workspace — everything the engine knows about *what a
/// workspace is*, independent of the adapter running it (SPEC §4, §18.4).
#[derive(Clone, Debug)]
pub struct WorkspaceIdentity {
    pub id: WorkspaceId,
    pub name: String,
    pub state: WorkspaceState,
    pub persistence: Persistence,
    pub owner: MemberId,
    pub members: Vec<Member>,
    /// The capabilities this workspace provides (adapter-declared, §18.2).
    pub capabilities: CapabilitySet,
    pub contract_version: ContractVersion,
    /// The most recent isolation attestation the engine evaluated (§18.3).
    pub last_attestation: Option<IsolationAttestation>,
    pub metadata: HashMap<String, String>,
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
    /// SPEC §17.1 — clipboard transfers are auditable. The event carries who and
    /// which direction, NEVER the content (I5 in the clipboard spec).
    ClipboardReadOut {
        workspace: WorkspaceId,
        by: Role,
    },
    ClipboardWrittenIn {
        workspace: WorkspaceId,
        by: Role,
    },
    /// SPEC §17.1 — storage transfers are auditable: who, which operation, which
    /// resource — never the bytes (storage spec I7).
    StorageTransfer {
        workspace: WorkspaceId,
        by: Role,
        op: StorageOp,
        resource: ResourceId,
    },
    WorkspaceDestroyed(WorkspaceId),
}

/// The storage operations that cross the boundary and are audited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageOp {
    Create,
    Write,
    Read,
    Delete,
}

/// Who is asking — the authorization context. Today just a Role; a MemberId
/// that *resolves* to a role arrives with collaboration, and capability
/// operations will still see no more than this. A capability never learns about
/// identity, sessions, or networking (good layering).
#[derive(Clone, Copy, Debug)]
pub struct AuthContext {
    pub role: Role,
}

impl AuthContext {
    pub fn role(role: Role) -> Self {
        Self { role }
    }
}

/// The grants in effect for a workspace, as the authorizer sees them (§4.6.2).
pub struct GrantView<'a> {
    pub collaborator_rights: &'a HashMap<AccessRight, bool>,
}

/// The policy interface. **The engine asks; the policy system answers.** The
/// engine depends on this trait, never on a concrete policy — today a simple
/// role matrix, tomorrow a full Permission Manager (role → right → capability →
/// decision) without touching a single capability.
pub trait Authorizer {
    fn allows(&self, ctx: &AuthContext, grants: &GrantView, right: AccessRight) -> bool;
}

/// The default policy: the SPEC §4.6 capability matrix. Owner has all; Observer
/// may only ViewDisplay (observing is not extracting, §4.6.1); Collaborator gets
/// what host policy granted, deny-by-default (§6.1).
pub struct RoleMatrixAuthorizer;

impl Authorizer for RoleMatrixAuthorizer {
    fn allows(&self, ctx: &AuthContext, grants: &GrantView, right: AccessRight) -> bool {
        match ctx.role {
            Role::Owner => true,
            Role::Observer => right == AccessRight::ViewDisplay,
            Role::Collaborator => grants
                .collaborator_rights
                .get(&right)
                .copied()
                .unwrap_or(false),
        }
    }
}

/// The engine's own record of a workspace (its policy + state). The adapter
/// holds the running reality; this holds the meaning.
struct Record {
    name: String,
    state: WorkspaceState,
    persistence: Persistence,
    catalog: Vec<AppSpec>,
    #[allow(dead_code)]
    collaborator_rights: HashMap<AccessRight, bool>,
    owner: MemberId,
    members: Vec<Member>,
    /// The effective capabilities of this workspace (SPEC §18.2). Today this is
    /// what the adapter declares; §6.4's host∩owner narrowing lands here later.
    capabilities: CapabilitySet,
    contract_version: ContractVersion,
    last_attestation: Option<IsolationAttestation>,
    metadata: HashMap<String, String>,
}

pub struct Engine<A: WorkspaceAdapter> {
    adapter: A,
    workspaces: HashMap<WorkspaceId, Record>,
    events: Vec<Event>,
    isolation_policy: IsolationPolicy,
    authorizer: Box<dyn Authorizer>,
}

impl<A: WorkspaceAdapter> Engine<A> {
    pub fn new(adapter: A) -> Self {
        Self {
            adapter,
            workspaces: HashMap::new(),
            events: Vec::new(),
            isolation_policy: IsolationPolicy::default(),
            authorizer: Box::new(RoleMatrixAuthorizer),
        }
    }

    /// Swap the policy system. Defaults to the SPEC §4.6 role matrix.
    pub fn with_authorizer(mut self, authorizer: Box<dyn Authorizer>) -> Self {
        self.authorizer = authorizer;
        self
    }

    /// The isolation policy the engine evaluates attestations against.
    pub fn isolation_policy(&self) -> IsolationPolicy {
        self.isolation_policy
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
        // The adapter must speak a compatible contract version (SPEC §18.4).
        let v = self.adapter.contract_version();
        if !v.compatible_with(CONTRACT_VERSION) {
            return Err(WseError::ContractMismatch {
                adapter: v.to_string(),
                engine: CONTRACT_VERSION.to_string(),
            });
        }
        let id = WorkspaceId::new();
        let def = WorkspaceDef {
            id: id.clone(),
            name: cfg.name.clone(),
            persistence: cfg.persistence,
            limits: cfg.limits,
        };
        self.adapter.create(&def)?;

        // Capability negotiation: the workspace provides what the adapter
        // declares (SPEC §18.2). The engine will decide on capabilities, never
        // on platform names.
        let capabilities = self.adapter.capabilities();
        let owner = cfg.owner.unwrap_or_default();
        let members = vec![Member {
            id: owner.clone(),
            role: Role::Owner,
        }];

        self.workspaces.insert(
            id.clone(),
            Record {
                name: cfg.name,
                state: WorkspaceState::Created,
                persistence: cfg.persistence,
                catalog: cfg.apps,
                collaborator_rights: cfg.collaborator_rights,
                owner,
                members,
                capabilities,
                contract_version: v,
                last_attestation: None,
                metadata: cfg.metadata,
            },
        );
        self.events.push(Event::WorkspaceCreated(id.clone()));
        Ok(id)
    }

    /// Capability negotiation — the engine asks *what a workspace provides*,
    /// never *what platform it runs on* (SPEC §18.2).
    pub fn capabilities(&self, id: &WorkspaceId) -> Option<CapabilitySet> {
        self.workspaces.get(id).map(|r| r.capabilities)
    }

    /// Does this workspace provide a given capability?
    pub fn supports(&self, id: &WorkspaceId, cap: Capability) -> bool {
        self.workspaces
            .get(id)
            .map(|r| r.capabilities.supports(cap))
            .unwrap_or(false)
    }

    /// The full identity of a workspace (SPEC §4, §18.4).
    pub fn identity(&self, id: &WorkspaceId) -> Option<WorkspaceIdentity> {
        self.workspaces.get(id).map(|r| WorkspaceIdentity {
            id: id.clone(),
            name: r.name.clone(),
            state: r.state,
            persistence: r.persistence,
            owner: r.owner.clone(),
            members: r.members.clone(),
            capabilities: r.capabilities,
            contract_version: r.contract_version,
            last_attestation: r.last_attestation.clone(),
            metadata: r.metadata.clone(),
        })
    }

    /// Start (or resume) a workspace. The adapter *attests* to isolation; the
    /// engine *evaluates* that attestation against its policy and only then
    /// marks the workspace running (SPEC §18.3). If the evidence is rejected,
    /// the engine stops it and errors — no partial-isolation tier.
    pub fn start(&mut self, id: &WorkspaceId) -> Result<()> {
        let from = self.require_state(id)?;
        self.check_transition(from, WorkspaceState::Running)?;

        let attestation: IsolationAttestation = self.adapter.start(id)?;
        // Record the evidence on the workspace's identity either way.
        if let Some(rec) = self.workspaces.get_mut(id) {
            rec.last_attestation = Some(attestation.clone());
        }
        if let Err(details) = self.isolation_policy.evaluate(&attestation) {
            // Reject the evidence; do not present an unsealed workspace as
            // running. Best-effort stop.
            let _ = self.adapter.stop(id);
            return Err(WseError::IsolationRejected {
                workspace: id.clone(),
                details,
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
            return Err(WseError::InvalidState {
                operation: "launch",
                state: rec.state,
            });
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

    // ── Clipboard capability (SPEC §9) ───────────────────────────────────────
    // The engine owns the policy (capability declared? role holds the right?),
    // then calls the adapter's mechanical interface. See
    // contract/capabilities/clipboard.md.

    /// A member copies OUT of the workspace clipboard (SPEC §9.2 direction:
    /// read). Requires the Clipboard capability and the ClipboardRead right.
    pub fn clipboard_read_out(
        &mut self,
        id: &WorkspaceId,
        role: Role,
    ) -> Result<Option<ClipboardItem>> {
        // Policy first (extract booleans so the record borrow ends here).
        {
            let rec = self
                .workspaces
                .get(id)
                .ok_or_else(|| WseError::NotFound(format!("workspace {id}")))?;
            if !rec.capabilities.supports(Capability::Clipboard) {
                return Err(WseError::CapabilityUnavailable(Capability::Clipboard));
            }
            let allowed = self.authorizer.allows(
                &AuthContext::role(role),
                &GrantView {
                    collaborator_rights: &rec.collaborator_rights,
                },
                AccessRight::ClipboardRead,
            );
            if !allowed {
                return Err(WseError::PermissionDenied {
                    right: AccessRight::ClipboardRead,
                    role,
                });
            }
        }
        // Mechanics.
        let clip = self
            .adapter
            .clipboard()
            .ok_or(WseError::CapabilityUnavailable(Capability::Clipboard))?;
        let data = clip.clipboard_peek(id)?;
        self.events.push(Event::ClipboardReadOut {
            workspace: id.clone(),
            by: role,
        });
        Ok(data)
    }

    /// A member pastes INTO the workspace clipboard (SPEC §9.2 direction:
    /// write). Requires the Clipboard capability and the ClipboardWrite right.
    pub fn clipboard_write_in(
        &mut self,
        id: &WorkspaceId,
        role: Role,
        data: ClipboardItem,
    ) -> Result<()> {
        {
            let rec = self
                .workspaces
                .get(id)
                .ok_or_else(|| WseError::NotFound(format!("workspace {id}")))?;
            if !rec.capabilities.supports(Capability::Clipboard) {
                return Err(WseError::CapabilityUnavailable(Capability::Clipboard));
            }
            let allowed = self.authorizer.allows(
                &AuthContext::role(role),
                &GrantView {
                    collaborator_rights: &rec.collaborator_rights,
                },
                AccessRight::ClipboardWrite,
            );
            if !allowed {
                return Err(WseError::PermissionDenied {
                    right: AccessRight::ClipboardWrite,
                    role,
                });
            }
        }
        let clip = self
            .adapter
            .clipboard()
            .ok_or(WseError::CapabilityUnavailable(Capability::Clipboard))?;
        clip.clipboard_put(id, data)?;
        self.events.push(Event::ClipboardWrittenIn {
            workspace: id.clone(),
            by: role,
        });
        Ok(())
    }

    // ── Storage capability (SPEC §8) ─────────────────────────────────────────
    // Persistent workspace-owned resources. A member's create/write/read/delete
    // crosses the boundary and needs the FileTransfer right; list is host
    // introspection (§17.2). See contract/capabilities/storage.md.

    /// Gate a boundary storage op: the workspace must provide Storage and the
    /// role must hold FileTransfer. Returns Ok once both hold.
    fn gate_storage_transfer(&self, id: &WorkspaceId, role: Role) -> Result<()> {
        let rec = self
            .workspaces
            .get(id)
            .ok_or_else(|| WseError::NotFound(format!("workspace {id}")))?;
        if !rec.capabilities.supports(Capability::Storage) {
            return Err(WseError::CapabilityUnavailable(Capability::Storage));
        }
        let allowed = self.authorizer.allows(
            &AuthContext::role(role),
            &GrantView {
                collaborator_rights: &rec.collaborator_rights,
            },
            AccessRight::FileTransfer,
        );
        if !allowed {
            return Err(WseError::PermissionDenied {
                right: AccessRight::FileTransfer,
                role,
            });
        }
        Ok(())
    }

    /// A member creates a workspace resource (SPEC §8). Requires FileTransfer.
    pub fn storage_create(
        &mut self,
        id: &WorkspaceId,
        role: Role,
        name: impl Into<String>,
        kind: ResourceKind,
    ) -> Result<ResourceMetadata> {
        self.gate_storage_transfer(id, role)?;
        let store = self
            .adapter
            .storage()
            .ok_or(WseError::CapabilityUnavailable(Capability::Storage))?;
        let meta = store.resource_create(id, name.into(), kind)?;
        self.events.push(Event::StorageTransfer {
            workspace: id.clone(),
            by: role,
            op: StorageOp::Create,
            resource: meta.id.clone(),
        });
        Ok(meta)
    }

    /// A member writes bytes into a resource. Requires FileTransfer.
    pub fn storage_write(
        &mut self,
        id: &WorkspaceId,
        role: Role,
        resource: &ResourceId,
        bytes: Vec<u8>,
    ) -> Result<()> {
        self.gate_storage_transfer(id, role)?;
        let store = self
            .adapter
            .storage()
            .ok_or(WseError::CapabilityUnavailable(Capability::Storage))?;
        store.resource_write(id, resource, bytes)?;
        self.events.push(Event::StorageTransfer {
            workspace: id.clone(),
            by: role,
            op: StorageOp::Write,
            resource: resource.clone(),
        });
        Ok(())
    }

    /// A member reads a resource's bytes out. Requires FileTransfer. `NotFound`
    /// if the resource is unknown or deleted (deletion is terminal, I3).
    pub fn storage_read(
        &mut self,
        id: &WorkspaceId,
        role: Role,
        resource: &ResourceId,
    ) -> Result<Vec<u8>> {
        self.gate_storage_transfer(id, role)?;
        let store = self
            .adapter
            .storage()
            .ok_or(WseError::CapabilityUnavailable(Capability::Storage))?;
        let bytes = store.resource_read(id, resource)?;
        self.events.push(Event::StorageTransfer {
            workspace: id.clone(),
            by: role,
            op: StorageOp::Read,
            resource: resource.clone(),
        });
        Ok(bytes)
    }

    /// A member deletes a resource. Requires FileTransfer. Returns whether it
    /// existed; deletion is terminal.
    pub fn storage_delete(
        &mut self,
        id: &WorkspaceId,
        role: Role,
        resource: &ResourceId,
    ) -> Result<bool> {
        self.gate_storage_transfer(id, role)?;
        let store = self
            .adapter
            .storage()
            .ok_or(WseError::CapabilityUnavailable(Capability::Storage))?;
        let existed = store.resource_delete(id, resource)?;
        self.events.push(Event::StorageTransfer {
            workspace: id.clone(),
            by: role,
            op: StorageOp::Delete,
            resource: resource.clone(),
        });
        Ok(existed)
    }

    /// Host/owner introspection: the resources a workspace holds (§17.2, "what
    /// can this workspace see?"). Metadata only, no bytes; not a boundary
    /// transfer, so ungated.
    pub fn storage_list(&mut self, id: &WorkspaceId) -> Result<Vec<ResourceMetadata>> {
        {
            let rec = self
                .workspaces
                .get(id)
                .ok_or_else(|| WseError::NotFound(format!("workspace {id}")))?;
            if !rec.capabilities.supports(Capability::Storage) {
                return Err(WseError::CapabilityUnavailable(Capability::Storage));
            }
        }
        let store = self
            .adapter
            .storage()
            .ok_or(WseError::CapabilityUnavailable(Capability::Storage))?;
        store.resource_list(id)
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
