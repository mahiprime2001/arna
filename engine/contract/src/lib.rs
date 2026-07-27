//! wse-contract — the Workspace Contract, expressed as Rust traits. This is the
//! machine-checkable form of /contract/CONTRACT.md. It is the ONLY thing the
//! engine knows about a platform. It imports no OS APIs.
//!
//! Two layers (VISION.md Pillar 3, SPEC §18.2/§18.3):
//!   - a MANDATORY CORE every adapter must satisfy identically (isolation), and
//!   - DECLARED CAPABILITIES that MAY differ per adapter, never faked.

use std::fmt;

use wse_common::*;

/// The contract is versioned. Adapters declare which version they speak; the
/// engine refuses an incompatible one. Same major = compatible (minor is
/// additive). This gives WSE a compatibility story as the platform evolves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ContractVersion {
    pub major: u16,
    pub minor: u16,
}

impl ContractVersion {
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }
    /// An adapter speaking `self` can serve an engine speaking `engine` when the
    /// major versions match. (Pre-1.0, we treat 0.x as compatible within 0.x.)
    pub fn compatible_with(self, engine: ContractVersion) -> bool {
        self.major == engine.major
    }
}

impl fmt::Display for ContractVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}.{}", self.major, self.minor)
    }
}

/// The version this build of the contract defines.
pub const CONTRACT_VERSION: ContractVersion = ContractVersion::new(0, 1);

/// What the engine hands an adapter to create a workspace. Platform-neutral.
#[derive(Clone, Debug)]
pub struct WorkspaceDef {
    pub id: WorkspaceId,
    pub name: String,
    pub persistence: Persistence,
    pub limits: ResourceLimits,
}

/// The isolation **evidence** an adapter returns when it starts a workspace.
///
/// Deliberately named an *attestation*, not a proof: an adapter cannot
/// mathematically prove isolation. It reports what it measured and checked
/// (`sealed`, plus human-readable `details`). The ENGINE evaluates this
/// attestation against its isolation policy (SPEC §18.3) and decides whether to
/// run the workspace. The adapter provides evidence; the engine owns the policy.
#[derive(Clone, Debug, Default)]
pub struct IsolationAttestation {
    /// The adapter's assessment that the workspace is sealed (no host
    /// filesystem, no host interop, etc.). Evidence, evaluated — never trusted
    /// blindly.
    pub sealed: bool,
    pub details: Vec<String>,
}

/// The boundary. Each platform implements this trait; nothing above it is
/// platform-specific. Methods cite the spec obligation they satisfy.
pub trait WorkspaceAdapter {
    /// Which contract version this adapter speaks (SPEC §18.4 / compatibility).
    fn contract_version(&self) -> ContractVersion {
        CONTRACT_VERSION
    }

    /// SPEC §18.2 — declare the workspace capabilities this adapter provides,
    /// honestly. Undeclared means absent. `CapabilitySet` lives in wse-common.
    fn capabilities(&self) -> CapabilitySet;

    /// SPEC §5.1 / §3 — define a workspace; not yet executing.
    fn create(&mut self, def: &WorkspaceDef) -> Result<()>;

    /// SPEC §5 / §18.3 — begin executing AND return an isolation attestation.
    /// The engine evaluates the attestation against policy; a workspace the
    /// engine judges unsealed is never presented as running.
    fn start(&mut self, id: &WorkspaceId) -> Result<IsolationAttestation>;

    /// SPEC §5 — suspend execution (the workspace itself survives, §3.2).
    fn stop(&mut self, id: &WorkspaceId) -> Result<()>;

    /// SPEC §5.5 — destroy contents irrecoverably; not merely unlist.
    fn destroy(&mut self, id: &WorkspaceId) -> Result<()>;

    /// SPEC §10 — launch a catalog app inside the workspace; it becomes a
    /// window on the canvas. The engine has already checked the app is granted.
    fn launch(&mut self, id: &WorkspaceId, app: &AppSpec) -> Result<Window>;

    /// SPEC §14 — the windows currently open in the workspace (metadata only).
    fn list_windows(&self, id: &WorkspaceId) -> Result<Vec<Window>>;

    // ── capability interfaces ────────────────────────────────────────────────
    // Each capability is its own trait. An adapter exposes the interface for a
    // capability it declares, and None for one it doesn't. The engine negotiates
    // via these hooks, gates on policy, then calls the mechanical interface.

    /// SPEC §9 — the Clipboard capability, if this adapter provides it.
    fn clipboard(&mut self) -> Option<&mut dyn ClipboardCapability> {
        None
    }
}

/// SPEC §9 — the mechanical Clipboard interface. The adapter reads/writes the
/// workspace's OWN clipboard; it never decides who may. Policy (role, access
/// right, direction) lives in the engine. See contract/capabilities/clipboard.md.
pub trait ClipboardCapability {
    /// The workspace clipboard's current content, or None if empty.
    fn clipboard_peek(&self, id: &WorkspaceId) -> Result<Option<ClipboardData>>;
    /// Replace the workspace clipboard content.
    fn clipboard_put(&mut self, id: &WorkspaceId, data: ClipboardData) -> Result<()>;
}
