//! wse-common — the shared vocabulary of WSE. Pure: no OS, no I/O, no platform
//! deps. Everything above the adapter boundary speaks in these types.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// ── identifiers ─────────────────────────────────────────────────────────────
// SPEC §3.3: identifiers are shown to humans in invitations, so they MUST be
// unguessable and MUST NOT be sequential. This std-only generator is a
// PLACEHOLDER that is merely non-sequential; a real adapter MUST use a CSPRNG.
static COUNTER: AtomicU64 = AtomicU64::new(0);

fn scrambled_128() -> u128 {
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut h1 = DefaultHasher::new();
    (c, t, 0x9e3779b97f4a7c15u64).hash(&mut h1);
    let hi = h1.finish() as u128;
    let mut h2 = DefaultHasher::new();
    (t, c, hi).hash(&mut h2);
    (hi << 64) | (h2.finish() as u128)
}

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, PartialEq, Eq, Hash)]
        pub struct $name(String);
        impl $name {
            pub fn new() -> Self {
                Self(format!("{:032x}", scrambled_128()))
            }
            /// Construct from a stable, meaningful string. For runtime *definitions*
            /// (public, non-secret environment identifiers) this gives a stable id
            /// across process runs — unlike `new()`, which is for unguessable
            /// runtime *instances* (workspaces, resources).
            pub fn from_raw(s: impl Into<String>) -> Self {
                Self(s.into())
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }
    };
}

id_type!(WorkspaceId);
id_type!(WindowId);
id_type!(MemberId);
id_type!(ResourceId);
id_type!(EventId);
id_type!(DeviceId);
id_type!(ApplicationId); // identity of a catalog *definition*
id_type!(ApplicationInstanceId); // identity of a *running instance* — never a PID
id_type!(RuntimeId); // identity of a runtime *definition* (e.g. "wse-linux-x11")

/// Wall-clock nanoseconds since the Unix epoch. Used to stamp events and runtime
/// attestations.
pub fn now_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

// ── lifecycle (SPEC §5) ─────────────────────────────────────────────────────
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WorkspaceState {
    Created,
    Running,
    Idle,
    Paused,
    Resuming,
    Saved,
    Archived,
    Deleted,
}

impl WorkspaceState {
    /// SPEC §5.2. The whole rule: anything not listed is forbidden.
    pub fn can_transition(self, to: WorkspaceState) -> bool {
        use WorkspaceState::*;
        matches!(
            (self, to),
            (Created, Running)
                | (Created, Deleted)
                | (Running, Idle)
                | (Running, Paused)
                | (Running, Saved)
                | (Running, Deleted)
                | (Idle, Running)
                | (Idle, Paused)
                | (Idle, Saved)
                | (Idle, Deleted)
                | (Paused, Resuming)
                | (Paused, Saved)
                | (Paused, Deleted)
                | (Resuming, Running)
                | (Saved, Resuming)
                | (Saved, Archived)
                | (Saved, Deleted)
                | (Archived, Saved)
                | (Archived, Deleted)
        )
    }
}

/// SPEC §5.4. Both first-class; neither is a degraded form of the other.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Persistence {
    Temporary,
    Saved,
}

// ── roles & access rights (SPEC §4) ─────────────────────────────────────────
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Owner,
    Collaborator,
    Observer,
}

/// A per-role access right (SPEC §4.6). Distinct from a workspace *capability*:
/// a capability is what the workspace *provides* (e.g. Clipboard exists); an
/// access right is what a member may *do* with it (e.g. read the clipboard out).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AccessRight {
    ViewDisplay,
    Keyboard,
    Pointer,
    ClipboardRead,
    ClipboardWrite,
    FileTransfer,
    /// Use an external device made available to the workspace (§12.4 consent).
    UseDevice,
}

/// SPEC §4.6. Where unspecified, the answer is no (§6.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Permission {
    Allowed,
    Configurable,
    Denied,
}

// ── capabilities (the workspace-service model) ──────────────────────────────
/// What a workspace *provides*. The engine negotiates on these, never on
/// platform names: it asks "does this workspace provide Clipboard?", never "am
/// I on Windows?". Each capability gets its own mini-spec + conformance suite.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Capability {
    Applications,
    Windows,
    Clipboard,
    Storage,
    Devices,
    Network,
    Audio,
    Camera,
}

impl Capability {
    pub const ALL: [Capability; 8] = [
        Capability::Applications,
        Capability::Windows,
        Capability::Clipboard,
        Capability::Storage,
        Capability::Devices,
        Capability::Network,
        Capability::Audio,
        Capability::Camera,
    ];

    /// How settled this capability's specification is. See
    /// contract/capabilities/README.md.
    pub fn maturity(self) -> CapabilityStatus {
        use Capability::*;
        use CapabilityStatus::*;
        match self {
            Applications | Windows => Stable,
            Clipboard | Storage | Devices => Draft,
            Network | Audio | Camera => Planned,
        }
    }
}

/// Maturity of a capability's specification.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CapabilityStatus {
    /// Shape settled; changes are additive.
    Stable,
    /// Specified and conformance-tested, but the shape may still change.
    Draft,
    /// Named in the model; not yet specified.
    Planned,
}

/// The runtime state of a capability in a workspace — a **contract** state, not
/// a platform state. Adapters translate platform reality into these (a crashed
/// driver -> Degraded, an unplugged device -> Unavailable) and never leak
/// platform terminology. See contract/capabilities/README.md.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CapabilityState {
    Unavailable,
    Available,
    ReadOnly,
    Degraded,
    Offline,
}

/// The set of capabilities an adapter/workspace declares (SPEC §18.2). Declared
/// only — undeclared means absent. Never faked.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CapabilitySet {
    applications: bool,
    windows: bool,
    clipboard: bool,
    storage: bool,
    devices: bool,
    network: bool,
    audio: bool,
    camera: bool,
}

impl CapabilitySet {
    pub fn none() -> Self {
        Self::default()
    }

    /// Builder: declare a capability. `CapabilitySet::none().with(Applications)`.
    pub fn with(mut self, c: Capability) -> Self {
        self.set(c, true);
        self
    }

    pub fn set(&mut self, c: Capability, on: bool) {
        use Capability::*;
        match c {
            Applications => self.applications = on,
            Windows => self.windows = on,
            Clipboard => self.clipboard = on,
            Storage => self.storage = on,
            Devices => self.devices = on,
            Network => self.network = on,
            Audio => self.audio = on,
            Camera => self.camera = on,
        }
    }

    pub fn supports(&self, c: Capability) -> bool {
        use Capability::*;
        match c {
            Applications => self.applications,
            Windows => self.windows,
            Clipboard => self.clipboard,
            Storage => self.storage,
            Devices => self.devices,
            Network => self.network,
            Audio => self.audio,
            Camera => self.camera,
        }
    }

    /// The capabilities actually declared, in a stable order.
    pub fn declared(&self) -> Vec<Capability> {
        Capability::ALL
            .iter()
            .copied()
            .filter(|c| self.supports(*c))
            .collect()
    }

    /// The capabilities present in BOTH sets. A workspace usably provides a
    /// capability only when the adapter can bridge it *and* the runtime provides
    /// it — two different concerns (platform orchestration vs. workspace
    /// execution), both required. See contract/core/runtime.md.
    pub fn intersect(&self, other: &CapabilitySet) -> CapabilitySet {
        let mut out = CapabilitySet::none();
        for c in Capability::ALL {
            out.set(c, self.supports(c) && other.supports(c));
        }
        out
    }
}

// ── runtime (contract/core/runtime.md) ──────────────────────────────────────
// A workspace runs on exactly one Runtime: the versioned, immutable execution
// environment *inside* the workspace (Linux userspace, X server, window manager,
// catalog apps, …). The adapter orchestrates the platform; the runtime provides
// the execution. They are separate contract boundaries — the Applications
// capability never learns whether the runtime is a WSL2 image, an OCI container,
// or a remote host.

/// A runtime's version. `patch` = same behaviour, `minor` = additive, `major` =
/// breaking. A runtime image is immutable: a change means a NEW version, never a
/// mutation of the existing one (that is what keeps conformance repeatable).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RuntimeVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl RuntimeVersion {
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self { major, minor, patch }
    }
}

impl fmt::Display for RuntimeVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// An immutable, versioned execution environment a workspace runs on. Mirrors the
/// capability model deliberately: it, too, is a contract boundary that declares
/// what it supports. `capabilities` are what the *runtime* provides inside the
/// workspace — negotiated separately from what the adapter can bridge.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RuntimeDescriptor {
    pub id: RuntimeId,
    pub name: String,
    pub version: RuntimeVersion,
    /// The immutable base the image is built from (e.g. "alpine-3.19"); part of
    /// the identity of *what* is executing.
    pub base: String,
    /// The immutable identifier of the exact image (a content digest, or any
    /// stable equivalent). Changing the image content changes this.
    pub digest: String,
    pub capabilities: CapabilitySet,
    pub metadata: HashMap<String, String>,
}

impl RuntimeDescriptor {
    /// The default when an adapter declares no runtime of its own: an
    /// unspecified host environment that provides nothing inside the workspace.
    /// Real adapters override `runtime()` with a named, versioned image.
    pub fn host() -> Self {
        Self {
            id: RuntimeId::new(),
            name: "host".into(),
            version: RuntimeVersion::new(0, 0, 0),
            base: "host".into(),
            digest: "none".into(),
            capabilities: CapabilitySet::none(),
            metadata: HashMap::new(),
        }
    }

    /// The attestation for this runtime, stamped with a start time. The engine
    /// records this when the workspace starts.
    pub fn attest(&self, at: u128) -> RuntimeAttestation {
        RuntimeAttestation {
            runtime: self.id.clone(),
            name: self.name.clone(),
            version: self.version,
            digest: self.digest.clone(),
            capabilities: self.capabilities.clone(),
            at,
        }
    }
}

/// Recorded when a workspace starts: exactly which runtime executed it, pinned to
/// an immutable `digest`. This is what makes every run and every bug report
/// reproducible — "Applications fail on wse-linux-x11 v1.3.2 (sha256:…)" names the
/// precise environment. The engine stamps `at`; the adapter supplies the rest.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RuntimeAttestation {
    pub runtime: RuntimeId,
    pub name: String,
    pub version: RuntimeVersion,
    /// The immutable identifier of the exact image that ran (a content digest, or
    /// any stable equivalent the platform can produce).
    pub digest: String,
    pub capabilities: CapabilitySet,
    pub at: u128,
}

// ── members (SPEC §4, §15) ──────────────────────────────────────────────────
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Member {
    pub id: MemberId,
    pub role: Role,
}

// ── applications & windows (SPEC §10, §14) ──────────────────────────────────
// The Applications capability is a *lifecycle*, not a launch call. It follows
// the same identity/handle split as Storage (resource≠handle) and Devices
// (descriptor≠handle):
//
//     ApplicationDescriptor  →  launch  →  ApplicationInstance
//        (immutable defn)                    (runtime state)
//
// A descriptor is a host-curated catalog definition. An instance is a running
// thing with its own stable ApplicationInstanceId — which is NOT a PID. The
// adapter may map that id onto a PID, a container, or a remote session; that
// mapping never escapes into the contract.

/// An immutable application definition in a workspace's catalog. `entry` is the
/// platform-neutral launch key ("browser"); the adapter maps it to whatever the
/// platform needs (an exe, a container, a remote process).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ApplicationDescriptor {
    pub id: ApplicationId,
    pub entry: String,
    pub name: String,
    pub metadata: HashMap<String, String>,
}

impl ApplicationDescriptor {
    pub fn new(entry: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: ApplicationId::new(),
            entry: entry.into(),
            name: name.into(),
            metadata: HashMap::new(),
        }
    }
}

/// The contract lifecycle of a running application. Platforms map their own
/// process/job/session states into these; the contract never exposes the
/// platform's own state names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ApplicationState {
    Declared,
    Launching,
    Running,
    Suspended,
    Resuming,
    Stopping,
    Stopped,
}

/// A running application: runtime state, not a definition. Its `id` is stable and
/// unguessable and is never a PID. It *owns* zero or more windows — Applications
/// establishes the association; the Windows capability owns window behaviour.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ApplicationInstance {
    pub id: ApplicationInstanceId,
    pub application: ApplicationId,
    pub state: ApplicationState,
    pub windows: Vec<WindowId>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

/// SPEC §14: the Window Manager owns metadata, never rendering.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Window {
    pub id: WindowId,
    pub app: String,
    pub title: String,
    pub bounds: Bounds,
    pub focused: bool,
}

// ── clipboard (SPEC §9) ─────────────────────────────────────────────────────
/// A single clipboard item: a MIME/content-type and its bytes. Formats are
/// DATA, not capabilities — `text/plain`, `text/html`, `image/png`,
/// `application/json` all flow through one model, so the contract needn't change
/// per format. `text/plain` is the baseline every implementation carries.
/// See contract/capabilities/clipboard.md.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ClipboardItem {
    pub mime: String,
    pub payload: Vec<u8>,
}

impl ClipboardItem {
    pub fn new(mime: impl Into<String>, payload: impl Into<Vec<u8>>) -> Self {
        Self {
            mime: mime.into(),
            payload: payload.into(),
        }
    }
    /// Convenience for the baseline format.
    pub fn text(s: impl Into<String>) -> Self {
        Self::new("text/plain", s.into().into_bytes())
    }
    pub fn is_text(&self) -> bool {
        self.mime.starts_with("text/")
    }
    /// The payload as UTF-8 text, if it is a text item and valid UTF-8.
    pub fn as_text(&self) -> Option<&str> {
        if self.is_text() {
            std::str::from_utf8(&self.payload).ok()
        } else {
            None
        }
    }
}

// ── storage / workspace persistence (SPEC §8) ───────────────────────────────
// The Storage capability owns RESOURCES, not files. No File/Folder/Path/Drive
// vocabulary. See contract/capabilities/storage.md.

/// The kind of a persistent resource. Deliberately minimal; a `Collection` kind
/// (with children) is a future addition, not a v0.1 concern.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ResourceKind {
    Blob,
}

/// Contract metadata for a resource — no platform fields. The `ResourceId` is
/// the stable, immutable identity (universal rule: every persistent object the
/// engine exposes has a stable contract identity).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ResourceMetadata {
    pub id: ResourceId,
    pub name: String,
    pub kind: ResourceKind,
    pub size: u64,
}

// ── devices / external resources (SPEC §7.2/§7.3, §12) ──────────────────────
// The Devices capability represents host-provided resources external to the
// workspace. Device classes are DATA, not capabilities. Discovery (descriptor),
// authorization (handle), and usage are separate. See
// contract/capabilities/devices.md.

/// A class of external device. Data, not a capability — a new class never grows
/// the contract.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeviceClass {
    Camera,
    Microphone,
    Speaker,
    Display,
    Printer,
    Usb,
    Gpu,
    Bluetooth,
    Nfc,
}

/// An immutable description of a device. NOT a permission, NOT a session, NOT a
/// handle — those are separate concepts.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DeviceDescriptor {
    pub id: DeviceId,
    pub class: DeviceClass,
    pub name: String,
    pub metadata: HashMap<String, String>,
}

/// The result of an authorized `request` — the right to use a device. Separate
/// from the descriptor (which merely describes) and from any session.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DeviceHandle {
    pub device: DeviceId,
}

// ── resource limits (SPEC §7) ───────────────────────────────────────────────
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ResourceLimits {
    pub cpu_cores: Option<u32>,
    pub memory_gb: Option<u32>,
    pub storage_gb: Option<u32>,
}

// ── events (core, not a capability) ─────────────────────────────────────────
// Events are the language the whole engine speaks: every capability emits them,
// every adapter forwards them, every SDK subscribes, every audit log records
// them. The ENVELOPE is defined by the core contract; the KIND is capability-
// defined but drawn from this closed set — so, exactly like errors, an adapter
// can never *invent* an event, only populate one. See contract/core/events.md.

/// Who caused an event.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Actor {
    /// The engine/host itself (lifecycle, policy).
    System,
    /// A member acting in a role. A MemberId that resolves to a role arrives
    /// with collaboration; the envelope is ready for it.
    Member(Role),
}

/// Which part of the contract an event came from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EventSource {
    /// Core lifecycle (workspace create/state/destroy).
    Core,
    /// A specific capability.
    Capability(Capability),
}

/// The type of an event. Grouped by source; each capability owns its variants,
/// but the set is closed so nothing can be invented outside the contract.
/// Payloads carry identity/metadata only — never content or bytes (audit vs.
/// privacy, SPEC §17.1).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EventKind {
    // Core lifecycle
    WorkspaceCreated,
    StateChanged {
        from: WorkspaceState,
        to: WorkspaceState,
    },
    WorkspaceDestroyed,
    // Applications (§10) — the lifecycle, through the one envelope. Payloads
    // carry the instance identity and the catalog entry, never a PID or bytes.
    ApplicationLaunchRequested {
        app: String,
    },
    ApplicationStarted {
        instance: ApplicationInstanceId,
        app: String,
    },
    ApplicationStopping {
        instance: ApplicationInstanceId,
    },
    ApplicationStopped {
        instance: ApplicationInstanceId,
    },
    // Windows (§14)
    WindowOpened {
        window: WindowId,
    },
    WindowFocused {
        window: WindowId,
    },
    WindowClosed {
        window: WindowId,
    },
    // Clipboard (§9) — who + direction, never content
    ClipboardRead,
    ClipboardWritten,
    // Storage (§8) — who + which resource, never bytes
    ResourceCreated {
        resource: ResourceId,
    },
    ResourceModified {
        resource: ResourceId,
    },
    ResourceRead {
        resource: ResourceId,
    },
    ResourceDeleted {
        resource: ResourceId,
    },
    // Devices (§12) — attach/detach/request/release, all metadata only
    DeviceAttached {
        device: DeviceId,
    },
    DeviceDetached {
        device: DeviceId,
    },
    DeviceRequested {
        device: DeviceId,
    },
    DeviceReleased {
        device: DeviceId,
    },
    /// A capability's contract state changed (e.g. Devices Available -> Degraded).
    CapabilityStateChanged {
        capability: Capability,
        from: CapabilityState,
        to: CapabilityState,
    },
}

/// A workspace event. The envelope is fixed by the core contract; adapters and
/// capabilities populate it, never redefine it.
///
/// Invariants (see contract/core/events.md): events are immutable and
/// append-only; `seq` is a per-workspace monotonically increasing ordering
/// authority; payloads never expose data the contract forbids.
#[derive(Clone, Debug)]
pub struct Event {
    pub id: EventId,
    pub workspace: WorkspaceId,
    /// Per-workspace monotonic sequence — the ordering authority.
    pub seq: u64,
    /// Wall-clock nanoseconds, best-effort/informational (ordering uses `seq`).
    pub at: u128,
    pub actor: Actor,
    pub source: EventSource,
    pub kind: EventKind,
}

impl Event {
    pub fn new(
        workspace: WorkspaceId,
        seq: u64,
        actor: Actor,
        source: EventSource,
        kind: EventKind,
    ) -> Self {
        Self {
            id: EventId::new(),
            workspace,
            seq,
            at: now_nanos(),
            actor,
            source,
            kind,
        }
    }
}

// ── the error contract ──────────────────────────────────────────────────────
/// The contract's error vocabulary. **Adapters never invent error kinds** —
/// they map platform-specific failures into these. This is what lets multiple
/// adapters and SDKs share one error model.
#[derive(Debug, PartialEq, Eq)]
pub enum WseError {
    /// SPEC §6.5 — a non-granted resource is reported as *not found*, never as
    /// "denied". A workspace must not distinguish "does not exist" from "exists
    /// but not granted". Probing must not reveal existence.
    NotFound(String),
    /// SPEC §5.2 — the state machine forbids this transition.
    InvalidTransition {
        from: WorkspaceState,
        to: WorkspaceState,
    },
    /// The operation requires a different state than the workspace is in.
    InvalidState {
        operation: &'static str,
        state: WorkspaceState,
    },
    /// SPEC §18.2 — the workspace does not provide this capability. Declared-only.
    CapabilityUnavailable(Capability),
    /// SPEC §4.3 / §4.6 — a role is refused an access right. Unlike NotFound,
    /// this is a *visible* refusal: the resource's existence is not a secret.
    PermissionDenied {
        right: AccessRight,
        role: Role,
    },
    /// SPEC §18.4 — the adapter speaks an incompatible contract version.
    ContractMismatch {
        adapter: String,
        engine: String,
    },
    /// SPEC §7 — a resource limit or dependency is unavailable.
    ResourceUnavailable(String),
    /// SPEC §18.3 — the engine rejected the adapter's isolation attestation.
    IsolationRejected {
        workspace: WorkspaceId,
        details: Vec<String>,
    },
    /// A platform/adapter failure mapped into the contract. Adapters map their
    /// native failures to this (or a more specific kind) — never a new kind.
    Internal(String),
}

impl fmt::Display for WseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use WseError::*;
        match self {
            NotFound(what) => write!(f, "not found: {what}"),
            InvalidTransition { from, to } => write!(f, "illegal transition {from:?} -> {to:?}"),
            InvalidState { operation, state } => {
                write!(f, "cannot {operation}: workspace is {state:?}")
            }
            CapabilityUnavailable(c) => write!(f, "capability unavailable: {c:?}"),
            PermissionDenied { right, role } => {
                write!(f, "permission denied: {role:?} may not {right:?}")
            }
            ContractMismatch { adapter, engine } => {
                write!(f, "contract mismatch: adapter {adapter}, engine {engine}")
            }
            ResourceUnavailable(what) => write!(f, "resource unavailable: {what}"),
            IsolationRejected { workspace, details } => {
                write!(f, "isolation rejected for {workspace}: {}", details.join("; "))
            }
            Internal(msg) => write!(f, "internal: {msg}"),
        }
    }
}

impl std::error::Error for WseError {}

pub type Result<T> = std::result::Result<T, WseError>;
