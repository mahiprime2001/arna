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
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use wse_common::*;
use wse_contract::{
    ApplicationsCapability, ClipboardCapability, ContractVersion, IsolationAttestation,
    StorageCapability, WindowsCapability, WorkspaceAdapter, WorkspaceDef, CONTRACT_VERSION,
};

mod dock;
mod job;
mod overlay;
pub use job::Job;
pub use overlay::{
    changes as overlay_changes, discard as overlay_discard, import as overlay_import,
    list as overlay_list, merge as overlay_merge, Change as OverlayChange,
};

// ── Win32 FFI (kept entirely inside this crate) ──────────────────────────────
type Hdesk = *mut c_void;
type Hwnd = *mut c_void;
type Handle = *mut c_void;
const GENERIC_ALL: u32 = 0x1000_0000;
const WM_CLOSE: u32 = 0x0010;
const CF_UNICODETEXT: u32 = 13;
const GMEM_MOVEABLE: u32 = 0x0002;

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

const WM_HOTKEY: u32 = 0x0312;
const MOD_ALT: u32 = 0x0001;
const MOD_CONTROL: u32 = 0x0002;
const VK_Q: u32 = 0x51;

#[repr(C)]
struct Msg {
    hwnd: Hwnd,
    message: u32,
    w_param: usize,
    l_param: isize,
    time: u32,
    pt_x: i32,
    pt_y: i32,
}

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
    fn OpenDesktopW(name: *const u16, flags: u32, inherit: i32, access: u32) -> Hdesk;
    fn SwitchDesktop(h: Hdesk) -> i32;
    fn SetThreadDesktop(h: Hdesk) -> i32;
    fn RegisterHotKey(hwnd: Hwnd, id: i32, modifiers: u32, vk: u32) -> i32;
    fn UnregisterHotKey(hwnd: Hwnd, id: i32) -> i32;
    fn GetMessageW(msg: *mut Msg, hwnd: Hwnd, min: u32, max: u32) -> i32;
    fn EnumDesktopWindows(hdesk: Hdesk, cb: WndEnumProc, lparam: isize) -> i32;
    fn IsWindowVisible(hwnd: Hwnd) -> i32;
    fn GetWindowTextLengthW(hwnd: Hwnd) -> i32;
    fn GetWindowTextW(hwnd: Hwnd, s: *mut u16, n: i32) -> i32;
    fn GetWindowThreadProcessId(hwnd: Hwnd, pid: *mut u32) -> u32;
    fn PostMessageW(hwnd: Hwnd, msg: u32, w: usize, l: isize) -> i32;
    // OS clipboard (the "external resource" the Shared mode bridges to).
    fn OpenClipboard(hwnd: Hwnd) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn SetClipboardData(fmt: u32, mem: Handle) -> Handle;
    fn GetClipboardData(fmt: u32) -> Handle;
    fn IsClipboardFormatAvailable(fmt: u32) -> i32;
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
    fn ResumeThread(thread: Handle) -> u32;
    fn GlobalAlloc(flags: u32, bytes: usize) -> Handle;
    fn GlobalLock(h: Handle) -> *mut c_void;
    fn GlobalUnlock(h: Handle) -> i32;
}

/// A NUL-terminated UTF-16 string for the Win32 `W` APIs.
pub(crate) fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub(crate) fn desktop_name(id: &WorkspaceId) -> String {
    format!("wse-{id}")
}

// ── "Enter a workspace": make its desktop the one you see + interact with ─────
// SwitchDesktop is *the* feature that turns a workspace from a technicality into a
// place. We switch the user to the workspace desktop and arm a return hotkey
// (Ctrl+Alt+Q) ON that desktop, so they can always get back to their real
// desktop — the shell's console lives there.

/// Enter a workspace's desktop. The user now sees + drives the apps running in it.
/// Press **Ctrl+Alt+Q** to return to the normal desktop. Non-blocking: a small
/// thread holds the return hotkey on the workspace desktop until it fires.
pub fn enter_workspace_desktop(id: &WorkspaceId) {
    let name = desktop_name(id);
    std::thread::spawn(move || unsafe {
        let target = OpenDesktopW(wide(&name).as_ptr(), 0, 0, GENERIC_ALL);
        if target.is_null() {
            return;
        }
        let default = OpenDesktopW(wide("Default").as_ptr(), 0, 0, GENERIC_ALL);
        // The hotkey must belong to the workspace desktop, so this thread joins it.
        SetThreadDesktop(target);
        SwitchDesktop(target);
        RegisterHotKey(std::ptr::null_mut(), 1, MOD_CONTROL | MOD_ALT, VK_Q);
        let mut msg: Msg = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            if msg.message == WM_HOTKEY {
                if !default.is_null() {
                    SwitchDesktop(default);
                }
                break;
            }
        }
        UnregisterHotKey(std::ptr::null_mut(), 1);
        CloseDesktop(target);
        if !default.is_null() {
            CloseDesktop(default);
        }
    });
}

/// A workspace's home dir under the default convention
/// (`%USERPROFILE%\.wse\workspaces\wse-<id>`).
pub(crate) fn workspace_home(id: &WorkspaceId) -> PathBuf {
    let base = std::env::var("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    base.join(".wse").join("workspaces").join(desktop_name(id))
}

/// Spawn the workspace dock on a workspace's desktop — a little shell (launch a
/// browser, Focus/Minimize/Close/Leave windows) so the desktop is usable without
/// a taskbar. Safe to call once per workspace.
pub fn spawn_workspace_dock(id: &WorkspaceId) {
    let name = desktop_name(id);
    let home = workspace_home(id);
    let profiles = home.join("profiles");
    let _ = std::fs::create_dir_all(&profiles);
    let Some(browser) = find_browser() else { return };
    dock::spawn_dock(name, browser, profiles, home.to_string_lossy().into_owned());
}

/// Import your real Edge (or Chrome) profile into a workspace, so its browser has
/// your logins, bookmarks and extensions — but ISOLATED (it's a copy; changes
/// stay in the workspace). Close the source browser first for a clean copy of
/// cookies/logins (locked files are skipped). Launch it with the "My Browser"
/// dock button or `launch_imported_browser`.
pub fn import_default_profile(id: &WorkspaceId, chrome: bool) -> Result<()> {
    let local = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .map_err(|_| WseError::Internal("no LOCALAPPDATA".into()))?;
    let src_root = if chrome {
        local.join("Google").join("Chrome").join("User Data")
    } else {
        local.join("Microsoft").join("Edge").join("User Data")
    };
    if !src_root.exists() {
        return Err(WseError::NotFound(format!(
            "{} profile not found at {}",
            if chrome { "Chrome" } else { "Edge" },
            src_root.display()
        )));
    }
    let dst = workspace_home(id).join("profiles").join("imported");
    let _ = std::fs::remove_dir_all(&dst); // fresh import
    std::fs::create_dir_all(&dst).map_err(|e| WseError::Internal(format!("import dst: {e}")))?;

    // Local State holds the profile's encryption key (DPAPI, same user → works).
    let _ = std::fs::copy(src_root.join("Local State"), dst.join("Local State"));
    // The user's profile subdir, copied into "Default" so a plain launch finds it.
    let prof = if src_root.join("Default").exists() {
        src_root.join("Default")
    } else {
        first_profile_dir(&src_root).unwrap_or_else(|| src_root.join("Default"))
    };
    copy_tree(&prof, &dst.join("Default"));
    // If you imported Chrome, launch Chrome too.
    set_preferred_browser(chrome);
    Ok(())
}

/// Launch a browser on the workspace desktop using the imported profile.
pub fn launch_imported_browser(id: &WorkspaceId) -> Result<()> {
    let home = workspace_home(id);
    let imported = home.join("profiles").join("imported");
    if !imported.exists() {
        return Err(WseError::NotFound(
            "no imported profile — run import first".into(),
        ));
    }
    let browser = find_browser()
        .ok_or_else(|| WseError::ResourceUnavailable("no Chromium browser found".into()))?;
    let cmd = format!(
        "\"{}\" --user-data-dir=\"{}\" --no-first-run --no-default-browser-check --new-window",
        browser.display(),
        imported.display()
    );
    create_process_on_desktop(&desktop_name(id), &cmd, &home.to_string_lossy())?;
    Ok(())
}

fn first_profile_dir(root: &Path) -> Option<PathBuf> {
    std::fs::read_dir(root).ok()?.flatten().find_map(|e| {
        let p = e.path();
        if p.is_dir() && p.file_name()?.to_string_lossy().starts_with("Profile ") {
            Some(p)
        } else {
            None
        }
    })
}

/// Best-effort recursive copy that skips browser caches and lock files (large or
/// locked). Failures on individual files are ignored so a running browser doesn't
/// abort the whole import.
fn copy_tree(src: &Path, dst: &Path) {
    const SKIP: &[&str] = &[
        "Cache", "Code Cache", "GPUCache", "DawnCache", "DawnGraphiteCache",
        "GrShaderCache", "ShaderCache", "Service Worker", "Crashpad", "CrashpadMetrics",
        "component_crx_cache", "Safe Browsing", "Lock", "SingletonLock", "SingletonCookie",
        "SingletonSocket",
    ];
    let _ = std::fs::create_dir_all(dst);
    let Ok(entries) = std::fs::read_dir(src) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_s = name.to_string_lossy();
        if SKIP.iter().any(|s| name_s.eq_ignore_ascii_case(s)) {
            continue;
        }
        let sp = entry.path();
        let dp = dst.join(&name);
        match entry.file_type() {
            Ok(t) if t.is_dir() => copy_tree(&sp, &dp),
            _ => {
                let _ = std::fs::copy(&sp, &dp);
            }
        }
    }
}

/// The titles of the visible windows currently on a workspace's desktop
/// (diagnostic — includes the dock and any apps). Empty if the desktop is gone.
pub fn desktop_window_titles(id: &WorkspaceId) -> Vec<String> {
    let name = desktop_name(id);
    unsafe {
        let d = OpenDesktopW(wide(&name).as_ptr(), 0, 0, GENERIC_ALL);
        if d.is_null() {
            return Vec::new();
        }
        let ws = windows_on(d);
        CloseDesktop(d);
        ws.into_iter().map(|(_, _, t)| t).collect()
    }
}

/// Return to the normal (Default) desktop immediately.
pub fn switch_to_default_desktop() {
    unsafe {
        let d = OpenDesktopW(wide("Default").as_ptr(), 0, 0, GENERIC_ALL);
        if !d.is_null() {
            SwitchDesktop(d);
            CloseDesktop(d);
        }
    }
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

/// Remove a directory tree, retrying briefly: a just-killed browser can hold file
/// locks in its profile for a moment, so a single remove_dir_all can fail. This
/// keeps destroy leaving zero leftover state (conformance criterion #6).
fn robust_rmdir(path: &Path) {
    for _ in 0..12 {
        if !path.exists() {
            return;
        }
        if std::fs::remove_dir_all(path).is_ok() && !path.exists() {
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let _ = std::fs::remove_dir_all(path);
}

// ── Workspace clipboard service ──────────────────────────────────────────────
// Windows has ONE clipboard per session, but WSE wants one clipboard per
// workspace. So the workspace OWNS its clipboard and the OS clipboard is an
// external resource this service may or may not bridge to — the contract's
// read/write never changes; only what they mean here. See contract/core has no
// entry for this: it is an adapter implementation detail (the contract is
// unchanged and locked).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ClipboardMode {
    /// Default. The workspace clipboard is private and never reaches Windows —
    /// copy in workspace A cannot be pasted in workspace B or on the real desktop.
    #[default]
    Isolated,
    /// The workspace clipboard *is* the Windows clipboard (normal Windows).
    Shared,
    /// Private workspace clipboard, plus an explicit user-triggered sync to/from
    /// Windows (the sync action is a native op, not part of the read/write
    /// contract). For read/write it behaves like Isolated until a sync happens.
    ControlledSync,
}

/// Write UTF-8 text to the OS clipboard (Shared mode). Best-effort: the clipboard
/// can be briefly locked by another process, so retry a little.
fn os_clipboard_write(text: &str) -> bool {
    let wide_text: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    for _ in 0..5 {
        unsafe {
            if OpenClipboard(std::ptr::null_mut()) == 0 {
                std::thread::sleep(Duration::from_millis(30));
                continue;
            }
            EmptyClipboard();
            let bytes = wide_text.len() * 2;
            let h = GlobalAlloc(GMEM_MOVEABLE, bytes);
            if !h.is_null() {
                let p = GlobalLock(h) as *mut u16;
                if !p.is_null() {
                    std::ptr::copy_nonoverlapping(wide_text.as_ptr(), p, wide_text.len());
                    GlobalUnlock(h);
                    SetClipboardData(CF_UNICODETEXT, h); // system owns h now
                }
            }
            CloseClipboard();
            return true;
        }
    }
    false
}

/// Read text from the OS clipboard (Shared mode), if it holds unicode text.
fn os_clipboard_read() -> Option<String> {
    for _ in 0..5 {
        unsafe {
            if OpenClipboard(std::ptr::null_mut()) == 0 {
                std::thread::sleep(Duration::from_millis(30));
                continue;
            }
            let mut out = None;
            if IsClipboardFormatAvailable(CF_UNICODETEXT) != 0 {
                let h = GetClipboardData(CF_UNICODETEXT);
                if !h.is_null() {
                    let p = GlobalLock(h) as *const u16;
                    if !p.is_null() {
                        let mut len = 0usize;
                        while *p.add(len) != 0 {
                            len += 1;
                        }
                        let slice = std::slice::from_raw_parts(p, len);
                        out = Some(String::from_utf16_lossy(slice));
                        GlobalUnlock(h);
                    }
                }
            }
            CloseClipboard();
            return out;
        }
    }
    None
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
    #[allow(dead_code)]
    level: SupportLevel,
}

/// The apps this runtime knows how to launch, isolated, on a workspace desktop:
/// a browser (Chrome/Edge), VS Code, and a terminal. Anything else is NotFound.
/// The actual command is built by `build_command`.
fn catalog(entry: &str) -> Option<CatalogEntry> {
    match entry {
        "browser" => Some(CatalogEntry { level: SupportLevel::Certified }),
        "editor" => Some(CatalogEntry { level: SupportLevel::Compatible }),
        "terminal" => Some(CatalogEntry { level: SupportLevel::Compatible }),
        _ => None,
    }
}

// Which Chromium browser WSE launches. Default Edge (always on Win11); set to
// Chrome with `set_preferred_browser(true)` (the shell's `browser chrome`).
static PREFER_CHROME: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Choose Chrome (true) or Edge (false) as the browser for subsequent launches.
pub fn set_preferred_browser(chrome: bool) {
    PREFER_CHROME.store(chrome, std::sync::atomic::Ordering::Relaxed);
}

/// Is Chrome available on this machine?
pub fn chrome_available() -> bool {
    first_existing(&CHROME_PATHS).is_some()
}

const EDGE_PATHS: [&str; 3] = [
    r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
    r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    r"C:\Program Files (x86)\Microsoft\Edge Beta\Application\msedge.exe",
];
const CHROME_PATHS: [&str; 2] = [
    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
];

fn first_existing(paths: &[&str]) -> Option<PathBuf> {
    paths.iter().map(PathBuf::from).find(|p| p.exists())
}

/// Locate the preferred installed Chromium browser (falls back to the other).
pub(crate) fn find_browser() -> Option<PathBuf> {
    if PREFER_CHROME.load(std::sync::atomic::Ordering::Relaxed) {
        first_existing(&CHROME_PATHS).or_else(|| first_existing(&EDGE_PATHS))
    } else {
        first_existing(&EDGE_PATHS).or_else(|| first_existing(&CHROME_PATHS))
    }
}

/// Locate installed VS Code (user or system install, stable or Insiders).
fn find_vscode() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let l = PathBuf::from(local);
        candidates.push(l.join(r"Programs\Microsoft VS Code\Code.exe"));
        candidates.push(l.join(r"Programs\Microsoft VS Code Insiders\Code - Insiders.exe"));
    }
    candidates.push(PathBuf::from(r"C:\Program Files\Microsoft VS Code\Code.exe"));
    candidates.push(PathBuf::from(r"C:\Program Files\Microsoft VS Code Insiders\Code - Insiders.exe"));
    candidates.into_iter().find(|p| p.exists())
}

/// Build the launch command for a catalog entry. `profile` isolates the instance
/// (a unique dir forces a fresh window, not a redirect to an existing one);
/// `home` is the workspace home (working dir / folder to open). Returns the
/// command line + CreateProcess flags, or None if the app isn't installed.
fn build_command(entry: &str, profile: &Path, home: &Path) -> Option<(String, u32)> {
    match entry {
        "browser" => {
            let b = find_browser()?;
            Some((
                format!(
                    "\"{}\" --user-data-dir=\"{}\" --no-first-run --no-default-browser-check \
                     --new-window about:blank",
                    b.display(),
                    profile.display()
                ),
                0,
            ))
        }
        "editor" => {
            let code = find_vscode()?;
            Some((
                format!(
                    "\"{}\" --new-window --user-data-dir=\"{}\" --extensions-dir=\"{}\" \"{}\"",
                    code.display(),
                    profile.display(),
                    profile.join("ext").display(),
                    home.display()
                ),
                0,
            ))
        }
        "terminal" => {
            // A PowerShell console on the workspace desktop — reliable and
            // per-desktop (unlike single-instance Windows Terminal).
            Some((
                format!(
                    "powershell.exe -NoExit -Command \"Set-Location -LiteralPath '{}'\"",
                    home.display()
                ),
                CREATE_NEW_CONSOLE,
            ))
        }
        _ => None,
    }
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
    home: PathBuf,
    instances: Vec<ApplicationInstance>,
    windows: Vec<NativeWindow>,
    meta: HashMap<ApplicationInstanceId, InstanceMeta>,
    /// The workspace's OWN clipboard (Isolated/ControlledSync modes). Private to
    /// the workspace; dropped when the workspace is destroyed.
    clipboard: Option<ClipboardItem>,
}

/// The subdirectories every workspace home gets — its own little environment.
/// `storage` holds contract resources; `profiles` holds per-app browser profiles;
/// the rest are the workspace's home folders (the foundation for `workspace://`
/// paths and, since ALL persistent state lives under one home, for future
/// snapshots). See contract/capabilities/storage.md.
const HOME_DIRS: &[&str] = &[
    "storage", "profiles", "documents", "downloads", "config", "cache", "tmp", "desktop",
];

pub struct WindowsNativeAdapter {
    /// Root under which every workspace home lives (`%USERPROFILE%\.wse\workspaces`).
    home_root: PathBuf,
    state: HashMap<WorkspaceId, WsState>,
    clipboard_mode: ClipboardMode,
}

impl Default for WindowsNativeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsNativeAdapter {
    pub fn new() -> Self {
        // Workspaces live in the user's home, like other tools' dot-dirs.
        let base = std::env::var("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir());
        let home_root = base.join(".wse").join("workspaces");
        let _ = std::fs::create_dir_all(&home_root);
        Self {
            home_root,
            state: HashMap::new(),
            clipboard_mode: ClipboardMode::default(), // Isolated (privacy-first)
        }
    }

    /// Choose where workspace homes live (defaults to `%USERPROFILE%\.wse`).
    pub fn with_home_root(mut self, root: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&root);
        self.home_root = root;
        self
    }

    /// Choose how the workspace clipboard relates to the Windows clipboard.
    pub fn with_clipboard_mode(mut self, mode: ClipboardMode) -> Self {
        self.clipboard_mode = mode;
        self
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
        // Known catalog app? (undetectability — unknown entries are NotFound.)
        if catalog(&app.entry).is_none() {
            return Err(WseError::NotFound(format!("app {}", app.entry)));
        }

        let (desktop, desktop_name, home) = {
            let st = self.ws(id)?;
            (st.desktop, st.desktop_name.clone(), st.home.clone())
        };

        // A fresh isolated profile per instance, under the workspace's Storage.
        let iid = ApplicationInstanceId::new();
        let profile = home.join("profiles").join(format!("inst-{iid}"));
        std::fs::create_dir_all(&profile)
            .map_err(|e| WseError::Internal(format!("profile: {e}")))?;

        // Which app to run (browser / VS Code / terminal); None if not installed.
        let (cmdline, flags) = build_command(&app.entry, &profile, &home).ok_or_else(|| {
            WseError::ResourceUnavailable(format!("app '{}' is not installed", app.entry))
        })?;

        // Which windows already exist on the desktop — so we can spot the new one.
        let before: std::collections::HashSet<isize> =
            windows_on(desktop as Hdesk).into_iter().map(|(_, h, _)| h).collect();

        // Launch on the workspace desktop (lpDesktop), working dir = workspace home.
        let pid = create_process_flags(&desktop_name, &cmdline, &home.to_string_lossy(), flags)?;

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
            robust_rmdir(&m.profile);
        }
        st.windows.clear();
        st.instances.clear();
    }
}

/// Launch a command line on a named desktop, with a working directory; return the
/// new process id.
pub(crate) fn create_process_on_desktop(desktop: &str, cmdline: &str, workdir: &str) -> Result<u32> {
    create_process_flags(desktop, cmdline, workdir, 0)
}

/// New console window for console apps (terminals).
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

fn create_process_flags(desktop: &str, cmdline: &str, workdir: &str, flags: u32) -> Result<u32> {
    let mut desk = wide(desktop);
    let mut cmd = wide(cmdline);
    let dir = wide(workdir);
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
            flags,
            std::ptr::null(),
            dir.as_ptr(),
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

// ── WRM generic executor: run a LaunchPlan, own it with a Job Object ──────────
// The data-driven launch path (WSE v2). A projected `LaunchPlan` — never
// app-specific code — becomes a real process on the workspace desktop, assigned
// to a Windows **Job Object** so the workspace owns its whole process tree
// (KILL_ON_JOB_CLOSE => zero orphans on teardown). The old `build_command` path
// stays until this is proven end-to-end for VS Code, then it retires.

const CREATE_SUSPENDED: u32 = 0x0000_0004;
const CREATE_UNICODE_ENVIRONMENT: u32 = 0x0000_0400;

/// Quote a command-line token if it contains whitespace. Workspace-home paths
/// usually don't, but be safe.
fn quote_arg(token: &str) -> String {
    if !token.is_empty() && !token.bytes().any(|b| b == b' ' || b == b'\t') {
        token.to_string()
    } else {
        format!("\"{}\"", token.replace('"', ""))
    }
}

/// The full command line — derived ENTIRELY from the plan (executable +
/// arguments). No application is named here; that's the whole point.
fn plan_command_line(plan: &wse_wrm::LaunchPlan) -> String {
    let mut s = quote_arg(&plan.executable.to_string_lossy());
    for a in &plan.arguments {
        s.push(' ');
        s.push_str(&quote_arg(a));
    }
    s
}

/// The child's environment: the host environment with the plan's projected
/// variables layered on top, as a UTF-16 double-NUL block. Empty when the plan
/// sets none (then the child simply inherits ours).
fn plan_environment_block(plan: &wse_wrm::LaunchPlan) -> Vec<u16> {
    use std::collections::BTreeMap;
    let mut vars: BTreeMap<String, String> = std::env::vars().collect();
    for (k, v) in &plan.environment {
        vars.insert(k.clone(), v.clone());
    }
    let mut block: Vec<u16> = Vec::new();
    for (k, v) in vars {
        block.extend(format!("{k}={v}").encode_utf16());
        block.push(0);
    }
    block.push(0);
    block
}

/// Create the workspace-scoped directories the plan projects (extensions dir,
/// user-data dir, …) and the working directory, so the app finds them.
fn stage_plan_dirs(plan: &wse_wrm::LaunchPlan) {
    let _ = std::fs::create_dir_all(&plan.working_directory);
    for r in &plan.resources {
        if let Some(p) = &r.workspace_path {
            let _ = std::fs::create_dir_all(p);
        }
    }
}

/// Execute a `LaunchPlan` on a workspace desktop; return the new process id and
/// the **Job** that owns it (drop or `terminate()` kills the whole tree).
/// Generic — no application-specific logic; everything comes from the plan.
pub fn execute_plan(desktop: &str, plan: &wse_wrm::LaunchPlan) -> Result<(u32, job::Job)> {
    stage_plan_dirs(plan);
    let job = job::Job::create().ok_or_else(|| WseError::Internal("CreateJobObject failed".into()))?;

    let mut desk = wide(desktop);
    let mut cmd = wide(&plan_command_line(plan));
    let dir = wide(&plan.working_directory.to_string_lossy());
    let mut env_block = plan_environment_block(plan);

    let (env_ptr, flags): (*const c_void, u32) = if plan.environment.is_empty() {
        (std::ptr::null(), CREATE_SUSPENDED)
    } else {
        (
            env_block.as_mut_ptr() as *const c_void,
            CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT,
        )
    };

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
            flags,
            env_ptr,
            dir.as_ptr(),
            &si,
            &mut pi,
        )
    };
    if ok == 0 {
        return Err(WseError::Internal("CreateProcess (plan) failed".into()));
    }
    // Own it before it runs, so processes it spawns at startup join the Job too.
    job.assign(pi.h_process);
    unsafe {
        ResumeThread(pi.h_thread);
        CloseHandle(pi.h_thread);
        CloseHandle(pi.h_process);
    }
    let _ = &env_block; // env_block must outlive CreateProcessW (Windows copies it)
    Ok((pi.dw_process_id, job))
}

impl WorkspaceAdapter for WindowsNativeAdapter {
    fn contract_version(&self) -> ContractVersion {
        CONTRACT_VERSION
    }

    fn capabilities(&self) -> CapabilitySet {
        CapabilitySet::none()
            .with(Capability::Applications)
            .with(Capability::Windows)
            .with(Capability::Clipboard)
            .with(Capability::Storage)
    }

    fn runtime(&self) -> RuntimeDescriptor {
        RuntimeDescriptor {
            id: RuntimeId::from_raw("windows-native"),
            name: "windows-native".into(),
            version: RuntimeVersion::new(0, 3, 0),
            base: "windows-host".into(),
            digest: "windows-native-host".into(),
            capabilities: CapabilitySet::none()
                .with(Capability::Applications)
                .with(Capability::Windows)
                .with(Capability::Clipboard)
                .with(Capability::Storage),
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
        // The workspace's HOME: its own little environment. Every persistent
        // thing (resources, profiles) lives under here, so destroy is one rm and
        // a snapshot is one copy.
        let home = self.home_root.join(&name);
        for sub in HOME_DIRS {
            std::fs::create_dir_all(home.join(sub))
                .map_err(|e| WseError::Internal(format!("home dir {sub}: {e}")))?;
        }
        self.state.insert(
            def.id.clone(),
            WsState {
                desktop: hd as isize,
                desktop_name: name,
                home,
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
                format!("isolated profile at {}", st.home.display()),
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
        dock::close(&st.desktop_name); // ask the dock thread to exit first
        Self::teardown_apps(&mut st);
        std::thread::sleep(Duration::from_millis(300)); // let windows/threads exit
        unsafe {
            CloseDesktop(st.desktop as Hdesk);
        }
        robust_rmdir(&st.home);
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

    fn storage(&mut self) -> Option<&mut dyn StorageCapability> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "touches the real OS clipboard"]
    fn os_clipboard_roundtrips() {
        // Verifies the Shared-mode Win32 clipboard bridge actually works.
        assert!(os_clipboard_write("wse-shared-roundtrip"));
        assert_eq!(os_clipboard_read().as_deref(), Some("wse-shared-roundtrip"));
    }

    #[test]
    fn copy_tree_copies_files_and_skips_caches() {
        // The profile-import engine, on synthetic data (never your real profile).
        let tmp = std::env::temp_dir().join(format!("wse-copytest-{}", std::process::id()));
        let (src, dst) = (tmp.join("src"), tmp.join("dst"));
        std::fs::create_dir_all(src.join("Cache")).unwrap();
        std::fs::write(src.join("Cache").join("big.bin"), b"junk").unwrap();
        std::fs::write(src.join("Bookmarks"), b"marks").unwrap();
        std::fs::create_dir_all(src.join("Local Storage")).unwrap();
        std::fs::write(src.join("Local Storage").join("x"), b"y").unwrap();

        copy_tree(&src, &dst);

        assert!(dst.join("Bookmarks").exists(), "real files must copy");
        assert!(dst.join("Local Storage").join("x").exists(), "nested files must copy");
        assert!(!dst.join("Cache").exists(), "cache dirs must be skipped");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ── WRM generic executor ────────────────────────────────────────────────
    #[test]
    fn plan_command_is_app_agnostic() {
        // Project VS Code purely from its manifest + policy, then build the exact
        // command the native runtime would run. Nothing about VS Code is hard-coded
        // in this path — it all comes from the data.
        let home = std::env::temp_dir().join(format!("wse-plan-{}", std::process::id()));
        let plan = wse_wrm::project(&wse_wrm::manifests::VSCODE, &wse_wrm::policies::CLEAN, &home)
            .expect("project vscode");
        let cmd = plan_command_line(&plan);
        assert!(cmd.contains("Code.exe"), "executable comes from the manifest: {cmd}");
        assert!(cmd.contains("--extensions-dir="), "extensions arg from the manifest");
        assert!(cmd.contains("--user-data-dir="), "user-data arg from the manifest");
        assert!(cmd.contains("vscode"), "resource paths are workspace-scoped under the home");
        assert!(cmd.contains("--new-window"), "manifest base args are preserved");
    }

    #[test]
    #[ignore = "launches VS Code on the current desktop (manual)"]
    fn launches_vscode_from_manifest_live() {
        // The end-to-end proof: manifest -> projector -> LaunchPlan -> execute_plan
        // -> a real VS Code, owned by a Job. No `if app == "vscode"` anywhere.
        let home = std::env::temp_dir().join(format!("wse-vscode-{}", std::process::id()));
        let plan = wse_wrm::project(&wse_wrm::manifests::VSCODE, &wse_wrm::policies::CLEAN, &home)
            .expect("project");
        let (pid, job) = execute_plan("Default", &plan).expect("launch");
        assert!(pid > 0, "got a pid back");
        std::mem::forget(job); // keep the Job open so VS Code stays up to eyeball it
    }
}

// ── Clipboard service (contract face) ────────────────────────────────────────
// The engine has checked the Clipboard capability + the role's clipboard right
// and will emit the event. The adapter only decides what "the clipboard" means
// for this workspace, per its mode. The contract (read/write) is unchanged.
impl ClipboardCapability for WindowsNativeAdapter {
    fn clipboard_peek(&self, id: &WorkspaceId) -> Result<Option<ClipboardItem>> {
        let st = self
            .state
            .get(id)
            .ok_or_else(|| WseError::NotFound(format!("workspace {id}")))?;
        match self.clipboard_mode {
            // Private workspace clipboard.
            ClipboardMode::Isolated | ClipboardMode::ControlledSync => Ok(st.clipboard.clone()),
            // The OS clipboard is the workspace clipboard.
            ClipboardMode::Shared => Ok(os_clipboard_read().map(ClipboardItem::text)),
        }
    }

    fn clipboard_put(&mut self, id: &WorkspaceId, data: ClipboardItem) -> Result<()> {
        // In Shared mode the OS clipboard is the workspace clipboard; text/plain
        // goes through the OS, anything else falls back to the private store.
        if self.clipboard_mode == ClipboardMode::Shared && data.mime == "text/plain" {
            let text = String::from_utf8_lossy(&data.payload).into_owned();
            if os_clipboard_write(&text) {
                return Ok(());
            }
        }
        let st = self
            .state
            .get_mut(id)
            .ok_or_else(|| WseError::NotFound(format!("workspace {id}")))?;
        st.clipboard = Some(data);
        Ok(())
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
            robust_rmdir(&m.profile);
        }
        Ok(())
    }

    fn app_instances(&self, id: &WorkspaceId) -> Result<Vec<ApplicationInstance>> {
        Ok(self.state.get(id).map(|s| s.instances.clone()).unwrap_or_default())
    }
}

// ── Storage service (contract face): the workspace's persistent memory ───────
// Contract resources are stored as REAL FILES under the workspace home's
// `storage/` folder: bytes in `<id>`, name in `<id>.name`. Because all persistent
// state (resources + browser profiles) lives under one home directory, destroy is
// a single rm and a snapshot would be a single copy. Resource ids are stable and
// immutable; a deleted id never resolves (I3). Contract unchanged: this is just
// where "resources" physically live.
impl WindowsNativeAdapter {
    fn storage_dir(&self, id: &WorkspaceId) -> Result<PathBuf> {
        Ok(self.ws(id)?.home.join("storage"))
    }
}

impl StorageCapability for WindowsNativeAdapter {
    fn resource_create(
        &mut self,
        id: &WorkspaceId,
        name: String,
        kind: ResourceKind,
    ) -> Result<ResourceMetadata> {
        let dir = self.storage_dir(id)?;
        let rid = ResourceId::new();
        std::fs::write(dir.join(rid.as_str()), [])
            .map_err(|e| WseError::Internal(format!("resource_create: {e}")))?;
        let _ = std::fs::write(dir.join(format!("{rid}.name")), name.as_bytes());
        Ok(ResourceMetadata { id: rid, name, kind, size: 0 })
    }

    fn resource_write(
        &mut self,
        id: &WorkspaceId,
        resource: &ResourceId,
        bytes: Vec<u8>,
    ) -> Result<()> {
        let path = self.storage_dir(id)?.join(resource.as_str());
        if !path.exists() {
            return Err(WseError::NotFound(format!("resource {resource}"))); // deleted/unknown (I3)
        }
        std::fs::write(&path, &bytes)
            .map_err(|e| WseError::Internal(format!("resource_write: {e}")))
    }

    fn resource_read(&self, id: &WorkspaceId, resource: &ResourceId) -> Result<Vec<u8>> {
        let path = self.storage_dir(id)?.join(resource.as_str());
        std::fs::read(&path).map_err(|_| WseError::NotFound(format!("resource {resource}")))
    }

    fn resource_delete(&mut self, id: &WorkspaceId, resource: &ResourceId) -> Result<bool> {
        let dir = self.storage_dir(id)?;
        let existed = dir.join(resource.as_str()).exists();
        let _ = std::fs::remove_file(dir.join(resource.as_str()));
        let _ = std::fs::remove_file(dir.join(format!("{resource}.name")));
        Ok(existed)
    }

    fn resource_list(&self, id: &WorkspaceId) -> Result<Vec<ResourceMetadata>> {
        let dir = self.storage_dir(id)?;
        let mut out = Vec::new();
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => return Ok(out),
        };
        for entry in entries.flatten() {
            let fname = entry.file_name();
            let fname = fname.to_string_lossy();
            if fname.ends_with(".name") {
                continue; // sidecar, not a resource
            }
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let name = std::fs::read_to_string(dir.join(format!("{fname}.name")))
                .unwrap_or_default();
            out.push(ResourceMetadata {
                id: ResourceId::from_raw(fname.to_string()),
                name,
                kind: ResourceKind::Blob,
                size,
            });
        }
        Ok(out)
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
