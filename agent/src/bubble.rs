//! App "bubble": run one app on a hidden Windows desktop, capture its window, and
//! drive it with window messages — so a remote person controls just that app
//! while the owner keeps using their real desktop. No VM / Sandbox needed.
//!
//! Best for **classic Win32 apps** (see `docs/APP-SHARING.md`): Chromium/DirectX
//! apps can capture black and ignore message-based input. The `curated_apps`
//! list is therefore deliberately small and tested.
//!
//! Non-Windows builds get stubs so the crate still compiles everywhere.

/// One app offered for bubble sharing.
#[derive(Clone, Copy)]
pub struct CuratedApp {
    pub id: &'static str,
    pub label: &'static str,
    pub cmd: &'static str,
}

/// The apps we offer to open in a bubble. Kept to ones that behave under the
/// hidden-desktop + PrintWindow + window-message technique.
pub fn curated_apps() -> &'static [CuratedApp] {
    &[
        CuratedApp { id: "charmap", label: "Character Map", cmd: "charmap.exe" },
        CuratedApp { id: "wordpad", label: "WordPad", cmd: "write.exe" },
        CuratedApp { id: "notepad", label: "Notepad", cmd: "notepad.exe" },
        CuratedApp { id: "mspaint", label: "Paint", cmd: "mspaint.exe" },
        CuratedApp { id: "explorer", label: "File Explorer", cmd: "explorer.exe" },
    ]
}

/// Resolve a curated app id to its launch command.
pub fn app_cmd(id: &str) -> Option<&'static str> {
    curated_apps().iter().find(|a| a.id == id).map(|a| a.cmd)
}

/// An app category that **can't** run in a bubble with this technique — shown in
/// the UI (greyed out) so people know why it's not offered.
#[derive(Clone, Copy)]
pub struct UnsupportedApp {
    pub label: &'static str,
    pub reason: &'static str,
}

/// Apps/kinds we deliberately don't offer for bubbling. They're GPU-composited or
/// message-input-resistant, so PrintWindow captures black and window messages
/// don't drive them — they'd need the heavier VM/Sunshine path.
pub fn unsupported_apps() -> &'static [UnsupportedApp] {
    &[
        UnsupportedApp {
            label: "Chrome, Edge & other browsers",
            reason: "GPU-composited (Chromium) — captures black and ignores window-message input",
        },
        UnsupportedApp {
            label: "Electron apps (VS Code, Slack, Discord…)",
            reason: "Chromium under the hood — same limits as browsers",
        },
        UnsupportedApp {
            label: "Games & DirectX/OpenGL apps",
            reason: "GPU-rendered frames can't be captured this way",
        },
        UnsupportedApp {
            label: "Full-screen / hardware-video apps",
            reason: "Bypass the normal window paint that capture relies on",
        },
    ]
}

#[cfg(windows)]
pub use imp::{Bubble, BubbleInput};

#[cfg(not(windows))]
pub use stub::{Bubble, BubbleInput};

// ---------------------------------------------------------------------------

#[cfg(windows)]
mod imp {
    use std::ffi::OsStr;
    use std::iter::once;
    use std::mem::{size_of, zeroed};
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::{null, null_mut};
    use std::sync::atomic::{AtomicU32, Ordering};

    use winapi::shared::minwindef::{BOOL, DWORD, LPARAM, TRUE, UINT, WPARAM};
    use winapi::shared::windef::{HBITMAP, HDC, HDESK, HWND, POINT, RECT};
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::{
        CreateProcessW, GetExitCodeProcess, TerminateProcess, PROCESS_INFORMATION, STARTUPINFOW,
    };
    use winapi::um::wingdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, SelectObject,
        BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use winapi::um::winuser::{
        ChildWindowFromPointEx, CloseDesktop, CreateDesktopW, EnumChildWindows, EnumDesktopWindows,
        GetClassNameW, GetClientRect, GetDC, GetWindowThreadProcessId, IsWindowVisible,
        MapWindowPoints, PostMessageW, PrintWindow, ReleaseDC, CWP_SKIPINVISIBLE,
        CWP_SKIPTRANSPARENT, MK_LBUTTON, MK_MBUTTON, MK_RBUTTON, PW_CLIENTONLY,
        PW_RENDERFULLCONTENT, WM_CHAR, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP,
        WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP,
    };

    const GENERIC_ALL: DWORD = 0x1000_0000;
    const STARTF_USESHOWWINDOW: DWORD = 0x0000_0001;
    const SW_SHOWNORMAL: u16 = 1;
    const STILL_ACTIVE: DWORD = 259;

    static SEQ: AtomicU32 = AtomicU32::new(1);

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(once(0)).collect()
    }

    fn lparam_xy(x: i32, y: i32) -> LPARAM {
        (((y & 0xFFFF) << 16) | (x & 0xFFFF)) as LPARAM
    }

    /// A cheap, `Send` handle the input thread uses to drive the bubble window
    /// with `PostMessage` (window messages are thread-safe; the HWND is just an
    /// integer here). Coordinates are normalized 0..1 over the captured client area.
    #[derive(Clone, Copy)]
    pub struct BubbleInput {
        hwnd: usize,
    }

    impl BubbleInput {
        /// Map a normalized point to client pixels, find the deepest child window
        /// under it, and return (child hwnd, point in that child's client coords).
        unsafe fn target_at(&self, nx: f64, ny: f64) -> (HWND, i32, i32) {
            let top = self.hwnd as HWND;
            let mut rc: RECT = zeroed();
            GetClientRect(top, &mut rc);
            let cx = (nx.clamp(0.0, 1.0) * rc.right as f64) as i32;
            let cy = (ny.clamp(0.0, 1.0) * rc.bottom as f64) as i32;
            let pt = POINT { x: cx, y: cy };
            // Hit-test children so clicks land on buttons/fields, not just the frame.
            let child = ChildWindowFromPointEx(top, pt, CWP_SKIPINVISIBLE | CWP_SKIPTRANSPARENT);
            if child.is_null() || child == top {
                (top, cx, cy)
            } else {
                let mut p = POINT { x: cx, y: cy };
                MapWindowPoints(top, child, &mut p, 1);
                (child, p.x, p.y)
            }
        }

        pub fn mouse_move(&self, nx: f64, ny: f64) {
            unsafe {
                let (h, x, y) = self.target_at(nx, ny);
                PostMessageW(h, WM_MOUSEMOVE, 0, lparam_xy(x, y));
            }
        }

        /// `button`: 0 = left, 1 = middle, 2 = right.
        pub fn mouse_button(&self, nx: f64, ny: f64, button: u8, down: bool) {
            unsafe {
                let (h, x, y) = self.target_at(nx, ny);
                let (msg, mk): (UINT, WPARAM) = match (button, down) {
                    (2, true) => (WM_RBUTTONDOWN, MK_RBUTTON as WPARAM),
                    (2, false) => (WM_RBUTTONUP, 0),
                    (1, true) => (WM_MBUTTONDOWN, MK_MBUTTON as WPARAM),
                    (1, false) => (WM_MBUTTONUP, 0),
                    (_, true) => (WM_LBUTTONDOWN, MK_LBUTTON as WPARAM),
                    (_, false) => (WM_LBUTTONUP, 0),
                };
                PostMessageW(h, msg, mk, lparam_xy(x, y));
            }
        }

        pub fn wheel(&self, nx: f64, ny: f64, delta: i32) {
            unsafe {
                // WM_MOUSEWHEEL wants screen coords in lParam; client child is fine
                // as the message target. Use a wheel notch per event.
                let (h, x, y) = self.target_at(nx, ny);
                let step = if delta > 0 { 120i32 } else { -120i32 };
                let wparam = ((step as u32) << 16) as WPARAM;
                PostMessageW(h, WM_MOUSEWHEEL, wparam, lparam_xy(x, y));
            }
        }

        pub fn key_char(&self, c: char) {
            unsafe {
                let h = self.hwnd as HWND;
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    PostMessageW(h, WM_CHAR, *u as WPARAM, 1);
                }
            }
        }

        /// Named/non-printable key via virtual-key code (down then up handled by caller).
        pub fn key_vk(&self, vk: u16, down: bool) {
            unsafe {
                let h = self.hwnd as HWND;
                PostMessageW(h, if down { WM_KEYDOWN } else { WM_KEYUP }, vk as WPARAM, 1);
            }
        }
    }

    struct Find {
        pid: DWORD,
        best_pid: HWND,
        best_pid_area: i64,
        best_any: HWND,
        best_any_area: i64,
    }

    unsafe extern "system" fn enum_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let f = &mut *(lparam as *mut Find);
        if IsWindowVisible(hwnd) != TRUE {
            return TRUE;
        }
        let mut r: RECT = zeroed();
        if winapi::um::winuser::GetWindowRect(hwnd, &mut r) == 0 {
            return TRUE;
        }
        let area = (r.right - r.left) as i64 * (r.bottom - r.top) as i64;
        if area <= 0 {
            return TRUE;
        }
        if area > f.best_any_area {
            f.best_any_area = area;
            f.best_any = hwnd;
        }
        let mut wpid: DWORD = 0;
        GetWindowThreadProcessId(hwnd, &mut wpid);
        if wpid == f.pid && area > f.best_pid_area {
            f.best_pid_area = area;
            f.best_pid = hwnd;
        }
        TRUE
    }

    unsafe extern "system" fn child_edit_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let out = &mut *(lparam as *mut HWND);
        let mut cls = [0u16; 64];
        let n = GetClassNameW(hwnd, cls.as_mut_ptr(), cls.len() as i32);
        let name = String::from_utf16_lossy(&cls[..n as usize]).to_lowercase();
        if name.contains("edit") {
            *out = hwnd;
            return 0;
        }
        TRUE
    }

    /// A running app on its own hidden desktop. Owns the desktop + process; lives
    /// on the capture thread. `Drop` tears everything down.
    pub struct Bubble {
        hdesk: HDESK,
        proc: HWND, // PROCESS_INFORMATION.hProcess (HANDLE) stored as a pointer
        pid: DWORD,
        hwnd: HWND,
    }

    impl Bubble {
        /// Create a hidden desktop and launch `cmd` on it.
        pub fn launch(cmd: &str) -> Result<Self, String> {
            unsafe {
                let n = SEQ.fetch_add(1, Ordering::Relaxed);
                let dname = format!("ArnaBubble{n}");
                let dwide = wide(&dname);
                let hdesk = CreateDesktopW(
                    dwide.as_ptr(),
                    null(),
                    null_mut(),
                    0,
                    GENERIC_ALL,
                    null_mut(),
                );
                if hdesk.is_null() {
                    return Err("could not create desktop".into());
                }

                let mut si: STARTUPINFOW = zeroed();
                si.cb = size_of::<STARTUPINFOW>() as DWORD;
                let mut deskbuf = wide(&dname);
                si.lpDesktop = deskbuf.as_mut_ptr();
                si.dwFlags = STARTF_USESHOWWINDOW;
                si.wShowWindow = SW_SHOWNORMAL;
                let mut pi: PROCESS_INFORMATION = zeroed();
                let mut cmdbuf = wide(cmd);
                let ok = CreateProcessW(
                    null(),
                    cmdbuf.as_mut_ptr(),
                    null_mut(),
                    null_mut(),
                    0,
                    0,
                    null_mut(),
                    null(),
                    &mut si,
                    &mut pi,
                );
                if ok == 0 {
                    CloseDesktop(hdesk);
                    return Err(format!("could not launch '{cmd}'"));
                }
                CloseHandle(pi.hThread);
                Ok(Bubble {
                    hdesk,
                    proc: pi.hProcess as HWND,
                    pid: pi.dwProcessId,
                    hwnd: null_mut(),
                })
            }
        }

        /// (Re)locate the app's main window on the bubble desktop. Returns true
        /// once a window is found.
        pub fn locate(&mut self) -> bool {
            unsafe {
                let mut f = Find {
                    pid: self.pid,
                    best_pid: null_mut(),
                    best_pid_area: 0,
                    best_any: null_mut(),
                    best_any_area: 0,
                };
                EnumDesktopWindows(self.hdesk, Some(enum_cb), &mut f as *mut _ as LPARAM);
                let w = if !f.best_pid.is_null() { f.best_pid } else { f.best_any };
                if !w.is_null() {
                    self.hwnd = w;
                }
                !self.hwnd.is_null()
            }
        }

        pub fn has_window(&self) -> bool {
            !self.hwnd.is_null()
        }

        /// A `Send` input handle for the current window (call after `locate`).
        pub fn input(&self) -> BubbleInput {
            BubbleInput { hwnd: self.hwnd as usize }
        }

        /// HWND of a child Edit control, if any — typed text goes here (else the
        /// window itself). Returned as usize so callers can stash it.
        pub fn edit_hwnd(&self) -> usize {
            unsafe {
                let mut found: HWND = null_mut();
                EnumChildWindows(self.hwnd, Some(child_edit_cb), &mut found as *mut _ as LPARAM);
                found as usize
            }
        }

        /// Capture the window's **client area** as tight, top-down BGRA. Returns
        /// (width, height, pixels).
        pub fn capture(&self) -> Option<(i32, i32, Vec<u8>)> {
            if self.hwnd.is_null() {
                return None;
            }
            unsafe {
                let mut rc: RECT = zeroed();
                if GetClientRect(self.hwnd, &mut rc) == 0 {
                    return None;
                }
                let (w, h) = (rc.right, rc.bottom);
                if w <= 0 || h <= 0 {
                    return None;
                }
                let screen: HDC = GetDC(null_mut());
                let mem = CreateCompatibleDC(screen);
                let bmp: HBITMAP = CreateCompatibleBitmap(screen, w, h);
                let old = SelectObject(mem, bmp as _);
                let ok = PrintWindow(self.hwnd, mem, PW_CLIENTONLY | PW_RENDERFULLCONTENT);
                SelectObject(mem, old);

                let mut bmi: BITMAPINFO = zeroed();
                bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as DWORD;
                bmi.bmiHeader.biWidth = w;
                bmi.bmiHeader.biHeight = -h;
                bmi.bmiHeader.biPlanes = 1;
                bmi.bmiHeader.biBitCount = 32;
                bmi.bmiHeader.biCompression = BI_RGB;
                let mut buf = vec![0u8; (w * h * 4) as usize];
                let got = GetDIBits(
                    screen,
                    bmp,
                    0,
                    h as UINT,
                    buf.as_mut_ptr() as _,
                    &mut bmi,
                    DIB_RGB_COLORS,
                );
                DeleteObject(bmp as _);
                DeleteDC(mem);
                ReleaseDC(null_mut(), screen);
                if ok == 0 || got == 0 {
                    return None;
                }
                Some((w, h, buf))
            }
        }

        /// Whether the launched process is still running.
        pub fn alive(&self) -> bool {
            unsafe {
                let mut code: DWORD = 0;
                if GetExitCodeProcess(self.proc as _, &mut code) == 0 {
                    return false;
                }
                code == STILL_ACTIVE
            }
        }
    }

    impl Drop for Bubble {
        fn drop(&mut self) {
            unsafe {
                TerminateProcess(self.proc as _, 0);
                CloseHandle(self.proc as _);
                CloseDesktop(self.hdesk);
            }
        }
    }

    // The HDESK/HWND are only ever touched on the capture thread that owns the
    // Bubble; mark Send so it can be moved into that thread's closure.
    unsafe impl Send for Bubble {}
}

// ---------------------------------------------------------------------------

#[cfg(not(windows))]
mod stub {
    #[derive(Clone, Copy)]
    pub struct BubbleInput;
    impl BubbleInput {
        pub fn mouse_move(&self, _nx: f64, _ny: f64) {}
        pub fn mouse_button(&self, _nx: f64, _ny: f64, _b: u8, _d: bool) {}
        pub fn wheel(&self, _nx: f64, _ny: f64, _d: i32) {}
        pub fn key_char(&self, _c: char) {}
        pub fn key_vk(&self, _vk: u16, _down: bool) {}
    }

    pub struct Bubble;
    impl Bubble {
        pub fn launch(_cmd: &str) -> Result<Self, String> {
            Err("app bubbles are Windows-only".into())
        }
        pub fn locate(&mut self) -> bool {
            false
        }
        pub fn has_window(&self) -> bool {
            false
        }
        pub fn input(&self) -> BubbleInput {
            BubbleInput
        }
        pub fn edit_hwnd(&self) -> usize {
            0
        }
        pub fn capture(&self) -> Option<(i32, i32, Vec<u8>)> {
            None
        }
        pub fn alive(&self) -> bool {
            false
        }
    }
}
