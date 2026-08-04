//! WSE Desktop — the visual launcher. A native Win32 window (dark, owner-drawn
//! workspace cards) over the same engine + native adapter as the CLI. Create a
//! workspace, enter it (switch to its desktop; Ctrl+Alt+Q to return), launch a
//! browser, suspend, or destroy — all with clicks.
#![windows_subsystem = "windows"]

use std::cell::RefCell;
use std::collections::HashSet;
use std::ffi::c_void;

use wse_adapter_windows_native::{
    enter_workspace_desktop, spawn_workspace_dock, WindowsNativeAdapter,
};
use wse_common::{ApplicationDescriptor, Persistence, WorkspaceId, WorkspaceState};
use wse_engine::{Engine, WorkspaceConfig};

type Hwnd = *mut c_void;
type Handle = *mut c_void;
type WndProc = extern "system" fn(Hwnd, u32, usize, isize) -> isize;

const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_CHILD: u32 = 0x4000_0000;
const WS_BORDER: u32 = 0x0080_0000;
const ES_AUTOHSCROLL: u32 = 0x0080;
const BS_PUSHBUTTON: u32 = 0;
const WM_CREATE: u32 = 0x0001;
const WM_PAINT: u32 = 0x000F;
const WM_COMMAND: u32 = 0x0111;
const WM_TIMER: u32 = 0x0113;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_DESTROY: u32 = 0x0002;
const WM_SETFONT: u32 = 0x0030;
const WM_GETTEXT: u32 = 0x000D;
const WM_SETTEXT: u32 = 0x000C;
const IDC_ARROW: u32 = 32512;
const SW_SHOW: i32 = 5;
const SM_CXSCREEN: i32 = 0;
const SM_CYSCREEN: i32 = 1;
const TRANSPARENT: i32 = 1;
const NULL_PEN: i32 = 8;
const DT_LEFT: u32 = 0;
const DT_VCENTER: u32 = 4;
const DT_SINGLELINE: u32 = 0x20;
const DT_RIGHT: u32 = 2;

// control ids
const ID_EDIT: u32 = 100;
const ID_CREATE: u32 = 101;
const ID_ENTER: u32 = 201;
const ID_LAUNCH: u32 = 202;
const ID_SUSPEND: u32 = 203;
const ID_DESTROY: u32 = 204;
const TIMER: usize = 1;

// layout
const W: i32 = 560;
const H: i32 = 680;
const LIST_TOP: i32 = 104;
const CARD_H: i32 = 66;
const CARD_GAP: i32 = 10;

#[repr(C)]
struct Rect { left: i32, top: i32, right: i32, bottom: i32 }
#[repr(C)]
struct PaintStruct { hdc: Handle, erase: i32, rc: Rect, restore: i32, inc: i32, rgb: [u8; 32] }
#[repr(C)]
struct Msg { hwnd: Hwnd, message: u32, w: usize, l: isize, time: u32, x: i32, y: i32 }
#[repr(C)]
struct WndClassW {
    style: u32, wnd_proc: WndProc, cls_extra: i32, wnd_extra: i32,
    inst: Handle, icon: Handle, cursor: Handle, bg: Handle,
    menu: *const u16, class: *const u16,
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
}
#[link(name = "gdi32")]
extern "system" {
    fn CreateSolidBrush(c: u32) -> Handle;
    fn RoundRect(dc: Handle, l: i32, t: i32, r: i32, b: i32, w: i32, h: i32) -> i32;
    fn SelectObject(dc: Handle, o: Handle) -> Handle;
    fn DeleteObject(o: Handle) -> i32;
    fn GetStockObject(i: i32) -> Handle;
    fn SetBkMode(dc: Handle, m: i32) -> i32;
    fn SetTextColor(dc: Handle, c: u32) -> u32;
    fn CreateFontW(h: i32, w: i32, e: i32, o: i32, wt: i32, it: u32, un: u32, st: u32, cs: u32, op: u32, cp: u32, q: u32, p: u32, face: *const u16) -> Handle;
}
#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(n: *const u16) -> Handle;
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

struct App {
    engine: Engine<WindowsNativeAdapter>,
    items: Vec<(String, WorkspaceId)>,
    selected: Option<usize>,
    docked: HashSet<WorkspaceId>,
    edit: isize,
    font: isize,
    next: u32,
}

thread_local! { static APP: RefCell<Option<App>> = const { RefCell::new(None) }; }

fn card_rect(i: usize) -> (i32, i32) {
    let y = LIST_TOP + i as i32 * (CARD_H + CARD_GAP);
    (y, y + CARD_H)
}

fn create_workspace() {
    APP.with(|a| {
        let mut b = a.borrow_mut();
        let Some(app) = &mut *b else { return };
        // read the name box; blank -> auto name
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
        let Some(i) = app.selected else { return };
        let Some((_, wid)) = app.items.get(i).cloned() else { return };
        match id {
            ID_ENTER => {
                if app.docked.insert(wid.clone()) {
                    spawn_workspace_dock(&wid);
                }
                enter_workspace_desktop(&wid);
            }
            ID_LAUNCH => {
                if app.engine.state(&wid) != Some(WorkspaceState::Running) {
                    let _ = app.engine.start(&wid);
                }
                let _ = app.engine.launch(&wid, "browser");
            }
            ID_SUSPEND => {
                let _ = app.engine.stop(&wid);
            }
            ID_DESTROY => {
                let _ = app.engine.destroy(&wid);
                app.items.remove(i);
                app.selected = if app.items.is_empty() { None } else { Some(i.min(app.items.len() - 1)) };
            }
            _ => {}
        }
    });
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
            let bg = CreateSolidBrush(rgb(24, 24, 30));
            FillRect(dc, &rc, bg);
            DeleteObject(bg);
            SetBkMode(dc, TRANSPARENT);
            SelectObject(dc, app.font as Handle);
            SelectObject(dc, GetStockObject(NULL_PEN));

            // header
            SetTextColor(dc, rgb(235, 235, 240));
            let mut hr = Rect { left: 16, top: 12, right: W - 16, bottom: 40 };
            let t = wide("WSE Workspaces");
            DrawTextW(dc, t.as_ptr(), -1, &mut hr, DT_LEFT | DT_VCENTER | DT_SINGLELINE);

            // cards
            let card = CreateSolidBrush(rgb(44, 44, 54));
            let sel = CreateSolidBrush(rgb(52, 78, 128));
            for (i, (name, wid)) in app.items.iter().enumerate() {
                let (top, bot) = card_rect(i);
                let brush = if Some(i) == app.selected { sel } else { card };
                SelectObject(dc, brush);
                RoundRect(dc, 16, top, W - 16, bot, 14, 14);

                let state = app.engine.state(wid);
                let state_s = match state {
                    Some(WorkspaceState::Running) => "Running",
                    Some(WorkspaceState::Saved) => "Suspended",
                    Some(WorkspaceState::Created) => "Ready",
                    _ => "-",
                };
                let apps = if state == Some(WorkspaceState::Running) {
                    app.engine.app_instances(wid).map(|v| v.len()).unwrap_or(0)
                } else {
                    0
                };

                SetTextColor(dc, rgb(240, 240, 245));
                let mut nr = Rect { left: 32, top: top + 8, right: W - 32, bottom: top + 34 };
                let nt = wide(name);
                DrawTextW(dc, nt.as_ptr(), -1, &mut nr, DT_LEFT | DT_SINGLELINE);

                SetTextColor(dc, rgb(160, 165, 180));
                let mut sr = Rect { left: 32, top: top + 34, right: W - 32, bottom: bot - 6 };
                let st = wide(&format!("{state_s}   -   {apps} app(s)"));
                DrawTextW(dc, st.as_ptr(), -1, &mut sr, DT_LEFT | DT_SINGLELINE);
            }
            DeleteObject(card);
            DeleteObject(sel);

            if app.items.is_empty() {
                SetTextColor(dc, rgb(140, 140, 155));
                let mut er = Rect { left: 16, top: LIST_TOP + 20, right: W - 16, bottom: LIST_TOP + 60 };
                let et = wide("No workspaces yet - type a name and click Create.");
                DrawTextW(dc, et.as_ptr(), -1, &mut er, DT_LEFT | DT_SINGLELINE);
            }
            let _ = DT_RIGHT;
            EndPaint(hwnd, &ps);
        }
    });
}

fn on_click(y: i32) {
    APP.with(|a| {
        let mut b = a.borrow_mut();
        let Some(app) = &mut *b else { return };
        if y < LIST_TOP {
            return;
        }
        let idx = ((y - LIST_TOP) / (CARD_H + CARD_GAP)) as usize;
        let within = (y - LIST_TOP) % (CARD_H + CARD_GAP) < CARD_H;
        if within && idx < app.items.len() {
            app.selected = Some(idx);
        }
    });
}

extern "system" fn wnd_proc(hwnd: Hwnd, msg: u32, w: usize, l: isize) -> isize {
    match msg {
        WM_CREATE => {
            build_controls(hwnd);
            0
        }
        WM_PAINT => { paint(hwnd); 0 }
        WM_LBUTTONDOWN => {
            on_click(((l >> 16) & 0xFFFF) as i16 as i32);
            unsafe { InvalidateRect(hwnd, std::ptr::null(), 1); }
            0
        }
        WM_COMMAND => {
            let id = (w & 0xFFFF) as u32;
            match id {
                ID_CREATE => create_workspace(),
                ID_ENTER | ID_LAUNCH | ID_SUSPEND | ID_DESTROY => act(id),
                _ => {}
            }
            unsafe { InvalidateRect(hwnd, std::ptr::null(), 1); }
            0
        }
        WM_TIMER => { unsafe { InvalidateRect(hwnd, std::ptr::null(), 1); } 0 }
        WM_DESTROY => {
            // tear down all workspaces on exit (no daemon yet)
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

fn build_controls(hwnd: Hwnd) {
    APP.with(|a| {
        let mut b = a.borrow_mut();
        let Some(app) = &mut *b else { return };
        unsafe {
            let inst = GetModuleHandleW(std::ptr::null());
            let font = CreateFontW(-19, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 5, 0, wide("Segoe UI").as_ptr());
            app.font = font as isize;

            let edit = CreateWindowExW(0, wide("EDIT").as_ptr(), std::ptr::null(),
                WS_CHILD | WS_VISIBLE | WS_BORDER | ES_AUTOHSCROLL, 16, 52, 300, 30,
                hwnd, ID_EDIT as usize as Handle, inst, std::ptr::null());
            app.edit = edit as isize;
            SendMessageW(edit, WM_SETFONT, font as usize, 1);

            let mk = |text: &str, id: u32, x: i32, y: i32, w: i32| {
                let h = CreateWindowExW(0, wide("BUTTON").as_ptr(), wide(text).as_ptr(),
                    WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON, x, y, w, 30,
                    hwnd, id as usize as Handle, inst, std::ptr::null());
                SendMessageW(h, WM_SETFONT, font as usize, 1);
            };
            mk("Create", ID_CREATE, 328, 52, 100);
            // action row along the bottom
            let by = H - 66;
            mk("Enter", ID_ENTER, 16, by, 120);
            mk("Launch", ID_LAUNCH, 148, by, 120);
            mk("Suspend", ID_SUSPEND, 280, by, 120);
            mk("Destroy", ID_DESTROY, 412, by, 120);
        }
    });
}

fn main() {
    APP.with(|a| {
        *a.borrow_mut() = Some(App {
            engine: Engine::new(WindowsNativeAdapter::new()),
            items: Vec::new(),
            selected: None,
            docked: HashSet::new(),
            edit: 0,
            font: 0,
            next: 0,
        });
    });

    unsafe {
        let inst = GetModuleHandleW(std::ptr::null());
        let class = wide("WseAppWindow");
        let wc = WndClassW {
            style: 0,
            wnd_proc,
            cls_extra: 0,
            wnd_extra: 0,
            inst,
            icon: std::ptr::null_mut(),
            cursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            bg: std::ptr::null_mut(),
            menu: std::ptr::null(),
            class: class.as_ptr(),
        };
        RegisterClassW(&wc);
        let sw = GetSystemMetrics(SM_CXSCREEN);
        let sh = GetSystemMetrics(SM_CYSCREEN);
        let hwnd = CreateWindowExW(0, class.as_ptr(), wide("WSE").as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            (sw - W) / 2, (sh - H) / 2, W, H,
            std::ptr::null_mut(), std::ptr::null_mut(), inst, std::ptr::null());
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
        SetTimer(hwnd, TIMER, 1000, std::ptr::null());

        let mut msg: Msg = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
