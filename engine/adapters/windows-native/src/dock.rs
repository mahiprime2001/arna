//! The **workspace dock** — a thin always-on-top bar on the left edge of a
//! workspace desktop. A fresh Windows desktop has NO taskbar or Start menu, so
//! without this you can't get a minimized window back or launch anything. The
//! dock gives the workspace its own tiny shell: launch a browser, and Focus /
//! Minimize / Close / Leave the windows running there. It runs on a thread whose
//! desktop is the workspace desktop, with its own Win32 message loop.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use super::{create_process_on_desktop, wide};

type Hwnd = *mut c_void;
type Hdesk = *mut c_void;
type Handle = *mut c_void;
type WndProc = extern "system" fn(Hwnd, u32, usize, isize) -> isize;
type EnumProc = extern "system" fn(Hwnd, isize) -> i32;

const GENERIC_ALL: u32 = 0x1000_0000;
const WS_POPUP: u32 = 0x8000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_CHILD: u32 = 0x4000_0000;
const WS_BORDER: u32 = 0x0080_0000;
const WS_VSCROLL: u32 = 0x0020_0000;
const WS_EX_TOPMOST: u32 = 0x0000_0008;
const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
const BS_PUSHBUTTON: u32 = 0;
const LBS_NOTIFY: u32 = 0x0001;
const SW_RESTORE: i32 = 9;
const SW_MINIMIZE: i32 = 6;
const SM_CYSCREEN: i32 = 1;
const WM_COMMAND: u32 = 0x0111;
const WM_TIMER: u32 = 0x0113;
const WM_DESTROY: u32 = 0x0002;
const WM_CLOSE: u32 = 0x0010;
const LB_ADDSTRING: u32 = 0x0180;
const LB_RESETCONTENT: u32 = 0x0184;
const LB_GETCURSEL: u32 = 0x0188;
const LB_SETITEMDATA: u32 = 0x019A;
const LB_GETITEMDATA: u32 = 0x0199;
const IDC_ARROW: u32 = 32512;

// control ids
const ID_NEW_BROWSER: u32 = 1001;
const ID_FOCUS: u32 = 1002;
const ID_MINIMIZE: u32 = 1003;
const ID_CLOSE: u32 = 1004;
const ID_LEAVE: u32 = 1005;
const ID_LIST: u32 = 2000;
const TIMER_ID: usize = 1;

#[repr(C)]
struct WndClassW {
    style: u32,
    wnd_proc: WndProc,
    cb_cls_extra: i32,
    cb_wnd_extra: i32,
    h_instance: Handle,
    h_icon: Handle,
    h_cursor: Handle,
    hbr_background: Handle,
    menu_name: *const u16,
    class_name: *const u16,
}

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
    fn RegisterClassW(c: *const WndClassW) -> u16;
    fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: Hwnd,
        menu: Handle,
        inst: Handle,
        param: *const c_void,
    ) -> Hwnd;
    fn DefWindowProcW(h: Hwnd, m: u32, w: usize, l: isize) -> isize;
    fn ShowWindow(h: Hwnd, cmd: i32) -> i32;
    fn TranslateMessage(m: *const Msg) -> i32;
    fn DispatchMessageW(m: *const Msg) -> isize;
    fn PostQuitMessage(code: i32);
    fn GetMessageW(m: *mut Msg, h: Hwnd, min: u32, max: u32) -> i32;
    fn SendMessageW(h: Hwnd, m: u32, w: usize, l: isize) -> isize;
    fn PostMessageW(h: Hwnd, m: u32, w: usize, l: isize) -> i32;
    fn SetTimer(h: Hwnd, id: usize, ms: u32, proc_: *const c_void) -> usize;
    fn KillTimer(h: Hwnd, id: usize) -> i32;
    fn GetSystemMetrics(i: i32) -> i32;
    fn LoadCursorW(inst: Handle, name: u32) -> Handle;
    fn SetForegroundWindow(h: Hwnd) -> i32;
    fn EnumDesktopWindows(d: Hdesk, cb: EnumProc, l: isize) -> i32;
    fn IsWindowVisible(h: Hwnd) -> i32;
    fn GetWindowTextLengthW(h: Hwnd) -> i32;
    fn GetWindowTextW(h: Hwnd, s: *mut u16, n: i32) -> i32;
    fn OpenDesktopW(name: *const u16, flags: u32, inherit: i32, access: u32) -> Hdesk;
    fn SwitchDesktop(d: Hdesk) -> i32;
    fn SetThreadDesktop(d: Hdesk) -> i32;
    fn CloseDesktop(d: Hdesk) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Handle;
}

struct DockCtx {
    browser: PathBuf,
    profiles: PathBuf,
    home: String,
    desktop_name: String,
    self_hwnd: isize,
    listbox: isize,
    counter: u32,
}

thread_local! {
    static CTX: RefCell<Option<DockCtx>> = const { RefCell::new(None) };
}

// desktop_name -> dock main hwnd, so destroy() can close the dock cleanly.
static REGISTRY: Mutex<Option<HashMap<String, isize>>> = Mutex::new(None);
static CLASS_SEQ: AtomicU32 = AtomicU32::new(0);

/// Close the dock on a desktop (called from the adapter's destroy).
pub(crate) fn close(desktop_name: &str) {
    let hwnd = REGISTRY
        .lock()
        .ok()
        .and_then(|g| g.as_ref().and_then(|m| m.get(desktop_name).copied()));
    if let Some(h) = hwnd {
        unsafe {
            PostMessageW(h as Hwnd, WM_CLOSE, 0, 0);
        }
    }
}

/// All titled, visible top-level windows on the current thread's desktop.
fn windows_here() -> Vec<(isize, String)> {
    extern "system" fn cb(h: Hwnd, l: isize) -> i32 {
        unsafe {
            let out = &mut *(l as *mut Vec<(isize, String)>);
            if IsWindowVisible(h) == 0 {
                return 1;
            }
            let len = GetWindowTextLengthW(h);
            if len <= 0 {
                return 1;
            }
            let mut buf = vec![0u16; len as usize + 1];
            let n = GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32);
            out.push((h as isize, String::from_utf16_lossy(&buf[..n.max(0) as usize])));
        }
        1
    }
    let mut out: Vec<(isize, String)> = Vec::new();
    unsafe {
        EnumDesktopWindows(std::ptr::null_mut(), cb, &mut out as *mut _ as isize);
    }
    out
}

fn refresh() {
    CTX.with(|c| {
        if let Some(ctx) = &*c.borrow() {
            let list = ctx.listbox as Hwnd;
            unsafe {
                SendMessageW(list, LB_RESETCONTENT, 0, 0);
            }
            for (h, title) in windows_here() {
                if h == ctx.self_hwnd {
                    continue; // don't list the dock itself
                }
                let w = wide(&title);
                let idx = unsafe { SendMessageW(list, LB_ADDSTRING, 0, w.as_ptr() as isize) };
                if idx >= 0 {
                    unsafe {
                        SendMessageW(list, LB_SETITEMDATA, idx as usize, h);
                    }
                }
            }
        }
    });
}

fn selected(list: isize) -> Option<isize> {
    unsafe {
        let idx = SendMessageW(list as Hwnd, LB_GETCURSEL, 0, 0);
        if idx < 0 {
            None
        } else {
            Some(SendMessageW(list as Hwnd, LB_GETITEMDATA, idx as usize, 0))
        }
    }
}

fn handle_command(id: u32) {
    CTX.with(|c| {
        let mut b = c.borrow_mut();
        let Some(ctx) = &mut *b else { return };
        match id {
            ID_NEW_BROWSER => {
                let profile = ctx.profiles.join(format!("dock-{}", ctx.counter));
                ctx.counter += 1;
                let _ = std::fs::create_dir_all(&profile);
                let cmd = format!(
                    "\"{}\" --user-data-dir=\"{}\" --no-first-run --no-default-browser-check \
                     --new-window about:blank",
                    ctx.browser.display(),
                    profile.display()
                );
                let _ = create_process_on_desktop(&ctx.desktop_name, &cmd, &ctx.home);
            }
            ID_FOCUS | ID_MINIMIZE | ID_CLOSE => {
                if let Some(h) = selected(ctx.listbox) {
                    let hw = h as Hwnd;
                    unsafe {
                        match id {
                            ID_FOCUS => {
                                ShowWindow(hw, SW_RESTORE);
                                SetForegroundWindow(hw);
                            }
                            ID_MINIMIZE => {
                                ShowWindow(hw, SW_MINIMIZE);
                            }
                            ID_CLOSE => {
                                PostMessageW(hw, WM_CLOSE, 0, 0);
                            }
                            _ => {}
                        }
                    }
                }
            }
            ID_LEAVE => unsafe {
                let def = OpenDesktopW(wide("Default").as_ptr(), 0, 0, GENERIC_ALL);
                if !def.is_null() {
                    SwitchDesktop(def);
                    CloseDesktop(def);
                }
            },
            _ => {}
        }
    });
}

extern "system" fn wnd_proc(hwnd: Hwnd, msg: u32, w: usize, l: isize) -> isize {
    match msg {
        WM_COMMAND => {
            handle_command((w & 0xFFFF) as u32);
            0
        }
        WM_TIMER => {
            refresh();
            0
        }
        WM_DESTROY => {
            unsafe {
                KillTimer(hwnd, TIMER_ID);
            }
            CTX.with(|c| {
                if let Some(ctx) = &*c.borrow() {
                    if let Ok(mut g) = REGISTRY.lock() {
                        if let Some(m) = g.as_mut() {
                            m.remove(&ctx.desktop_name);
                        }
                    }
                }
            });
            unsafe {
                PostQuitMessage(0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, w, l) },
    }
}

/// Launch the dock on `desktop_name`. Non-blocking: it lives on its own thread
/// with a message loop until the workspace is destroyed.
pub(crate) fn spawn_dock(desktop_name: String, browser: PathBuf, profiles: PathBuf, home: String) {
    std::thread::spawn(move || unsafe {
        let hdesk = OpenDesktopW(wide(&desktop_name).as_ptr(), 0, 0, GENERIC_ALL);
        if hdesk.is_null() {
            return;
        }
        SetThreadDesktop(hdesk);
        let hinst = GetModuleHandleW(std::ptr::null());
        let class = format!("WseDock{}", CLASS_SEQ.fetch_add(1, Ordering::Relaxed));
        let class_w = wide(&class);
        let wc = WndClassW {
            style: 0,
            wnd_proc,
            cb_cls_extra: 0,
            cb_wnd_extra: 0,
            h_instance: hinst,
            h_icon: std::ptr::null_mut(),
            h_cursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            hbr_background: 16 as Handle, // COLOR_BTNFACE+1
            menu_name: std::ptr::null(),
            class_name: class_w.as_ptr(),
        };
        RegisterClassW(&wc);

        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        let main = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_w.as_ptr(),
            wide("WSE").as_ptr(),
            WS_POPUP | WS_VISIBLE | WS_BORDER,
            0,
            0,
            184,
            screen_h,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinst,
            std::ptr::null(),
        );
        if main.is_null() {
            CloseDesktop(hdesk);
            return;
        }

        let button = |text: &str, id: u32, y: i32, h: i32| {
            CreateWindowExW(
                0,
                wide("BUTTON").as_ptr(),
                wide(text).as_ptr(),
                WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
                8,
                y,
                168,
                h,
                main,
                (id as usize) as Handle,
                hinst,
                std::ptr::null(),
            );
        };
        button("+ New Browser", ID_NEW_BROWSER, 10, 34);
        let list = CreateWindowExW(
            0,
            wide("LISTBOX").as_ptr(),
            std::ptr::null(),
            WS_CHILD | WS_VISIBLE | WS_BORDER | WS_VSCROLL | LBS_NOTIFY,
            8,
            52,
            168,
            screen_h - 268,
            main,
            (ID_LIST as usize) as Handle,
            hinst,
            std::ptr::null(),
        );
        button("Focus", ID_FOCUS, screen_h - 208, 30);
        button("Minimize", ID_MINIMIZE, screen_h - 174, 30);
        button("Close app", ID_CLOSE, screen_h - 140, 30);
        button("Leave (Ctrl+Alt+Q)", ID_LEAVE, screen_h - 98, 34);

        CTX.with(|c| {
            *c.borrow_mut() = Some(DockCtx {
                browser,
                profiles,
                home,
                desktop_name: desktop_name.clone(),
                self_hwnd: main as isize,
                listbox: list as isize,
                counter: 0,
            });
        });
        {
            let mut g = REGISTRY.lock().unwrap();
            g.get_or_insert_with(HashMap::new)
                .insert(desktop_name.clone(), main as isize);
        }
        SetTimer(main, TIMER_ID, 1000, std::ptr::null());
        refresh();

        let mut msg: Msg = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        CloseDesktop(hdesk);
    });
}
