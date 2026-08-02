//! wse-adapter-windows-native — the NATIVE Windows adapter. No WSL, no VM.
//!
//! A workspace is a **separate Windows desktop** plus an **isolated per-app
//! profile**. Native apps run on that desktop, invisible to the owner's real
//! screen. It attests the `DesktopProfile` isolation model (honest: input +
//! presentation separated, per-app storage isolated, host filesystem SHARED —
//! contract/core/isolation.md), never `SealedVm`.
//!
//! Platform code lives ONLY here (adapter Rule 3), talking to Win32 via direct
//! FFI — no external crates. The crate is organised as the native **services**
//! the runtime provides:
//!   - Application service — launch / stop / enumerate instances (CreateProcessW)
//!   - Window service       — enumerate / focus / close (EnumDesktopWindows)
//!   - Browser profile mgr  — a fresh isolated profile per instance
//!   - Catalog              — the isolatable apps WSE knows how to launch
//!
//! Effective capabilities = adapter ∩ runtime; both grow together as services land.

use std::collections::HashMap;
use std::ffi::c_void;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use wse_common::*;
use wse_contract::{
    ApplicationsCapability, ContractVersion, IsolationAttestation, WindowsCapability,
    WorkspaceAdapter, WorkspaceDef, CONTRACT_VERSION,
};

// ── Win32 FFI (kept entirely inside this crate) ──────────────────────────────
type Hdesk = *mut c_void;
type Hwnd = *mut c_void;
type Handle = *mut c_void;
const GENERIC_ALL: u32 = 0x1000_0000;
const WM_CLOSE: u32 = 0x0010;

#[repr(C)]
struct StartupInfoW {
    cb: u32,
    lp_reserved: *mut u16,
    lp_desktop: *mut u16,
    lp_title: *mut u16,
    dw_x: u32,
    dw_y: u32,
    dw_x_size: u32,
    dw_y_size: u32,
    dw_x_count_chars: u32,
    dw_y_count_chars: u32,
    dw_fill_attribute: u32,
    dw_flags: u32,
    w_show_window: u16,
    cb_reserved2: u16,
    lp_reserved2: *mut u8,
    h_std_input: Handle,
    h_std_output: Handle,
    h_std_error: Handle,
}

#[repr(C)]
struct ProcessInformation {
    h_process: Handle,
    h_thread: Handle,
    dw_process_id: u32,
    dw_thread_id: u32,
}

type WndEnumProc = extern "system" fn(Hwnd, isize) -> i32;

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
    fn EnumDesktopWindows(hdesk: Hdesk, cb: WndEnumProc, lparam: isize) -> i32;
    fn IsWindowVisible(hwnd: Hwnd) -> i32;
    fn GetWindowTextLengthW(hwnd: Hwnd) -> i32;
    fn GetWindowTextW(hwnd: Hwnd, s: *mut u16, n: i32) -> i32;
    fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
    fn PostMessageW(hwnd: Hwnd, msg: u32, w: usize, l: isize) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateProcessW(
        app: *const u16,
        cmd: *mut u16,
        pa: *const c_void,
        ta: *const c_void,
        inherit: i32,
        flags: u32,
        env: *const c_void,
        dir: *const u16,
        si: *const StartupInfoW,
        pi: *mut ProcessInformation,
    ) -> i32;
    fn CloseHandle(h: Handle) -> i32;
}

/// A NUL-terminated UTF-16 string for the Win32 `W` APIs.
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn desktop_name(id: &WorkspaceId) -> String {
    format!("wse-{id}")
}

// ── Window service: enumerate windows on a desktop ───────────────────────────
/// A window seen on a desktop: (owning pid, HWND as isize, title).
extern "system" fn collect_window(hwnd: Hwnd, lparam: isize) -> i32 {
    unsafe {
        let out = &mut *(lparam as *mut Vec<(u32, isize, String)>);
        if IsWindowVisible(hwnd) == 0 {
            return 1;
        }
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return 1; // untitled windows aren't user-facing app windows
        }
        let mut buf = vec![0u16; len as usize + 1];
        let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        let title = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        out.push((pid, hwnd as isize, title));
    }
    1
}

fn windows_on(hdesk: Hdesk) -> Vec<(u32, isize, String)> {
    let mut out: Vec<(u32, isize, String)> = Vec::new();
    unsafe {
        EnumDesktopWindows(hdesk, collect_window, &mut out as *mut _ as isize);
    }
    out
}

/// Kill a process and its whole tree (browsers spawn children). Uses taskkill —
/// a native Windows tool — rather than a Toolhelp snapshot dance.
fn kill_tree(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .output();
}

// ── Catalog: the isolatable apps WSE knows how to launch ─────────────────────
/// How well WSE can isolate + manage an app (surfaced to users as expectations).
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
enum SupportLevel {
    /// Fully isolated + managed (own profile, own desktop).
    Certified,
    /// Runs correctly; some features may be unavailable.
    Compatible,
    /// Known quirks; best effort.
    Experimental,
}

struct CatalogEntry {
    url: &'static str,
    #[allow(dead_code)]
    level: SupportLevel,
}

/// Map a catalog `entry` to how the native runtime launches it. Browser-first:
/// every entry is a web app in an isolated browser instance (Certified). Apps we
/// cannot isolate cleanly are simply absent — the engine reports them NotFound.
fn catalog(entry: &str) -> Option<CatalogEntry> {
    match entry {
        "browser" => Some(CatalogEntry { url: "about:blank", level: SupportLevel::Certified }),
        "editor" => Some(CatalogEntry { url: "about:blank", level: SupportLevel::Certified }),
        "terminal" => Some(CatalogEntry { url: "about:blank", level: SupportLevel::Certified }),
        _ => None,
    }
}

/// Locate an installed Chromium browser (Edge is present on every Win11).
fn find_browser() -> Option<PathBuf> {
    let candidates = [
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files (x86)\Microsoft\Edge Beta\Application\msedge.exe",
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    ];
    candidates.iter().map(PathBuf::from).find(|p| p.exists())
}

// ── per-workspace state ──────────────────────────────────────────────────────
struct NativeWindow {
    id: WindowId,
    hwnd: isize,
    app: String,
    title: String,
}

struct InstanceMeta {
    pid: u32,
    profile: PathBuf,
}

#[derive(Default)]
struct WsState {
    desktop: isize,
    desktop_name: String,
    profile_dir: PathBuf,
    instances: Vec<ApplicationInstance>,
    windows: Vec<NativeWindow>,
    meta: HashMap<ApplicationInstanceId, InstanceMeta>,
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

    fn ws(&self, id: &WorkspaceId) -> Result<&WsState> {
        self.state
            .get(id)
            .ok_or_else(|| WseError::NotFound(format!("workspace {id}")))
    }

    /// Application service: launch a catalog app as an isolated browser instance
    /// on the workspace desktop, and find the window it opens.
    fn launch_native(
        &mut self,
        id: &WorkspaceId,
        app: &ApplicationDescriptor,
    ) -> Result<ApplicationInstance> {
        let spec = catalog(&app.entry)
            .ok_or_else(|| WseError::NotFound(format!("app {}", app.entry)))?;
        let browser = find_browser()
            .ok_or_else(|| WseError::ResourceUnavailable("no Chromium browser found".into()))?;

        let (desktop, desktop_name, profile_root) = {
            let st = self.ws(id)?;
            (st.desktop, st.desktop_name.clone(), st.profile_dir.clone())
        };

        // Browser profile manager: a fresh isolated profile per instance.
        let iid = ApplicationInstanceId::new();
        let profile = profile_root.join(format!("inst-{iid}"));
        std::fs::create_dir_all(&profile)
            .map_err(|e| WseError::Internal(format!("profile: {e}")))?;

        // Which windows already exist on the desktop — so we can spot the new one.
        let before: std::collections::HashSet<isize> =
            windows_on(desktop as Hdesk).into_iter().map(|(_, h, _)| h).collect();

        // Launch on the workspace desktop (lpDesktop), own profile, new window.
        let cmdline = format!(
            "\"{}\" --user-data-dir=\"{}\" --no-first-run --no-default-browser-check \
             --new-window {}",
            browser.display(),
            profile.display(),
            spec.url
        );
        let pid = create_process_on_desktop(&desktop_name, &cmdline)?;

        // Window service: poll for the new window this instance opened.
        let deadline = Instant::now() + Duration::from_secs(12);
        let (hwnd, title) = loop {
            if let Some((_, h, t)) = windows_on(desktop as Hdesk)
                .into_iter()
                .find(|(_, h, t)| !before.contains(h) && !t.is_empty())
            {
                break (h, t);
            }
            if Instant::now() > deadline {
                kill_tree(pid);
                let _ = std::fs::remove_dir_all(&profile);
                return Err(WseError::Internal(format!(
                    "app '{}' opened no window within timeout",
                    app.entry
                )));
            }
            std::thread::sleep(Duration::from_millis(200));
        };

        let wid = WindowId::new();
        let instance = ApplicationInstance {
            id: iid.clone(),
            application: app.id.clone(),
            state: ApplicationState::Running,
            windows: vec![wid.clone()],
        };
        let st = self.state.get_mut(id).unwrap();
        st.windows.push(NativeWindow {
            id: wid,
            hwnd,
            app: app.entry.clone(),
            title,
        });
        st.instances.push(instance.clone());
        st.meta.insert(iid, InstanceMeta { pid, profile });
        Ok(instance)
    }

    /// Tear down every app process + profile on a workspace (used by stop/destroy).
    fn teardown_apps(st: &mut WsState) {
        for w in &st.windows {
            unsafe {
                PostMessageW(w.hwnd as Hwnd, WM_CLOSE, 0, 0);
            }
        }
        for (_, m) in st.meta.drain() {
            kill_tree(m.pid);
            let _ = std::fs::remove_dir_all(&m.profile);
        }
        st.windows.clear();
        st.instances.clear();
    }
}

/// Launch a command line on a named desktop; return the new process id.
fn create_process_on_desktop(desktop: &str, cmdline: &str) -> Result<u32> {
    let mut desk = wide(desktop);
    let mut cmd = wide(cmdline);
    let mut si: StartupInfoW = unsafe { std::mem::zeroed() };
    si.cb = std::mem::size_of::<StartupInfoW>() as u32;
    si.lp_desktop = desk.as_mut_ptr();
    let mut pi: ProcessInformation = unsafe { std::mem::zeroed() };
    let ok = unsafe {
        CreateProcessW(
            std::ptr::null(),
            cmd.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            0,
            std::ptr::null(),
            std::ptr::null(),
            &si,
            &mut pi,
        )
    };
    if ok == 0 {
        return Err(WseError::Internal("CreateProcess failed".into()));
    }
    unsafe {
        CloseHandle(pi.h_thread);
        CloseHandle(pi.h_process);
    }
    Ok(pi.dw_process_id)
}

impl WorkspaceAdapter for WindowsNativeAdapter {
    fn contract_version(&self) -> ContractVersion {
        CONTRACT_VERSION
    }

    fn capabilities(&self) -> CapabilitySet {
        CapabilitySet::none()
            .with(Capability::Applications)
            .with(Capability::Windows)
    }

    fn runtime(&self) -> RuntimeDescriptor {
        RuntimeDescriptor {
            id: RuntimeId::from_raw("windows-native"),
            name: "windows-native".into(),
            version: RuntimeVersion::new(0, 2, 0),
            base: "windows-host".into(),
            digest: "windows-native-host".into(),
            capabilities: CapabilitySet::none()
                .with(Capability::Applications)
                .with(Capability::Windows),
            metadata: HashMap::new(),
        }
    }

    fn create(&mut self, def: &WorkspaceDef) -> Result<()> {
        let name = desktop_name(&def.id);
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
            return Err(WseError::Internal(format!("CreateDesktop('{name}') failed")));
        }
        let profile_dir = self.data_dir.join(&name);
        std::fs::create_dir_all(&profile_dir)
            .map_err(|e| WseError::Internal(format!("profile dir: {e}")))?;
        self.state.insert(
            def.id.clone(),
            WsState {
                desktop: hd as isize,
                desktop_name: name,
                profile_dir,
                ..Default::default()
            },
        );
        Ok(())
    }

    fn start(&mut self, id: &WorkspaceId) -> Result<IsolationAttestation> {
        let st = self.ws(id)?;
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

    fn stop(&mut self, id: &WorkspaceId) -> Result<()> {
        if let Some(st) = self.state.get_mut(id) {
            Self::teardown_apps(st);
        }
        Ok(())
    }

    fn destroy(&mut self, id: &WorkspaceId) -> Result<()> {
        let mut st = self
            .state
            .remove(id)
            .ok_or_else(|| WseError::NotFound(format!("workspace {id}")))?;
        Self::teardown_apps(&mut st);
        std::thread::sleep(Duration::from_millis(300)); // let windows/threads exit
        unsafe {
            CloseDesktop(st.desktop as Hdesk);
        }
        let _ = std::fs::remove_dir_all(&st.profile_dir);
        Ok(())
    }

    fn applications(&mut self) -> Option<&mut dyn ApplicationsCapability> {
        Some(self)
    }

    fn windows(&mut self) -> Option<&mut dyn WindowsCapability> {
        Some(self)
    }
}

// ── Application service (contract face) ──────────────────────────────────────
impl ApplicationsCapability for WindowsNativeAdapter {
    fn app_launch(
        &mut self,
        id: &WorkspaceId,
        app: &ApplicationDescriptor,
    ) -> Result<ApplicationInstance> {
        self.launch_native(id, app)
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
        // Close its windows, then kill its process tree and drop its profile.
        for w in st.windows.iter().filter(|w| inst.windows.contains(&w.id)) {
            unsafe {
                PostMessageW(w.hwnd as Hwnd, WM_CLOSE, 0, 0);
            }
        }
        st.windows.retain(|w| !inst.windows.contains(&w.id));
        if let Some(m) = st.meta.remove(instance) {
            kill_tree(m.pid);
            let _ = std::fs::remove_dir_all(&m.profile);
        }
        Ok(())
    }

    fn app_instances(&self, id: &WorkspaceId) -> Result<Vec<ApplicationInstance>> {
        Ok(self.state.get(id).map(|s| s.instances.clone()).unwrap_or_default())
    }
}

// ── Window service (contract face) ───────────────────────────────────────────
impl WindowsCapability for WindowsNativeAdapter {
    fn list_windows(&self, id: &WorkspaceId) -> Result<Vec<Window>> {
        // Focus is a contract concept the adapter maintains: the most recently
        // launched window is focused, at most one (matching the reference).
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
