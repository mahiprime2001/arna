//! WSE Desktop — the visual launcher. A native Win32 window over the same engine
//! as the CLI, but fully owner-drawn (flat, dark, modern) — no default gray
//! controls. Dark title bar, accent buttons with hover, rounded workspace cards.
#![windows_subsystem = "windows"]

use std::cell::RefCell;
use std::collections::HashSet;
use std::ffi::c_void;

use wse_adapter_windows_native::{enter_workspace_desktop, spawn_workspace_dock, WindowsNativeAdapter};
use wse_common::{ApplicationDescriptor, Persistence, WorkspaceId, WorkspaceState};
use wse_engine::{Engine, WorkspaceConfig};

type Hwnd = *mut c_void;
type Handle = *mut c_void;
type WndProc = extern "system" fn(Hwnd, u32, usize, isize) -> isize;

const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_CHILD: u32 = 0x4000_0000;
const ES_AUTOHSCROLL: u32 = 0x0080;
const WM_CREATE: u32 = 0x0001;
const WM_PAINT: u32 = 0x000F;
const WM_TIMER: u32 = 0x0113;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_DESTROY: u32 = 0x0002;
const WM_SETFONT: u32 = 0x0030;
const WM_GETTEXT: u32 = 0x000D;
const WM_SETTEXT: u32 = 0x000C;
const WM_CTLCOLOREDIT: u32 = 0x0133;
const IDC_ARROW: u32 = 32512;
const SW_SHOW: i32 = 5;
const SM_CXSCREEN: i32 = 0;
const SM_CYSCREEN: i32 = 1;
const TRANSPARENT: i32 = 1;
const NULL_PEN: i32 = 8;
const DT_LEFT: u32 = 0;
const DT_CENTER: u32 = 1;
const DT_VCENTER: u32 = 4;
const DT_SINGLELINE: u32 = 0x20;

// action ids
const ID_CREATE: u32 = 1;
const ID_ENTER: u32 = 2;
const ID_LAUNCH: u32 = 3;
const ID_SUSPEND: u32 = 4;
const ID_DESTROY: u32 = 5;

// layout
const W: i32 = 580;
const H: i32 = 700;
const M: i32 = 20;
const LIST_TOP: i32 = 130;
const CARD_H: i32 = 68;
const CARD_GAP: i32 = 12;

// palette
fn c_bg() -> u32 { rgb(20, 21, 28) }
fn c_card() -> u32 { rgb(32, 34, 44) }
fn c_card_sel() -> u32 { rgb(42, 48, 82) }
fn c_card_hover() -> u32 { rgb(40, 42, 54) }
fn c_input() -> u32 { rgb(36, 38, 48) }
fn c_accent() -> u32 { rgb(99, 102, 241) }
fn c_accent_hi() -> u32 { rgb(124, 126, 255) }
fn c_surface() -> u32 { rgb(44, 46, 58) }
fn c_surface_hi() -> u32 { rgb(58, 60, 74) }
fn c_danger() -> u32 { rgb(120, 52, 66) }
fn c_danger_hi() -> u32 { rgb(150, 64, 80) }
fn c_text() -> u32 { rgb(233, 233, 241) }
fn c_sub() -> u32 { rgb(150, 152, 170) }

#[repr(C)]
struct Rect { left: i32, top: i32, right: i32, bottom: i32 }
#[repr(C)]
struct PaintStruct { hdc: Handle, erase: i32, rc: Rect, restore: i32, inc: i32, rgb: [u8; 32] }
#[repr(C)]
struct Msg { hwnd: Hwnd, message: u32, w: usize, l: isize, time: u32, x: i32, y: i32 }
#[repr(C)]
struct WndClassW {
    style: u32, wnd_proc: WndProc, cls_extra: i32, wnd_extra: i32,
    inst: Handle, icon: Handle, cursor: Handle, bg: Handle, menu: *const u16, class: *const u16,
}

#[link(name = "user32")]
extern "system" {
    fn RegisterClassW(c: *const WndClassW) -> u16;
    fn CreateWindowExW(ex: u32, class: *const u16, name: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32, parent: Hwnd, menu: Handle, inst: Handle, p: *const c_void) -> Hwnd;
    fn DefWindowProcW(h: Hwnd, m: u32, w: usize, l: isize) -> isize;
    fn ShowWindow(h: Hwnd, c: i32) -> i32;
    fn UpdateWindow(h: Hwnd) -> i32;
    fn GetMessageW(m: *mut Msg, h: Hwnd, a: u32, b: u32) -> i32;
    fn TranslateMessage(m: *const Msg) -> i32;
    fn DispatchMessageW(m: *const Msg) -> isize;
    fn PostQuitMessage(c: i32);
    fn SendMessageW(h: Hwnd, m: u32, w: usize, l: isize) -> isize;
    fn SetTimer(h: Hwnd, id: usize, ms: u32, p: *const c_void) -> usize;
    fn LoadCursorW(i: Handle, n: u32) -> Handle;
    fn GetSystemMetrics(i: i32) -> i32;
    fn BeginPaint(h: Hwnd, ps: *mut PaintStruct) -> Handle;
    fn EndPaint(h: Hwnd, ps: *const PaintStruct) -> i32;
    fn GetClientRect(h: Hwnd, r: *mut Rect) -> i32;
    fn FillRect(dc: Handle, r: *const Rect, b: Handle) -> i32;
    fn InvalidateRect(h: Hwnd, r: *const Rect, e: i32) -> i32;
    fn DrawTextW(dc: Handle, t: *const u16, n: i32, r: *mut Rect, f: u32) -> i32;
    fn SetFocus(h: Hwnd) -> Hwnd;
}
#[link(name = "gdi32")]
extern "system" {
    fn CreateSolidBrush(c: u32) -> Handle;
    fn RoundRect(dc: Handle, l: i32, t: i32, r: i32, b: i32, w: i32, h: i32) -> i32;
    fn SelectObject(dc: Handle, o: Handle) -> Handle;
    fn DeleteObject(o: Handle) -> i32;
    fn GetStockObject(i: i32) -> Handle;
    fn SetBkMode(dc: Handle, m: i32) -> i32;
    fn SetBkColor(dc: Handle, c: u32) -> u32;
    fn SetTextColor(dc: Handle, c: u32) -> u32;
    fn CreateFontW(h: i32, w: i32, e: i32, o: i32, wt: i32, it: u32, un: u32, st: u32, cs: u32, op: u32, cp: u32, q: u32, p: u32, face: *const u16) -> Handle;
}
#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(n: *const u16) -> Handle;
}
#[link(name = "dwmapi")]
extern "system" {
    fn DwmSetWindowAttribute(h: Hwnd, attr: u32, val: *const c_void, size: u32) -> i32;
}

fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() }
fn rgb(r: u8, g: u8, b: u8) -> u32 { (r as u32) | ((g as u32) << 8) | ((b as u32) << 16) }

fn catalog() -> Vec<ApplicationDescriptor> {
    vec![
        ApplicationDescriptor::new("browser", "Browser"),
        ApplicationDescriptor::new("terminal", "Terminal"),
        ApplicationDescriptor::new("editor", "Editor"),
    ]
}

#[derive(Clone, Copy, PartialEq)]
enum Kind { Primary, Secondary, Danger }

struct Btn { id: u32, label: &'static str, x: i32, y: i32, w: i32, h: i32, kind: Kind }

struct App {
    engine: Engine<WindowsNativeAdapter>,
    items: Vec<(String, WorkspaceId)>,
    selected: Option<usize>,
    docked: HashSet<WorkspaceId>,
    edit: isize,
    font: isize,
    font_h: isize,
    font_s: isize,
    input_brush: isize,
    buttons: Vec<Btn>,
    hover_btn: Option<u32>,
    hover_card: Option<usize>,
    next: u32,
}

thread_local! { static APP: RefCell<Option<App>> = const { RefCell::new(None) }; }

fn card_top(i: usize) -> i32 { LIST_TOP + i as i32 * (CARD_H + CARD_GAP) }

fn create_workspace() {
    APP.with(|a| {
        let mut b = a.borrow_mut();
        let Some(app) = &mut *b else { return };
        let mut buf = [0u16; 128];
        let n = unsafe { SendMessageW(app.edit as Hwnd, WM_GETTEXT, 128, buf.as_mut_ptr() as isize) };
        let mut name = String::from_utf16_lossy(&buf[..n.max(0) as usize]).trim().to_string();
        if name.is_empty() {
            app.next += 1;
            name = format!("Workspace {}", app.next);
        }
        let cfg = WorkspaceConfig::new(&name, Persistence::Temporary, catalog());
        if let Ok(id) = app.engine.create_workspace(cfg) {
            app.items.push((name, id));
            app.selected = Some(app.items.len() - 1);
        }
        unsafe { SendMessageW(app.edit as Hwnd, WM_SETTEXT, 0, wide("").as_ptr() as isize); }
    });
}

fn act(id: u32) {
    APP.with(|a| {
        let mut b = a.borrow_mut();
        let Some(app) = &mut *b else { return };
        if id == ID_CREATE {
            drop(b);
            create_workspace();
            return;
        }
        let Some(i) = app.selected else { return };
        let Some((_, wid)) = app.items.get(i).cloned() else { return };
        match id {
            ID_ENTER => {
                if app.docked.insert(wid.clone()) { spawn_workspace_dock(&wid); }
                enter_workspace_desktop(&wid);
            }
            ID_LAUNCH => {
                if app.engine.state(&wid) != Some(WorkspaceState::Running) { let _ = app.engine.start(&wid); }
                let _ = app.engine.launch(&wid, "browser");
            }
            ID_SUSPEND => { let _ = app.engine.stop(&wid); }
            ID_DESTROY => {
                let _ = app.engine.destroy(&wid);
                app.items.remove(i);
                app.selected = if app.items.is_empty() { None } else { Some(i.min(app.items.len() - 1)) };
            }
            _ => {}
        }
    });
}

fn fill_round(dc: Handle, x: i32, y: i32, w: i32, h: i32, color: u32, radius: i32) {
    unsafe {
        let brush = CreateSolidBrush(color);
        SelectObject(dc, GetStockObject(NULL_PEN));
        let old = SelectObject(dc, brush);
        RoundRect(dc, x, y, x + w, y + h, radius, radius);
        SelectObject(dc, old);
        DeleteObject(brush);
    }
}

fn text(dc: Handle, s: &str, x: i32, y: i32, w: i32, h: i32, color: u32, fmt: u32) {
    unsafe {
        SetTextColor(dc, color);
        let mut r = Rect { left: x, top: y, right: x + w, bottom: y + h };
        let t = wide(s);
        DrawTextW(dc, t.as_ptr(), -1, &mut r, fmt | DT_SINGLELINE);
    }
}

fn paint(hwnd: Hwnd) {
    APP.with(|a| {
        let mut b = a.borrow_mut();
        let Some(app) = &mut *b else { return };
        unsafe {
            let mut ps: PaintStruct = std::mem::zeroed();
            let dc = BeginPaint(hwnd, &mut ps);
            let mut rc: Rect = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            let bg = CreateSolidBrush(c_bg());
            FillRect(dc, &rc, bg);
            DeleteObject(bg);
            SetBkMode(dc, TRANSPARENT);

            // header
            SelectObject(dc, app.font_h as Handle);
            text(dc, "WSE Desktop", M, 22, 400, 34, c_text(), DT_LEFT | DT_VCENTER);
            SelectObject(dc, app.font_s as Handle);
            text(dc, "your workspaces", M, 58, 400, 20, c_sub(), DT_LEFT | DT_VCENTER);

            // input pill (the EDIT sits inside it)
            fill_round(dc, M, 88, W - M * 2 - 132, 40, c_input(), 12);

            SelectObject(dc, app.font as Handle);
            // buttons (create + actions)
            for btn in &app.buttons {
                let hovered = app.hover_btn == Some(btn.id);
                let color = match (btn.kind, hovered) {
                    (Kind::Primary, false) => c_accent(),
                    (Kind::Primary, true) => c_accent_hi(),
                    (Kind::Secondary, false) => c_surface(),
                    (Kind::Secondary, true) => c_surface_hi(),
                    (Kind::Danger, false) => c_danger(),
                    (Kind::Danger, true) => c_danger_hi(),
                };
                fill_round(dc, btn.x, btn.y, btn.w, btn.h, color, 10);
                text(dc, btn.label, btn.x, btn.y, btn.w, btn.h, c_text(), DT_CENTER | DT_VCENTER);
            }

            // cards
            for (i, (name, wid)) in app.items.iter().enumerate() {
                let top = card_top(i);
                let color = if Some(i) == app.selected { c_card_sel() }
                    else if Some(i) == app.hover_card { c_card_hover() }
                    else { c_card() };
                fill_round(dc, M, top, W - M * 2, CARD_H, color, 12);

                let state = app.engine.state(wid);
                let (state_s, dot) = match state {
                    Some(WorkspaceState::Running) => ("Running", rgb(80, 200, 120)),
                    Some(WorkspaceState::Saved) => ("Suspended", rgb(220, 180, 90)),
                    Some(WorkspaceState::Created) => ("Ready", c_sub()),
                    _ => ("-", c_sub()),
                };
                let apps = if state == Some(WorkspaceState::Running) {
                    app.engine.app_instances(wid).map(|v| v.len()).unwrap_or(0)
                } else { 0 };

                SelectObject(dc, app.font as Handle);
                text(dc, name, M + 16, top + 10, W - M * 2 - 32, 26, c_text(), DT_LEFT | DT_VCENTER);
                fill_round(dc, M + 16, top + 42, 8, 8, dot, 4);
                SelectObject(dc, app.font_s as Handle);
                text(dc, &format!("{state_s}   ·   {apps} app(s)"), M + 32, top + 38, 300, 18, c_sub(), DT_LEFT | DT_VCENTER);
            }

            if app.items.is_empty() {
                SelectObject(dc, app.font_s as Handle);
                text(dc, "No workspaces yet — type a name and press Create.", M, LIST_TOP + 16, W - M * 2, 24, c_sub(), DT_LEFT | DT_VCENTER);
            }
            EndPaint(hwnd, &ps);
        }
    });
}

fn hit_button(app: &App, x: i32, y: i32) -> Option<u32> {
    app.buttons.iter().find(|b| x >= b.x && x < b.x + b.w && y >= b.y && y < b.y + b.h).map(|b| b.id)
}
fn hit_card(app: &App, y: i32) -> Option<usize> {
    if y < LIST_TOP { return None; }
    let idx = ((y - LIST_TOP) / (CARD_H + CARD_GAP)) as usize;
    let within = (y - LIST_TOP) % (CARD_H + CARD_GAP) < CARD_H;
    if within && idx < app.items.len() { Some(idx) } else { None }
}

extern "system" fn wnd_proc(hwnd: Hwnd, msg: u32, w: usize, l: isize) -> isize {
    match msg {
        WM_CREATE => { build(hwnd); 0 }
        WM_PAINT => { paint(hwnd); 0 }
        WM_MOUSEMOVE => {
            let (x, y) = ((l & 0xFFFF) as i16 as i32, ((l >> 16) & 0xFFFF) as i16 as i32);
            let mut changed = false;
            APP.with(|a| {
                if let Some(app) = &mut *a.borrow_mut() {
                    let hb = hit_button(app, x, y);
                    let hc = if hb.is_none() { hit_card(app, y) } else { None };
                    if hb != app.hover_btn || hc != app.hover_card {
                        app.hover_btn = hb; app.hover_card = hc; changed = true;
                    }
                }
            });
            if changed { unsafe { InvalidateRect(hwnd, std::ptr::null(), 0); } }
            0
        }
        WM_LBUTTONDOWN => {
            let (x, y) = ((l & 0xFFFF) as i16 as i32, ((l >> 16) & 0xFFFF) as i16 as i32);
            let mut clicked: Option<u32> = None;
            APP.with(|a| {
                if let Some(app) = &mut *a.borrow_mut() {
                    if let Some(id) = hit_button(app, x, y) { clicked = Some(id); }
                    else if let Some(i) = hit_card(app, y) { app.selected = Some(i); }
                }
            });
            if let Some(id) = clicked { act(id); }
            unsafe { InvalidateRect(hwnd, std::ptr::null(), 0); }
            0
        }
        WM_CTLCOLOREDIT => {
            APP.with(|a| {
                if let Some(app) = &*a.borrow() {
                    unsafe {
                        SetBkColor(w as Handle, c_input());
                        SetTextColor(w as Handle, c_text());
                    }
                    return app.input_brush;
                }
                0
            })
        }
        WM_TIMER => { unsafe { InvalidateRect(hwnd, std::ptr::null(), 0); } 0 }
        WM_DESTROY => {
            APP.with(|a| {
                if let Some(app) = &mut *a.borrow_mut() {
                    let ids: Vec<_> = app.items.iter().map(|(_, w)| w.clone()).collect();
                    for id in ids { let _ = app.engine.destroy(&id); }
                }
            });
            unsafe { PostQuitMessage(0); }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, w, l) },
    }
}

fn build(hwnd: Hwnd) {
    APP.with(|a| {
        let mut b = a.borrow_mut();
        let Some(app) = &mut *b else { return };
        unsafe {
            let inst = GetModuleHandleW(std::ptr::null());
            let mkfont = |sz: i32, weight: i32| CreateFontW(sz, 0, 0, 0, weight, 0, 0, 0, 1, 0, 0, 5, 0, wide("Segoe UI").as_ptr());
            app.font = mkfont(-19, 400) as isize;
            app.font_h = mkfont(-30, 600) as isize;
            app.font_s = mkfont(-16, 400) as isize;
            app.input_brush = CreateSolidBrush(c_input()) as isize;

            // borderless dark edit inside the input pill
            let edit = CreateWindowExW(0, wide("EDIT").as_ptr(), std::ptr::null(),
                WS_CHILD | WS_VISIBLE | ES_AUTOHSCROLL, M + 12, 99, W - M * 2 - 132 - 24, 22,
                hwnd, 100usize as Handle, inst, std::ptr::null());
            app.edit = edit as isize;
            SendMessageW(edit, WM_SETFONT, app.font as usize, 1);
            SetFocus(edit);

            // buttons
            let cx = W - M - 120;
            app.buttons.push(Btn { id: ID_CREATE, label: "Create", x: cx, y: 88, w: 120, h: 40, kind: Kind::Primary });
            let by = H - 60;
            let bw = (W - M * 2 - 3 * 12) / 4;
            let mut x = M;
            for (id, label, kind) in [
                (ID_ENTER, "Enter", Kind::Primary),
                (ID_LAUNCH, "Launch", Kind::Secondary),
                (ID_SUSPEND, "Suspend", Kind::Secondary),
                (ID_DESTROY, "Destroy", Kind::Danger),
            ] {
                app.buttons.push(Btn { id, label, x, y: by, w: bw, h: 40, kind });
                x += bw + 12;
            }
        }
    });
}

fn main() {
    APP.with(|a| {
        *a.borrow_mut() = Some(App {
            engine: Engine::new(WindowsNativeAdapter::new()),
            items: Vec::new(), selected: None, docked: HashSet::new(),
            edit: 0, font: 0, font_h: 0, font_s: 0, input_brush: 0,
            buttons: Vec::new(), hover_btn: None, hover_card: None, next: 0,
        });
    });
    unsafe {
        let inst = GetModuleHandleW(std::ptr::null());
        let class = wide("WseAppWindow");
        let bg = CreateSolidBrush(c_bg());
        let wc = WndClassW {
            style: 0, wnd_proc, cls_extra: 0, wnd_extra: 0, inst,
            icon: std::ptr::null_mut(), cursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            bg, menu: std::ptr::null(), class: class.as_ptr(),
        };
        RegisterClassW(&wc);
        let sw = GetSystemMetrics(SM_CXSCREEN);
        let sh = GetSystemMetrics(SM_CYSCREEN);
        let hwnd = CreateWindowExW(0, class.as_ptr(), wide("WSE").as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE, (sw - W) / 2, (sh - H) / 2, W, H,
            std::ptr::null_mut(), std::ptr::null_mut(), inst, std::ptr::null());
        // dark title bar (Win10 2004+/Win11)
        let dark: i32 = 1;
        DwmSetWindowAttribute(hwnd, 20, &dark as *const i32 as *const c_void, 4);
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
        SetTimer(hwnd, 1, 1000, std::ptr::null());
        let mut msg: Msg = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
