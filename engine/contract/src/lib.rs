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

    /// SPEC §18.2 — declare the workspace capabilities this adapter can *bridge*,
    /// honestly. Undeclared means absent. `CapabilitySet` lives in wse-common.
    /// The workspace's effective capabilities are these intersected with the
    /// runtime's (see `runtime`) — the adapter orchestrates, the runtime executes.
    fn capabilities(&self) -> CapabilitySet;

    /// The runtime this adapter runs workspaces on — the immutable, versioned
    /// execution environment *inside* the workspace. Defaults to an unspecified
    /// host runtime that provides nothing; real adapters override it with a
    /// named, versioned image. See contract/core/runtime.md.
    fn runtime(&self) -> RuntimeDescriptor {
        RuntimeDescriptor::host()
    }

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

    // ── capability interfaces ────────────────────────────────────────────────
    // Every capability is its own trait behind a negotiation hook. An adapter
    // exposes the interface for a capability it declares, and None for one it
    // doesn't. The engine negotiates via these hooks, gates on policy, then
    // calls the mechanical interface. Nothing capability-specific is mandatory:
    // a minimal adapter implements only lifecycle + isolation and declares no
    // capabilities.

    /// SPEC §10 — the Applications capability, if this adapter provides it.
    fn applications(&mut self) -> Option<&mut dyn ApplicationsCapability> {
        None
    }

    /// SPEC §14 — the Windows capability, if this adapter provides it.
    fn windows(&mut self) -> Option<&mut dyn WindowsCapability> {
        None
    }

    /// SPEC §9 — the Clipboard capability, if this adapter provides it.
    fn clipboard(&mut self) -> Option<&mut dyn ClipboardCapability> {
        None
    }

    /// SPEC §8 — the Storage capability (workspace persistence), if provided.
    fn storage(&mut self) -> Option<&mut dyn StorageCapability> {
        None
    }

    /// SPEC §12 — the Devices capability (external resources), if provided.
    fn devices(&mut self) -> Option<&mut dyn DevicesCapability> {
        None
    }
}

/// SPEC §10 — the mechanical Applications interface: an application *lifecycle*,
/// not a one-shot launch. The adapter maps a platform-neutral descriptor onto
/// whatever the platform runs (process, container, remote session) and reports
/// it back as an `ApplicationInstance` with a stable id — never a PID. The engine
/// has already checked the descriptor is in the catalog and the workspace runs;
/// the adapter is mechanical (create the instance, tear it down, report state).
pub trait ApplicationsCapability {
    /// Launch a catalog descriptor; return the new running instance. Any windows
    /// it opens are associated on the instance — window *behaviour* stays with
    /// the Windows capability.
    fn app_launch(
        &mut self,
        id: &WorkspaceId,
        app: &ApplicationDescriptor,
    ) -> Result<ApplicationInstance>;

    /// Stop a running instance. After this the instance no longer exists.
    fn app_stop(&mut self, id: &WorkspaceId, instance: &ApplicationInstanceId) -> Result<()>;

    /// The instances currently alive in the workspace (runtime state).
    fn app_instances(&self, id: &WorkspaceId) -> Result<Vec<ApplicationInstance>>;
}

/// SPEC §14 — the mechanical Windows interface: the windows currently open in
/// the workspace (metadata only; the window manager never renders here).
pub trait WindowsCapability {
    fn list_windows(&self, id: &WorkspaceId) -> Result<Vec<Window>>;
}

/// SPEC §9 — the mechanical Clipboard interface. The adapter reads/writes the
/// workspace's OWN clipboard; it never decides who may. Policy (role, access
/// right, direction) lives in the engine. See contract/capabilities/clipboard.md.
pub trait ClipboardCapability {
    /// The workspace clipboard's current content, or None if empty.
    fn clipboard_peek(&self, id: &WorkspaceId) -> Result<Option<ClipboardItem>>;
    /// Replace the workspace clipboard content.
    fn clipboard_put(&mut self, id: &WorkspaceId, data: ClipboardItem) -> Result<()>;
}

/// SPEC §8 — the mechanical Storage interface: persistent workspace-owned
/// resources, no filesystem vocabulary. The adapter stores/retrieves resources;
/// it never decides who may. Policy (FileTransfer right) lives in the engine.
/// See contract/capabilities/storage.md.
pub trait StorageCapability {
    /// Create a resource and mint its stable, immutable id.
    fn resource_create(
        &mut self,
        id: &WorkspaceId,
        name: String,
        kind: ResourceKind,
    ) -> Result<ResourceMetadata>;
    /// Replace a resource's bytes. `NotFound` if the id is unknown or deleted.
    fn resource_write(&mut self, id: &WorkspaceId, resource: &ResourceId, bytes: Vec<u8>)
        -> Result<()>;
    /// Read a resource's bytes. `NotFound` if the id is unknown or deleted.
    fn resource_read(&self, id: &WorkspaceId, resource: &ResourceId) -> Result<Vec<u8>>;
    /// Delete a resource. Returns whether it existed. Deletion is terminal.
    fn resource_delete(&mut self, id: &WorkspaceId, resource: &ResourceId) -> Result<bool>;
    /// Metadata for every resource in the workspace (no bytes).
    fn resource_list(&self, id: &WorkspaceId) -> Result<Vec<ResourceMetadata>>;
}

/// SPEC §12 — the mechanical Devices interface: external host resources made
/// available to a workspace. Discovery, authorization, and usage are separate.
/// The adapter never surfaces host-machine camera/mic (§7.3). Policy (UseDevice
/// right) lives in the engine. See contract/capabilities/devices.md.
pub trait DevicesCapability {
    /// Host makes a device available to the workspace (§12.1/§12.4).
    fn device_attach(
        &mut self,
        id: &WorkspaceId,
        class: DeviceClass,
        name: String,
    ) -> Result<DeviceDescriptor>;
    /// Host withdraws a device. Returns whether it was available.
    fn device_detach(&mut self, id: &WorkspaceId, device: &DeviceId) -> Result<bool>;
    /// Discovery: the devices currently available to the workspace. Non-available
    /// devices never appear (§12.1, §6.5).
    fn device_enumerate(&self, id: &WorkspaceId) -> Result<Vec<DeviceDescriptor>>;
    /// Authorization mechanics: yield a handle for an available device.
    /// `NotFound` if the device is not available.
    fn device_request(&mut self, id: &WorkspaceId, device: &DeviceId) -> Result<DeviceHandle>;
    /// End a granted use. Returns whether a handle was held.
    fn device_release(&mut self, id: &WorkspaceId, device: &DeviceId) -> Result<bool>;
    /// The capability's current contract state for this workspace.
    fn device_state(&self, id: &WorkspaceId) -> Result<CapabilityState>;
}
