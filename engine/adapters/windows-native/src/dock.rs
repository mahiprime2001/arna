//! The **workspace dock** — a small, floating, Mac-style bar at the bottom of a
//! workspace desktop. A fresh Windows desktop has no taskbar/Start, so the dock
//! is how you (re)open the workspace's apps.
//!
//! It shows the workspace's chosen apps as **persistent pinned tiles** (with each
//! app's real icon): the tile stays whether the app is open or closed, so closing
//! an app never makes it vanish — click the tile to (re)launch it. Plus a "+"
//! extra-browser launcher, a "My" (imported profile) tile, and an accent "Leave"
//! tile (set apart) that switches back to your real desktop — the way out. It
//! floats (reserves no screen space) and is owner-drawn (raw GDI).

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use super::{create_process_flags, create_process_on_desktop, wide};

type Hwnd = *mut c_void;
type Hdesk = *mut c_void;
type Handle = *mut c_void;
type WndProc = extern "system" fn(Hwnd, u32, usize, isize) -> isize;

const GENERIC_ALL: u32 = 0x1000_0000;
const WS_POPUP: u32 = 0x8000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_EX_TOPMOST: u32 = 0x0000_0008;
const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
const WS_EX_LAYERED: u32 = 0x0008_0000;
const LWA_ALPHA: u32 = 2;
const SM_CXSCREEN: i32 = 0;
const SM_CYSCREEN: i32 = 1;
const WM_PAINT: u32 = 0x000F;
const WM_TIMER: u32 = 0x0113;
const WM_DESTROY: u32 = 0x0002;
const WM_CLOSE: u32 = 0x0010;
const WM_LBUTTONDOWN: u32 = 0x0201;
const IDC_ARROW: u32 = 32512;
const DI_NORMAL: u32 = 0x0003;
const NULL_PEN: i32 = 8;
const TRANSPARENT: i32 = 1;
const DT_CENTER: u32 = 0x0000_0001;
const DT_VCENTER: u32 = 0x0000_0004;
const DT_SINGLELINE: u32 = 0x0000_0020;
const SWP_NOACTIVATE: u32 = 0x0010;
const HWND_TOPMOST: isize = -1;
const TIMER_ID: usize = 1;

// layout
const PAD: i32 = 12;
const TILE: i32 = 56;
const GAP: i32 = 8;
const ICON: i32 = 36;
const HEIGHT: i32 = TILE + 20;

/// A workspace app pinned to the dock: always shown, (re)launched on click.
pub(crate) struct PinnedApp {
    pub label: String,
    pub exe: String, // for the tile icon
    pub cmdline: String,
    pub flags: u32,
}

#[derive(Clone)]
enum TileKind {
    Pinned(usize),
    NewBrowser,
    MyBrowser,
    Exit,
}

struct Tile {
    kind: TileKind,
    x: i32,
    w: i32,
}

#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct PaintStruct {
    hdc: Handle,
    erase: i32,
    rc_paint: Rect,
    restore: i32,
    inc_update: i32,
    reserved: [u8; 32],
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

#[link(name = "user32")]
extern "system" {
    fn RegisterClassW(c: *const WndClassW) -> u16;
    fn CreateWindowExW(
        ex: u32, class: *const u16, name: *const u16, style: u32,
        x: i32, y: i32, w: i32, h: i32,
        parent: Hwnd, menu: Handle, inst: Handle, param: *const c_void,
    ) -> Hwnd;
    fn DefWindowProcW(h: Hwnd, m: u32, w: usize, l: isize) -> isize;
    fn TranslateMessage(m: *const Msg) -> i32;
    fn DispatchMessageW(m: *const Msg) -> isize;
    fn PostQuitMessage(code: i32);
    fn GetMessageW(m: *mut Msg, h: Hwnd, min: u32, max: u32) -> i32;
    fn PostMessageW(h: Hwnd, m: u32, w: usize, l: isize) -> i32;
    fn SetTimer(h: Hwnd, id: usize, ms: u32, p: *const c_void) -> usize;
    fn KillTimer(h: Hwnd, id: usize) -> i32;
    fn GetSystemMetrics(i: i32) -> i32;
    fn LoadCursorW(inst: Handle, name: u32) -> Handle;
    fn SetWindowPos(h: Hwnd, after: isize, x: i32, y: i32, w: i32, ht: i32, flags: u32) -> i32;
    fn SetWindowRgn(h: Hwnd, rgn: Handle, redraw: i32) -> i32;
    fn InvalidateRect(h: Hwnd, r: *const Rect, erase: i32) -> i32;
    fn SetLayeredWindowAttributes(h: Hwnd, key: u32, alpha: u8, flags: u32) -> i32;
    fn BeginPaint(h: Hwnd, ps: *mut PaintStruct) -> Handle;
    fn EndPaint(h: Hwnd, ps: *const PaintStruct) -> i32;
    fn GetClientRect(h: Hwnd, r: *mut Rect) -> i32;
    fn FillRect(hdc: Handle, r: *const Rect, brush: Handle) -> i32;
    fn DrawTextW(hdc: Handle, text: *const u16, count: i32, r: *mut Rect, fmt: u32) -> i32;
    fn DrawIconEx(hdc: Handle, x: i32, y: i32, icon: Handle, cx: i32, cy: i32, step: u32, brush: Handle, flags: u32) -> i32;
    fn DestroyIcon(icon: Handle) -> i32;
    fn OpenDesktopW(name: *const u16, flags: u32, inherit: i32, access: u32) -> Hdesk;
    fn SwitchDesktop(d: Hdesk) -> i32;
    fn SetThreadDesktop(d: Hdesk) -> i32;
    fn CloseDesktop(d: Hdesk) -> i32;
}

#[link(name = "shell32")]
extern "system" {
    fn ExtractIconW(inst: Handle, path: *const u16, index: u32) -> Handle;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateSolidBrush(color: u32) -> Handle;
    fn CreateRoundRectRgn(l: i32, t: i32, r: i32, b: i32, w: i32, h: i32) -> Handle;
    fn RoundRect(hdc: Handle, l: i32, t: i32, r: i32, b: i32, w: i32, h: i32) -> i32;
    fn SelectObject(hdc: Handle, obj: Handle) -> Handle;
    fn DeleteObject(obj: Handle) -> i32;
    fn GetStockObject(i: i32) -> Handle;
    fn SetBkMode(hdc: Handle, mode: i32) -> i32;
    fn SetTextColor(hdc: Handle, color: u32) -> u32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Handle;
}

fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

struct DockCtx {
    profiles: PathBuf,
    home: String,
    desktop_name: String,
    pinned: Vec<PinnedApp>,
    tiles: Vec<Tile>,
    counter: u32,
}

thread_local! {
    static CTX: RefCell<Option<DockCtx>> = const { RefCell::new(None) };
}

static REGISTRY: Mutex<Option<HashMap<String, isize>>> = Mutex::new(None);
static CLASS_SEQ: AtomicU32 = AtomicU32::new(0);

pub(crate) fn close(desktop_name: &str) {
    let hwnd = REGISTRY
        .lock()
        .ok()
        .and_then(|g| g.as_ref().and_then(|m| m.get(desktop_name).copied()));
    if let Some(h) = hwnd {
        unsafe { PostMessageW(h as Hwnd, WM_CLOSE, 0, 0); }
    }
}

/// Rebuild the tile list (pinned apps + launchers + Leave) and re-lay-out / resize.
fn refresh(hwnd: Hwnd) {
    CTX.with(|c| {
        let mut b = c.borrow_mut();
        let Some(ctx) = &mut *b else { return };

        let mut kinds: Vec<TileKind> = (0..ctx.pinned.len()).map(TileKind::Pinned).collect();
        kinds.push(TileKind::NewBrowser);
        if ctx.profiles.join("imported").exists() {
            kinds.push(TileKind::MyBrowser);
        }
        kinds.push(TileKind::Exit);

        let mut tiles = Vec::new();
        let mut x = PAD;
        for k in kinds {
            if matches!(k, TileKind::Exit) {
                x += GAP * 2; // set the Leave button apart
            }
            tiles.push(Tile { kind: k, x, w: TILE });
            x += TILE + GAP;
        }
        let width = x - GAP + PAD;
        ctx.tiles = tiles;

        unsafe {
            let sw = GetSystemMetrics(SM_CXSCREEN);
            let sh = GetSystemMetrics(SM_CYSCREEN);
            let px = (sw - width) / 2;
            let py = sh - HEIGHT - 28;
            SetWindowPos(hwnd, HWND_TOPMOST, px, py, width, HEIGHT, SWP_NOACTIVATE);
            let rgn = CreateRoundRectRgn(0, 0, width + 1, HEIGHT + 1, 22, 22);
            SetWindowRgn(hwnd, rgn, 1);
            InvalidateRect(hwnd, std::ptr::null(), 1);
        }
    });
}

fn draw_glyph(hdc: Handle, x: i32, text: &str) {
    let mut r = Rect { left: x, top: 0, right: x + TILE, bottom: HEIGHT };
    let w = wide(text);
    unsafe {
        SetTextColor(hdc, rgb(235, 235, 240));
        DrawTextW(hdc, w.as_ptr(), -1, &mut r, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    }
}

/// Draw a pinned app's exe icon (or its label if the icon can't be extracted).
fn draw_pinned(hdc: Handle, x: i32, app: &PinnedApp) {
    let icon_off = (TILE - ICON) / 2;
    unsafe {
        let icon = if app.exe.is_empty() {
            std::ptr::null_mut()
        } else {
            ExtractIconW(std::ptr::null_mut(), wide(&app.exe).as_ptr(), 0)
        };
        // ExtractIcon returns 1 (as a handle) when the file has no icons.
        if !icon.is_null() && icon as isize != 1 {
            DrawIconEx(hdc, x + icon_off, 10 + icon_off, icon, ICON, ICON, 0, std::ptr::null_mut(), DI_NORMAL);
            DestroyIcon(icon);
        } else {
            draw_glyph(hdc, x, &app.label);
        }
    }
}

fn paint(hwnd: Hwnd) {
    CTX.with(|c| {
        let b = c.borrow();
        let Some(ctx) = &*b else { return };
        unsafe {
            let mut ps: PaintStruct = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            let mut rc: Rect = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);

            let bg = CreateSolidBrush(rgb(28, 28, 34));
            FillRect(hdc, &rc, bg);
            DeleteObject(bg);

            SetBkMode(hdc, TRANSPARENT);
            let tile_brush = CreateSolidBrush(rgb(54, 54, 64));
            // The Leave button gets an accent colour so it's obviously the way out.
            let leave_brush = CreateSolidBrush(rgb(64, 120, 205));
            let null_pen = GetStockObject(NULL_PEN);
            SelectObject(hdc, null_pen);
            let old_brush = SelectObject(hdc, tile_brush);

            for t in &ctx.tiles {
                SelectObject(hdc, if matches!(t.kind, TileKind::Exit) { leave_brush } else { tile_brush });
                RoundRect(hdc, t.x, 10, t.x + TILE, 10 + TILE, 14, 14);
                match &t.kind {
                    TileKind::Pinned(i) => draw_pinned(hdc, t.x, &ctx.pinned[*i]),
                    TileKind::NewBrowser => draw_glyph(hdc, t.x, "+"),
                    TileKind::MyBrowser => draw_glyph(hdc, t.x, "My"),
                    TileKind::Exit => draw_glyph(hdc, t.x, "Leave"),
                }
            }
            SelectObject(hdc, old_brush);
            DeleteObject(tile_brush);
            DeleteObject(leave_brush);
            EndPaint(hwnd, &ps);
        }
    });
}

fn on_click(x: i32) {
    CTX.with(|c| {
        let mut b = c.borrow_mut();
        let Some(ctx) = &mut *b else { return };
        let hit = ctx
            .tiles
            .iter()
            .find(|t| x >= t.x && x < t.x + t.w)
            .map(|t| t.kind.clone());
        let Some(tile) = hit else { return };
        match tile {
            TileKind::Pinned(i) => {
                let app = &ctx.pinned[i];
                let _ = create_process_flags(&ctx.desktop_name, &app.cmdline, &ctx.home, app.flags);
            }
            TileKind::NewBrowser => {
                let profile = ctx.profiles.join(format!("dock-{}", ctx.counter));
                ctx.counter += 1;
                let _ = std::fs::create_dir_all(&profile);
                // A fresh extra browser reuses the workspace's first pinned browser
                // exe if there is one; else the label falls back.
                if let Some(exe) = ctx.pinned.iter().find(|a| a.label == "Browser").map(|a| a.exe.clone()) {
                    let cmd = format!(
                        "\"{}\" --user-data-dir=\"{}\" --no-first-run --no-default-browser-check --new-window about:blank",
                        exe, profile.display()
                    );
                    let _ = create_process_on_desktop(&ctx.desktop_name, &cmd, &ctx.home);
                }
            }
            TileKind::MyBrowser => {
                let imported = ctx.profiles.join("imported");
                if imported.exists() {
                    if let Some(exe) = ctx.pinned.iter().find(|a| a.label == "Browser").map(|a| a.exe.clone()) {
                        let cmd = format!(
                            "\"{}\" --user-data-dir=\"{}\" --no-first-run --no-default-browser-check --new-window",
                            exe, imported.display()
                        );
                        let _ = create_process_on_desktop(&ctx.desktop_name, &cmd, &ctx.home);
                    }
                }
            }
            TileKind::Exit => unsafe {
                let def = OpenDesktopW(wide("Default").as_ptr(), 0, 0, GENERIC_ALL);
                if !def.is_null() {
                    SwitchDesktop(def);
                    CloseDesktop(def);
                }
            },
        }
    });
}

extern "system" fn wnd_proc(hwnd: Hwnd, msg: u32, w: usize, l: isize) -> isize {
    match msg {
        WM_PAINT => {
            paint(hwnd);
            0
        }
        WM_LBUTTONDOWN => {
            on_click((l & 0xFFFF) as i16 as i32);
            0
        }
        WM_TIMER => {
            refresh(hwnd);
            0
        }
        WM_DESTROY => {
            unsafe { KillTimer(hwnd, TIMER_ID); }
            CTX.with(|c| {
                if let Some(ctx) = &*c.borrow() {
                    if let Ok(mut g) = REGISTRY.lock() {
                        if let Some(m) = g.as_mut() {
                            m.remove(&ctx.desktop_name);
                        }
                    }
                }
            });
            unsafe { PostQuitMessage(0); }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, w, l) },
    }
}

/// Launch the dock on `desktop_name`, pinning `pinned` apps. Non-blocking: own
/// thread + message loop until the workspace is destroyed.
pub(crate) fn spawn_dock(
    desktop_name: String,
    profiles: PathBuf,
    home: String,
    pinned: Vec<PinnedApp>,
) {
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
            hbr_background: std::ptr::null_mut(),
            menu_name: std::ptr::null(),
            class_name: class_w.as_ptr(),
        };
        RegisterClassW(&wc);

        let main = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
            class_w.as_ptr(),
            wide("WSE").as_ptr(),
            WS_POPUP | WS_VISIBLE,
            0, 0, 200, HEIGHT,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinst,
            std::ptr::null(),
        );
        if main.is_null() {
            CloseDesktop(hdesk);
            return;
        }
        SetLayeredWindowAttributes(main, 0, 240, LWA_ALPHA);

        CTX.with(|c| {
            *c.borrow_mut() = Some(DockCtx {
                profiles,
                home,
                desktop_name: desktop_name.clone(),
                pinned,
                tiles: Vec::new(),
                counter: 0,
            });
        });
        {
            let mut g = REGISTRY.lock().unwrap();
            g.get_or_insert_with(HashMap::new)
                .insert(desktop_name.clone(), main as isize);
        }
        refresh(main);
        SetTimer(main, TIMER_ID, 1500, std::ptr::null());

        let mut msg: Msg = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        CloseDesktop(hdesk);
    });
}
