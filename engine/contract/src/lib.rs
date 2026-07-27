//! wse-contract — the Workspace Contract, expressed as Rust traits. This is the
//! machine-checkable form of /contract/CONTRACT.md. It is the ONLY thing the
//! engine knows about a platform. It imports no OS APIs.
//!
//! Two layers (VISION.md Pillar 3, SPEC §18.2/§18.3):
//!   - a MANDATORY CORE every adapter must satisfy identically (isolation), and
//!   - DECLARED CAPABILITIES that MAY differ per adapter, never faked.

use wse_common::*;

/// Declared capabilities (SPEC §18.2). MAY differ per adapter; undeclared means
/// absent. These never include isolation — that is the mandatory core (§18.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CapabilitySet {
    pub gpu_acceleration: bool,
    pub live_memory_snapshot: bool,
    pub usb_passthrough: bool,
    pub audio: bool,
    pub display_hotplug: bool,
    /// Tier-1 native application support for the platform's own OS (e.g. real
    /// Windows .exe on the Windows adapter). A Linux-in-VM adapter declares
    /// this false and offers Linux apps (plus Wine as its own capability).
    pub native_apps: bool,
}

/// What the engine hands an adapter to create a workspace. Platform-neutral.
#[derive(Clone, Debug)]
pub struct WorkspaceDef {
    pub id: WorkspaceId,
    pub name: String,
    pub persistence: Persistence,
    pub limits: ResourceLimits,
}

/// The evidence an adapter returns to prove a started workspace is sealed. The
/// engine refuses to present an unsealed workspace as running (SPEC §18.3).
#[derive(Clone, Debug, Default)]
pub struct IsolationReport {
    pub sealed: bool,
    pub details: Vec<String>,
}

/// The boundary. Each platform implements this trait; nothing above it is
/// platform-specific. Methods cite the spec obligation they satisfy.
pub trait WorkspaceAdapter {
    /// SPEC §18.2 — declare capabilities honestly.
    fn capabilities(&self) -> CapabilitySet;

    /// SPEC §5.1 / §3 — define a workspace; not yet executing.
    fn create(&mut self, def: &WorkspaceDef) -> Result<()>;

    /// SPEC §5 / §18.3 — begin executing AND return proof of isolation. An
    /// adapter that cannot prove the seal returns `sealed: false`; the engine
    /// then refuses to run it rather than expose something leaky.
    fn start(&mut self, id: &WorkspaceId) -> Result<IsolationReport>;

    /// SPEC §5 — suspend execution (the workspace itself survives, §3.2).
    fn stop(&mut self, id: &WorkspaceId) -> Result<()>;

    /// SPEC §5.5 — destroy contents irrecoverably; not merely unlist.
    fn destroy(&mut self, id: &WorkspaceId) -> Result<()>;

    /// SPEC §10 — launch a catalog app inside the workspace; it becomes a
    /// window on the canvas. The engine has already checked the app is granted.
    fn launch(&mut self, id: &WorkspaceId, app: &AppSpec) -> Result<Window>;

    /// SPEC §14 — the windows currently open in the workspace (metadata only).
    fn list_windows(&self, id: &WorkspaceId) -> Result<Vec<Window>>;
}
