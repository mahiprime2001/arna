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
    ApplicationsCapability, ClipboardCapability, ContractVersion, IsolationAttestation,
    WindowsCapability, WorkspaceAdapter, WorkspaceDef, CONTRACT_VERSION,
};

// Pinned digests of the shipped runtime images (built by runtimes/*/build.sh).
// Immutable: a change is a new version, never an in-place edit.
const X11_DIGEST: &str =
    "sha256:baf95a179b091ce528135bfa488fe90387e079a386006223bb1db571152e212b";
const LITE_DIGEST: &str =
    "sha256:730365b06319ce69416f88f16d313afc78224546c1d5dcd1e99ae7ace9e3bd20";

/// A runtime this adapter can run workspaces on: its contract descriptor plus the
/// immutable image to import. The adapter is written ONCE and parameterised by
/// this — the whole point of the runtime boundary. `linux_x11_v1` provides
/// Applications + Windows; `lite_v1` provides nothing. Same adapter, different
/// runtime → different effective capabilities, no code change.
#[derive(Clone)]
pub struct RuntimeSpec {
    descriptor: RuntimeDescriptor,
    image: PathBuf,
}

fn default_image_dir() -> PathBuf {
    std::env::temp_dir().join("arna-workspaces")
}

impl RuntimeSpec {
    /// wse-linux-x11 v1.1.0 — display stack + clipboard service (Applications,
    /// Windows, Clipboard).
    pub fn linux_x11_v1() -> Self {
        let image = std::env::var("WSE_RUNTIME_IMAGE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_image_dir().join("wse-linux-x11-v1.1.0.tar"));
        RuntimeSpec {
            descriptor: RuntimeDescriptor {
                id: RuntimeId::from_raw("wse-linux-x11"),
                name: "wse-linux-x11".into(),
                version: RuntimeVersion::new(1, 1, 0),
                base: "alpine-3.20".into(),
                digest: X11_DIGEST.into(),
                capabilities: CapabilitySet::none()
                    .with(Capability::Applications)
                    .with(Capability::Windows)
                    .with(Capability::Clipboard),
                metadata: HashMap::new(),
            },
            image,
        }
    }

    /// wse-lite v1.0.0 — deliberately minimal/headless; provides NO capabilities.
    pub fn lite_v1() -> Self {
        let image = std::env::var("WSE_LITE_IMAGE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_image_dir().join("wse-lite-v1.0.0.tar"));
        RuntimeSpec {
            descriptor: RuntimeDescriptor {
                id: RuntimeId::from_raw("wse-lite"),
                name: "wse-lite".into(),
                version: RuntimeVersion::new(1, 0, 0),
                base: "alpine-3.20".into(),
                digest: LITE_DIGEST.into(),
                capabilities: CapabilitySet::none(),
                metadata: HashMap::new(),
            },
            image,
        }
    }
}

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
    runtime: RuntimeSpec,
    state: HashMap<WorkspaceId, WsState>,
}

impl Default for WindowsAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsAdapter {
    /// The adapter on its default runtime (wse-linux-x11 v1.0.0).
    pub fn new() -> Self {
        Self::with_runtime(RuntimeSpec::linux_x11_v1())
    }

    /// The SAME adapter on a chosen runtime. Nothing about the adapter changes —
    /// only which runtime it imports and negotiates against.
    pub fn with_runtime(runtime: RuntimeSpec) -> Self {
        let data_dir = std::env::temp_dir().join("arna-workspaces");
        let _ = std::fs::create_dir_all(&data_dir);
        Self {
            data_dir,
            runtime,
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

    /// Bring the runtime's display up (idempotent; the runtime provides the
    /// script). Needed before any op that touches X — launching apps or the
    /// X11-backed clipboard.
    fn ensure_display(&self, id: &WorkspaceId) -> Result<()> {
        self.inside(id, "/opt/wse/start-display.sh").map(|_| ())
    }

    /// The runtime image to import. Required — the runtime *is* the image.
    fn runtime_image(&self) -> Result<&PathBuf> {
        let image = &self.runtime.image;
        if image.exists() {
            Ok(image)
        } else {
            Err(WseError::ResourceUnavailable(format!(
                "runtime image for {} not found at {} — build it with its build.sh \
                 or set the image env var",
                self.runtime.descriptor.name,
                image.display()
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
            .with(Capability::Clipboard)
    }

    fn runtime(&self) -> RuntimeDescriptor {
        // Whichever runtime this adapter was built with. The engine intersects
        // this runtime's capabilities with the adapter's bridgeable set above.
        self.runtime.descriptor.clone()
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

    fn clipboard(&mut self) -> Option<&mut dyn ClipboardCapability> {
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
        self.ensure_display(id)?;

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

// ── Clipboard capability: bridge to the runtime's clipboard service ──────────
// The engine has already checked the capability + the role's clipboard right and
// will emit the event. The adapter only translates: contract op -> runtime
// clip.sh call, and runtime failure -> WseError. No X11 knowledge lives here; it
// is all in the runtime's clip.sh (the "clipboard service").
impl ClipboardCapability for WindowsAdapter {
    fn clipboard_peek(&self, id: &WorkspaceId) -> Result<Option<ClipboardItem>> {
        self.ensure_display(id)?;
        let raw = self.inside(id, "/opt/wse/clip.sh get")?;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(None); // nothing owns the clipboard
        }
        let mut lines = trimmed.lines();
        let mime = lines.next().unwrap_or("").trim().to_string();
        let b64: String = lines.map(str::trim).collect();
        let payload = base64_decode(&b64)?;
        Ok(Some(ClipboardItem { mime, payload }))
    }

    fn clipboard_put(&mut self, id: &WorkspaceId, data: ClipboardItem) -> Result<()> {
        self.ensure_display(id)?;
        let b64 = base64_encode(&data.payload);
        // mime comes from the contract item; base64 keeps the payload intact
        // through the Windows -> wsl.exe -> sh layers.
        self.inside(
            id,
            &format!("echo {b64} | /opt/wse/clip.sh set {}", data.mime),
        )?;
        Ok(())
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

/// Minimal base64 decode (no dependency), inverse of `base64_encode`. Ignores
/// whitespace and padding. Used to bring clipboard payloads back from the runtime.
fn base64_decode(s: &str) -> Result<Vec<u8>> {
    fn val(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a' + 26) as u32),
            b'0'..=b'9' => Some((c - b'0' + 52) as u32),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let clean: Vec<u8> = s
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
        .collect();
    let mut out = Vec::with_capacity(clean.len() * 3 / 4);
    for chunk in clean.chunks(4) {
        let mut n: u32 = 0;
        for &c in chunk {
            let v = val(c).ok_or_else(|| WseError::Internal(format!("bad base64 byte {c}")))?;
            n = (n << 6) | v;
        }
        n <<= 6 * (4 - chunk.len()); // pad to a full 24-bit group
        let nbytes = chunk.len() * 6 / 8;
        for i in 0..nbytes {
            out.push(((n >> (16 - 8 * i)) & 0xff) as u8);
        }
    }
    Ok(out)
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
