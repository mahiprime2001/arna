//! wse-adapter-windows-native — the NATIVE Windows adapter. No WSL, no VM.
//!
//! A workspace is a **separate Windows desktop** plus an **isolated per-app
//! profile**. Native apps run on that desktop, invisible to the owner's real
//! screen; the owner keeps working. This is honest about what it isolates: input
//! and presentation are separated and per-app storage is isolated, but the host
//! filesystem is SHARED. So it attests the `DesktopProfile` isolation model, not
//! `SealedVm` (contract/core/isolation.md).
//!
//! Platform code lives ONLY here (adapter Rule 3). It calls Win32
//! (user32/kernel32) via direct FFI — no external crates.
//!
//! This is the minimal, truthful first milestone: real lifecycle + a real
//! isolation attestation, declaring NO capabilities yet. It therefore passes
//! `run_core` and honestly reports every capability unavailable. Applications,
//! Windows, and Clipboard (the isolatable-app catalog) are added incrementally.

use std::collections::HashMap;
use std::ffi::c_void;
use std::path::PathBuf;

use wse_common::*;
use wse_contract::{
    ContractVersion, IsolationAttestation, WorkspaceAdapter, WorkspaceDef, CONTRACT_VERSION,
};

// ── Win32 FFI (kept entirely inside this crate) ──────────────────────────────
type Hdesk = *mut c_void;
const GENERIC_ALL: u32 = 0x1000_0000;

#[link(name = "user32")]
extern "system" {
    fn CreateDesktopW(
        name: *const u16,
        device: *const u16,
        devmode: *const c_void,
        flags: u32,
        access: u32,
        sa: *const c_void,
    ) -> Hdesk;
    fn CloseDesktop(h: Hdesk) -> i32;
}

/// A NUL-terminated UTF-16 string for the Win32 `W` APIs.
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Desktop names are visible object names; keep them prefixed + unambiguous so a
/// user's own desktops are never touched.
fn desktop_name(id: &WorkspaceId) -> String {
    format!("wse-{id}")
}

// ── per-workspace state ──────────────────────────────────────────────────────
struct WsState {
    /// HDESK stored as isize so the adapter needs no unsafe Send/Sync games.
    desktop: isize,
    desktop_name: String,
    profile_dir: PathBuf,
}

pub struct WindowsNativeAdapter {
    data_dir: PathBuf,
    state: HashMap<WorkspaceId, WsState>,
}

impl Default for WindowsNativeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsNativeAdapter {
    pub fn new() -> Self {
        let data_dir = std::env::temp_dir().join("arna-native-workspaces");
        let _ = std::fs::create_dir_all(&data_dir);
        Self {
            data_dir,
            state: HashMap::new(),
        }
    }
}

impl WorkspaceAdapter for WindowsNativeAdapter {
    fn contract_version(&self) -> ContractVersion {
        CONTRACT_VERSION
    }

    fn capabilities(&self) -> CapabilitySet {
        // Minimal + truthful. Applications/Windows/Clipboard are added as the
        // isolatable-app catalog is wired; effective = adapter ∩ runtime.
        CapabilitySet::none()
    }

    fn runtime(&self) -> RuntimeDescriptor {
        // The runtime is the host itself: native Windows, apps on a separate
        // desktop. There is no separable image, so `base`/`digest` describe the
        // host environment. Capabilities grow as the catalog is added.
        RuntimeDescriptor {
            id: RuntimeId::from_raw("windows-native"),
            name: "windows-native".into(),
            version: RuntimeVersion::new(0, 1, 0),
            base: "windows-host".into(),
            digest: "windows-native-host".into(),
            capabilities: CapabilitySet::none(),
            metadata: HashMap::new(),
        }
    }

    fn create(&mut self, def: &WorkspaceDef) -> Result<()> {
        let name = desktop_name(&def.id);
        // A separate desktop = the workspace's own input/display space.
        let hd = unsafe {
            CreateDesktopW(
                wide(&name).as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                GENERIC_ALL,
                std::ptr::null(),
            )
        };
        if hd.is_null() {
            return Err(WseError::Internal(format!(
                "CreateDesktop('{name}') failed"
            )));
        }
        // An isolated per-workspace profile directory (apps get their own state).
        let profile_dir = self.data_dir.join(&name);
        std::fs::create_dir_all(&profile_dir)
            .map_err(|e| WseError::Internal(format!("profile dir: {e}")))?;

        self.state.insert(
            def.id.clone(),
            WsState {
                desktop: hd as isize,
                desktop_name: name,
                profile_dir,
            },
        );
        Ok(())
    }

    fn start(&mut self, id: &WorkspaceId) -> Result<IsolationAttestation> {
        let st = self
            .state
            .get(id)
            .ok_or_else(|| WseError::NotFound(format!("workspace {id}")))?;
        // Honest attestation of the DesktopProfile model: what it separates, and
        // what it explicitly does NOT seal. The engine's policy decides if this
        // model is acceptable for the deployment.
        Ok(IsolationAttestation {
            model: IsolationModel::DesktopProfile,
            isolated: true,
            details: vec![
                format!("separate desktop '{}' (own input + display)", st.desktop_name),
                format!("isolated profile at {}", st.profile_dir.display()),
                "shares the host filesystem — not a sealed VM".into(),
            ],
        })
    }

    fn stop(&mut self, _id: &WorkspaceId) -> Result<()> {
        // Suspend: the workspace record + desktop survive. Nothing to tear down
        // until apps run on it.
        Ok(())
    }

    fn destroy(&mut self, id: &WorkspaceId) -> Result<()> {
        let st = self
            .state
            .remove(id)
            .ok_or_else(|| WseError::NotFound(format!("workspace {id}")))?;
        // No apps in the core-only milestone, so the desktop closes cleanly.
        unsafe {
            CloseDesktop(st.desktop as Hdesk);
        }
        let _ = std::fs::remove_dir_all(&st.profile_dir);
        Ok(())
    }

    // No capability hooks yet -> every capability is unavailable, truthfully.
}
