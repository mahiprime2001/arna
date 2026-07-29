//! wse-adapter-windows — the first independent implementation of the Workspace
//! Contract. It is a **translation layer**: platform (WSL2 / wsl.exe) -> contract
//! -> engine, never the reverse (adapter Rule 4). All Windows/WSL concepts stay
//! inside this crate (Rule 3).
//!
//! v1 milestone: a **minimal, truthful** adapter — real lifecycle + a real
//! isolation attestation, declaring NO capabilities. It therefore passes
//! `run_core` and honestly reports every capability as unavailable. Capabilities
//! (Applications, Windows, Clipboard, Storage, Devices) are added incrementally,
//! each enabling its own conformance suite, without changing the contract.
//!
//! The mechanics (create/harden/verify/destroy via WSL2) are the proven approach
//! from the Go reference in `services/wsl`, ported as a consumer of the contract.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use wse_common::*;
use wse_contract::{
    ApplicationsCapability, ContractVersion, IsolationAttestation, WindowsCapability,
    WorkspaceAdapter, WorkspaceDef, CONTRACT_VERSION,
};

/// The immutable runtime image this adapter imports (built by
/// `runtimes/wse-linux-x11/build.sh`). Pinned by version + digest; a change is a
/// new version, never an in-place edit (contract/core/runtime.md).
const RUNTIME_VERSION: RuntimeVersion = RuntimeVersion { major: 1, minor: 0, patch: 0 };
const RUNTIME_DIGEST: &str =
    "sha256:34be3169d4fc23e99be659e2b6224b401a19818b42d61a2053548ef733ee7dc4";
const RUNTIME_IMAGE_FILE: &str = "wse-linux-x11-v1.0.0.tar";

/// Every distro this adapter owns is prefixed, so a user's own WSL installs are
/// never touched.
const PREFIX: &str = "arna-ws-";

fn distro_name(id: &WorkspaceId) -> String {
    format!("{PREFIX}{id}")
}

/// Per-workspace runtime state the adapter tracks. Platform ids (X window ids)
/// live here and never escape: the engine sees only contract ids.
#[derive(Default)]
struct WsState {
    instances: Vec<ApplicationInstance>,
    windows: Vec<XWindow>,
}

/// A window the adapter opened: its contract `id` mapped to the private X id.
struct XWindow {
    id: WindowId,
    xid: String,
    app: String,   // the catalog entry it came from
    title: String, // the contract title (the descriptor's name)
}

/// The isolation config written into every workspace: automount off (no host
/// filesystem) and interop off (cannot launch host .exe). See ADR-006.
const WSL_CONF: &str = "[automount]\nenabled = false\nmountFsTab = false\n\n[interop]\nenabled = false\nappendWindowsPath = false\n\n[network]\ngenerateResolvConf = true\n";

pub struct WindowsAdapter {
    data_dir: PathBuf,
    image: PathBuf,
    state: HashMap<WorkspaceId, WsState>,
}

impl Default for WindowsAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsAdapter {
    pub fn new() -> Self {
        let data_dir = std::env::temp_dir().join("arna-workspaces");
        let _ = std::fs::create_dir_all(&data_dir);
        // The runtime image: explicit via WSE_RUNTIME_IMAGE, else alongside the
        // workspace data dir. Resolved here; required at create().
        let image = std::env::var("WSE_RUNTIME_IMAGE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| data_dir.join(RUNTIME_IMAGE_FILE));
        Self {
            data_dir,
            image,
            state: HashMap::new(),
        }
    }

    // ── wsl.exe plumbing ─────────────────────────────────────────────────────
    /// Run wsl.exe with args; return decoded stdout or a mapped contract error.
    fn wsl(&self, args: &[&str]) -> Result<String> {
        let out = Command::new("wsl.exe")
            .args(args)
            .output()
            .map_err(|e| WseError::Internal(format!("wsl.exe not runnable: {e}")))?;
        let stdout = decode_wsl(&out.stdout);
        if out.status.success() {
            Ok(stdout)
        } else {
            // Adapters map platform failures into the contract vocabulary.
            let stderr = decode_wsl(&out.stderr);
            Err(WseError::Internal(format!(
                "wsl {}: {}",
                args.join(" "),
                stderr.trim()
            )))
        }
    }

    /// Run a command inside a workspace as root; return decoded stdout.
    fn inside(&self, id: &WorkspaceId, script: &str) -> Result<String> {
        self.wsl(&[
            "-d",
            &distro_name(id),
            "--user",
            "root",
            "--",
            "sh",
            "-c",
            script,
        ])
    }

    /// The runtime image to import. Required — the runtime *is* the image.
    fn runtime_image(&self) -> Result<&PathBuf> {
        if self.image.exists() {
            Ok(&self.image)
        } else {
            Err(WseError::ResourceUnavailable(format!(
                "runtime image not found at {} — build it with runtimes/wse-linux-x11/build.sh \
                 or set WSE_RUNTIME_IMAGE",
                self.image.display()
            )))
        }
    }

    /// Write the isolation config and terminate so it takes effect on next start.
    fn harden(&self, id: &WorkspaceId) -> Result<()> {
        // base64 the config in, to survive the Windows -> wsl.exe -> sh layers.
        let enc = base64_encode(WSL_CONF.as_bytes());
        self.inside(
            id,
            &format!("echo {enc} | base64 -d > /etc/wsl.conf && chmod 0644 /etc/wsl.conf"),
        )?;
        self.wsl(&["--terminate", &distro_name(id)])?;
        Ok(())
    }

    /// Actively verify the seal (adapter Rule: attest with evidence, not trust).
    /// No host drive is mounted, and host executables cannot be launched.
    fn verify_isolation(&self, id: &WorkspaceId) -> IsolationAttestation {
        let mut details = Vec::new();
        let mut sealed = true;

        // /proc/mounts is authoritative: a host drive appears as a mount at
        // /mnt/<letter>. Empty /mnt/c directories left by WSL are not mounts.
        match self.inside(id, "cat /proc/mounts") {
            Ok(mounts) => {
                let host = mounts.lines().any(|l| {
                    l.split_whitespace()
                        .nth(1)
                        .map(is_drive_mount)
                        .unwrap_or(false)
                });
                if host {
                    sealed = false;
                    details.push("a host drive is mounted".into());
                } else {
                    details.push("no host drives are mounted".into());
                }
            }
            Err(e) => {
                sealed = false;
                details.push(format!("could not read mounts: {e}"));
            }
        }

        // interop off: launching a host .exe must fail.
        match self.inside(
            id,
            "/mnt/c/Windows/System32/cmd.exe /c ver >/dev/null 2>&1 && echo LEAK || echo OK",
        ) {
            Ok(out) if out.contains("OK") => {
                details.push("host executables cannot be launched".into())
            }
            _ => {
                sealed = false;
                details.push("host executable launch was NOT blocked".into());
            }
        }

        IsolationAttestation { sealed, details }
    }
}

impl WorkspaceAdapter for WindowsAdapter {
    fn contract_version(&self) -> ContractVersion {
        CONTRACT_VERSION
    }

    fn capabilities(&self) -> CapabilitySet {
        // What this adapter can *bridge*. The effective set a workspace provides
        // is this ∩ runtime.capabilities — both the adapter and the runtime must
        // agree (contract/core/runtime.md).
        CapabilitySet::none()
            .with(Capability::Applications)
            .with(Capability::Windows)
    }

    fn runtime(&self) -> RuntimeDescriptor {
        // wse-linux-x11 v1.0.0: the immutable WSL2 Linux image this adapter
        // imports — Xvfb + openbox + xterm + xdotool + fonts + a catalog launcher.
        // It PROVIDES Applications + Windows inside the workspace; the adapter
        // BRIDGES them. See runtimes/wse-linux-x11/ and contract/core/runtime.md.
        RuntimeDescriptor {
            id: RuntimeId::from_raw("wse-linux-x11"),
            name: "wse-linux-x11".into(),
            version: RUNTIME_VERSION,
            base: "alpine-3.20".into(),
            digest: RUNTIME_DIGEST.into(),
            capabilities: CapabilitySet::none()
                .with(Capability::Applications)
                .with(Capability::Windows),
            metadata: std::collections::HashMap::new(),
        }
    }

    fn create(&mut self, def: &WorkspaceDef) -> Result<()> {
        let name = distro_name(&def.id);
        let image = self.runtime_image()?.clone();
        let inst = self.data_dir.join(&name);
        std::fs::create_dir_all(&inst)
            .map_err(|e| WseError::Internal(format!("mkdir: {e}")))?;
        self.wsl(&[
            "--import",
            &name,
            inst.to_string_lossy().as_ref(),
            image.to_string_lossy().as_ref(),
            "--version",
            "2",
        ])?;
        self.harden(&def.id)?;
        Ok(())
    }

    fn start(&mut self, id: &WorkspaceId) -> Result<IsolationAttestation> {
        // A trivial command starts the distro fresh (re-reading wsl.conf).
        self.inside(id, "true")?;
        Ok(self.verify_isolation(id))
    }

    fn stop(&mut self, id: &WorkspaceId) -> Result<()> {
        self.wsl(&["--terminate", &distro_name(id)]).map(|_| ())
    }

    fn destroy(&mut self, id: &WorkspaceId) -> Result<()> {
        let name = distro_name(id);
        let _ = self.wsl(&["--terminate", &name]);
        self.wsl(&["--unregister", &name])?;
        let _ = std::fs::remove_dir_all(self.data_dir.join(&name));
        // All in-workspace processes (Xvfb, openbox, apps) die with the distro;
        // drop the tracked state too — no orphans (criterion #6).
        self.state.remove(id);
        Ok(())
    }

    fn applications(&mut self) -> Option<&mut dyn ApplicationsCapability> {
        Some(self)
    }

    fn windows(&mut self) -> Option<&mut dyn WindowsCapability> {
        Some(self)
    }
}

// ── Applications capability: map a catalog entry onto the runtime's launcher ──
// The adapter carries NO app knowledge — the runtime's /opt/wse/launch.sh maps
// entry -> command. The adapter mints the contract ids, tracks instances/windows,
// and translates the runtime's X window ids into contract WindowIds (which never
// leak upward). Lifecycle *events* are the engine's; the adapter only does the
// mechanics — so a real app produces the same contract events as the mock.
impl ApplicationsCapability for WindowsAdapter {
    fn app_launch(
        &mut self,
        id: &WorkspaceId,
        app: &ApplicationDescriptor,
    ) -> Result<ApplicationInstance> {
        // The runtime provides the display; the adapter only asks it to come up.
        self.inside(id, "/opt/wse/start-display.sh")?;

        let iid = ApplicationInstanceId::new();
        let marker = format!("wse-{iid}");
        // The runtime launches the app and returns its X window id. Deterministic
        // focus (newest) is the launcher's job.
        let out = self.inside(
            id,
            &format!("/opt/wse/launch.sh {} {}", app.entry, marker),
        )?;
        let xid = out.split_whitespace().last().unwrap_or("").to_string();
        if xid.is_empty() || !xid.chars().all(|c| c.is_ascii_digit()) {
            return Err(WseError::Internal(format!(
                "runtime launcher returned no window id (out: {out:?})"
            )));
        }

        let wid = WindowId::new();
        let st = self.state.entry(id.clone()).or_default();
        st.windows.push(XWindow {
            id: wid.clone(),
            xid,
            app: app.entry.clone(),
            title: app.name.clone(),
        });
        let instance = ApplicationInstance {
            id: iid,
            application: app.id.clone(),
            state: ApplicationState::Running,
            windows: vec![wid],
        };
        st.instances.push(instance.clone());
        Ok(instance)
    }

    fn app_stop(&mut self, id: &WorkspaceId, instance: &ApplicationInstanceId) -> Result<()> {
        let st = self
            .state
            .get_mut(id)
            .ok_or_else(|| WseError::NotFound(format!("workspace {id}")))?;
        let pos = st
            .instances
            .iter()
            .position(|i| &i.id == instance)
            .ok_or_else(|| WseError::NotFound(format!("instance {instance}")))?;
        let inst = st.instances.remove(pos);
        // The instance's windows close with it.
        let mut xids = Vec::new();
        st.windows.retain(|w| {
            if inst.windows.contains(&w.id) {
                xids.push(w.xid.clone());
                false
            } else {
                true
            }
        });
        for xid in xids {
            let _ = self.inside(id, &format!("DISPLAY=:0 xdotool windowkill {xid}"));
        }
        Ok(())
    }

    fn app_instances(&self, id: &WorkspaceId) -> Result<Vec<ApplicationInstance>> {
        Ok(self
            .state
            .get(id)
            .map(|s| s.instances.clone())
            .unwrap_or_default())
    }
}

// ── Windows capability: list the workspace's windows (metadata only) ─────────
impl WindowsCapability for WindowsAdapter {
    fn list_windows(&self, id: &WorkspaceId) -> Result<Vec<Window>> {
        // Focus is a contract concept the adapter maintains, exactly as the
        // reference does: the most recently launched window is focused, at most
        // one. The runtime already makes this physically true (the launcher does
        // `windowactivate --sync` on the newest), so the model matches reality
        // without a racy live `getactivewindow` query.
        let windows = match self.state.get(id) {
            Some(st) => &st.windows,
            None => return Ok(Vec::new()),
        };
        let last = windows.len().saturating_sub(1);
        Ok(windows
            .iter()
            .enumerate()
            .map(|(i, w)| Window {
                id: w.id.clone(),
                app: w.app.clone(),
                title: w.title.clone(),
                bounds: Bounds::default(),
                focused: i == last,
            })
            .collect())
    }
}

// ── helpers (platform-facing; never leak above the boundary) ─────────────────

/// wsl.exe emits UTF-16LE for management output. Decode it (or pass UTF-8 through).
fn decode_wsl(bytes: &[u8]) -> String {
    let b = if bytes.starts_with(&[0xFF, 0xFE]) {
        &bytes[2..]
    } else {
        bytes
    };
    // Heuristic: ASCII-as-UTF-16LE has a NUL in every high byte.
    let sample = b.iter().skip(1).step_by(2).take(16);
    let zeros = sample.clone().filter(|&&x| x == 0).count();
    let checked = sample.count();
    if checked > 0 && zeros * 2 >= checked {
        let u: Vec<u16> = b
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&u)
    } else {
        String::from_utf8_lossy(b).into_owned()
    }
}

/// Is a mountpoint a host drive root (`/mnt/<single-letter>`)? WSL's /mnt/wsl,
/// /mnt/wslg are multi-char and correctly excluded.
fn is_drive_mount(mp: &str) -> bool {
    if let Some(rest) = mp.strip_prefix("/mnt/") {
        rest.len() == 1 && rest.as_bytes()[0].is_ascii_lowercase()
    } else {
        false
    }
}

/// Minimal base64 (no dependency) for shipping the wsl.conf through shell layers.
fn base64_encode(input: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | (b[2] as u32);
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            T[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}
