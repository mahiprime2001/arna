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

// ── roles & capabilities (SPEC §4) ──────────────────────────────────────────
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Owner,
    Collaborator,
    Observer,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Capability {
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

// ── resources (SPEC §7) ─────────────────────────────────────────────────────
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ResourceLimits {
    pub cpu_cores: Option<u32>,
    pub memory_gb: Option<u32>,
    pub storage_gb: Option<u32>,
}

// ── errors ──────────────────────────────────────────────────────────────────
#[derive(Debug, PartialEq, Eq)]
pub enum WseError {
    /// SPEC §5.2 — an illegal state transition was requested.
    InvalidTransition {
        from: WorkspaceState,
        to: WorkspaceState,
    },
    /// SPEC §6.5 — a non-granted resource is reported as *not found*, never as
    /// "denied". A workspace must not distinguish "does not exist" from
    /// "exists but not granted".
    NotFound(String),
    /// The workspace is not in a state where this operation is valid.
    NotRunning(WorkspaceId),
    /// SPEC §18.3 — the adapter could not prove the workspace is sealed, so the
    /// engine refuses to run it. There is no partial-isolation tier.
    NotIsolated {
        workspace: WorkspaceId,
        details: Vec<String>,
    },
    /// The adapter failed for a platform-specific reason.
    Adapter(String),
}

impl fmt::Display for WseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WseError::InvalidTransition { from, to } => {
                write!(f, "illegal transition {from:?} -> {to:?}")
            }
            WseError::NotFound(what) => write!(f, "not found: {what}"),
            WseError::NotRunning(id) => write!(f, "workspace {id} is not running"),
            WseError::NotIsolated { workspace, details } => {
                write!(f, "workspace {workspace} is not sealed: {}", details.join("; "))
            }
            WseError::Adapter(msg) => write!(f, "adapter error: {msg}"),
        }
    }
}

impl std::error::Error for WseError {}

pub type Result<T> = std::result::Result<T, WseError>;
