//! wse-common — the shared vocabulary of WSE. Pure: no OS, no I/O, no platform
//! deps. Everything above the adapter boundary speaks in these types.

use std::collections::hash_map::DefaultHasher;
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

fn now_nanos() -> u128 {
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
            Clipboard | Storage => Draft,
            Devices | Network | Audio | Camera => Planned,
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
}

// ── members (SPEC §4, §15) ──────────────────────────────────────────────────
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Member {
    pub id: MemberId,
    pub role: Role,
}

// ── applications & windows (SPEC §10, §14) ──────────────────────────────────
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AppSpec {
    pub id: String,
    pub name: String,
}

impl AppSpec {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }
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
    // Applications (§10)
    ApplicationStarted {
        app: String,
        window: WindowId,
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
