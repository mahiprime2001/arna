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

use std::path::PathBuf;
use std::process::Command;

use wse_common::*;
use wse_contract::{
    ContractVersion, IsolationAttestation, WorkspaceAdapter, WorkspaceDef, CONTRACT_VERSION,
};

/// A tiny Alpine rootfs, fetched once. This is a v1 stand-in to make the pipeline
/// real; a purpose-built image is a later refinement behind this same adapter.
const ALPINE_URL: &str =
    "https://dl-cdn.alpinelinux.org/alpine/v3.20/releases/x86_64/alpine-minirootfs-3.20.3-x86_64.tar.gz";

/// Every distro this adapter owns is prefixed, so a user's own WSL installs are
/// never touched.
const PREFIX: &str = "arna-ws-";

fn distro_name(id: &WorkspaceId) -> String {
    format!("{PREFIX}{id}")
}

/// The isolation config written into every workspace: automount off (no host
/// filesystem) and interop off (cannot launch host .exe). See ADR-006.
const WSL_CONF: &str = "[automount]\nenabled = false\nmountFsTab = false\n\n[interop]\nenabled = false\nappendWindowsPath = false\n\n[network]\ngenerateResolvConf = true\n";

pub struct WindowsAdapter {
    data_dir: PathBuf,
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
        Self { data_dir }
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

    /// The cached rootfs path, fetched once via curl.exe (bundled on Windows).
    fn rootfs(&self) -> Result<PathBuf> {
        let dst = self.data_dir.join("rootfs-alpine.tar.gz");
        if dst.metadata().map(|m| m.len() > 0).unwrap_or(false) {
            return Ok(dst);
        }
        let status = Command::new("curl.exe")
            .args(["-sL", "--max-time", "180", "-o"])
            .arg(&dst)
            .arg(ALPINE_URL)
            .status()
            .map_err(|e| WseError::Internal(format!("curl.exe not runnable: {e}")))?;
        if !status.success() {
            return Err(WseError::ResourceUnavailable("could not fetch rootfs".into()));
        }
        Ok(dst)
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
        // What this adapter can *bridge*. v1: lifecycle + isolation only. As the
        // runtime gains a display stack we declare Applications/Windows here and
        // the workspace's effective set is this ∩ runtime.capabilities.
        CapabilitySet::none()
    }

    fn runtime(&self) -> RuntimeDescriptor {
        // wse-linux-x11: the WSL2 Linux runtime this adapter imports. v0.1.0 is
        // the bare rootfs — no display stack yet, so it provides nothing inside
        // the workspace. Building the immutable Runtime v1 image (Xvfb + WM +
        // wmctrl + catalog apps) bumps this to v1.0.0 and declares Applications.
        // See runtimes/wse-linux-x11/ and contract/core/runtime.md.
        RuntimeDescriptor {
            id: RuntimeId::from_raw("wse-linux-x11"),
            name: "wse-linux-x11".into(),
            version: RuntimeVersion::new(0, 1, 0),
            base: "alpine-3.20".into(),
            digest: "alpine-minirootfs-3.20.3-x86_64".into(),
            capabilities: CapabilitySet::none(),
            metadata: std::collections::HashMap::new(),
        }
    }

    fn create(&mut self, def: &WorkspaceDef) -> Result<()> {
        let name = distro_name(&def.id);
        let root = self.rootfs()?;
        let inst = self.data_dir.join(&name);
        std::fs::create_dir_all(&inst)
            .map_err(|e| WseError::Internal(format!("mkdir: {e}")))?;
        self.wsl(&[
            "--import",
            &name,
            inst.to_string_lossy().as_ref(),
            root.to_string_lossy().as_ref(),
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
        Ok(())
    }

    // No capability hooks -> every capability is unavailable, truthfully.
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
