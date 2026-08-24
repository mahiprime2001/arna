//! Native workspace surface capture. Grabs a workspace's front app window from
//! its own (possibly background) desktop via PrintWindow — proven feasible by the
//! capture spike — so a NATIVE workspace can be streamed to a guest without the
//! host viewing it. Only the workspace's window is captured, never the host
//! desktop or the host's other apps. (v1: the front window; compositing the full
//! window set is a later refinement.)

use std::ffi::c_void;

use super::{desktop_name, wide};
use wse_common::WorkspaceId;

type Handle = *mut c_void;

const PW_RENDERFULLCONTENT: u32 = 0x0000_0002;
const GENERIC_ALL: u32 = 0x1000_0000;

#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct BitmapInfoHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bit_count: u16,
    compression: u32,
    size_image: u32,
    x_ppm: i32,
    y_ppm: i32,
    clr_used: u32,
    clr_important: u32,
}

#[repr(C)]
struct BitmapInfo {
    header: BitmapInfoHeader,
    colors: [u32; 1],
}

type EnumProc = extern "system" fn(Handle, isize) -> i32;

#[link(name = "user32")]
extern "system" {
    fn OpenDesktopW(name: *const u16, flags: u32, inherit: i32, access: u32) -> Handle;
    fn CloseDesktop(h: Handle) -> i32;
    fn EnumDesktopWindows(hdesk: Handle, cb: EnumProc, lparam: isize) -> i32;
    fn IsWindowVisible(h: Handle) -> i32;
    fn GetWindowTextLengthW(h: Handle) -> i32;
    fn GetWindowRect(h: Handle, r: *mut Rect) -> i32;
    fn GetWindowDC(h: Handle) -> Handle;
    fn ReleaseDC(h: Handle, hdc: Handle) -> i32;
    fn PrintWindow(h: Handle, hdc: Handle, flags: u32) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateCompatibleDC(hdc: Handle) -> Handle;
    fn CreateCompatibleBitmap(hdc: Handle, w: i32, h: i32) -> Handle;
    fn SelectObject(hdc: Handle, obj: Handle) -> Handle;
    fn DeleteDC(hdc: Handle) -> i32;
    fn DeleteObject(obj: Handle) -> i32;
    fn GetDIBits(
        hdc: Handle,
        bmp: Handle,
        start: u32,
        lines: u32,
        bits: *mut c_void,
        info: *mut BitmapInfo,
        usage: u32,
    ) -> i32;
}

extern "system" fn first_window_cb(h: Handle, l: isize) -> i32 {
    unsafe {
        let out = &mut *(l as *mut Vec<Handle>);
        if IsWindowVisible(h) != 0 && GetWindowTextLengthW(h) > 0 {
            out.push(h);
        }
    }
    1
}

/// Capture the workspace's front app window as RGB pixels: `(width, height, rgb)`.
/// `None` if the workspace desktop or a window isn't there yet.
pub fn capture_workspace_frame(id: &WorkspaceId) -> Option<(u32, u32, Vec<u8>)> {
    unsafe {
        let hdesk = OpenDesktopW(wide(&desktop_name(id)).as_ptr(), 0, 0, GENERIC_ALL);
        if hdesk.is_null() {
            return None;
        }
        let mut ws: Vec<Handle> = Vec::new();
        EnumDesktopWindows(hdesk, first_window_cb, &mut ws as *mut _ as isize);
        CloseDesktop(hdesk);
        let hwnd = *ws.first()?;

        let mut r: Rect = std::mem::zeroed();
        GetWindowRect(hwnd, &mut r);
        let (w, h) = ((r.right - r.left).max(1), (r.bottom - r.top).max(1));

        let hdc_win = GetWindowDC(hwnd);
        let mem = CreateCompatibleDC(hdc_win);
        let bmp = CreateCompatibleBitmap(hdc_win, w, h);
        let old = SelectObject(mem, bmp);
        PrintWindow(hwnd, mem, PW_RENDERFULLCONTENT);

        let mut bi: BitmapInfo = std::mem::zeroed();
        bi.header.size = std::mem::size_of::<BitmapInfoHeader>() as u32;
        bi.header.width = w;
        bi.header.height = -h; // top-down
        bi.header.planes = 1;
        bi.header.bit_count = 32;
        let mut px = vec![0u8; (w as usize) * (h as usize) * 4];
        GetDIBits(mem, bmp, 0, h as u32, px.as_mut_ptr() as *mut c_void, &mut bi, 0);

        SelectObject(mem, old);
        DeleteObject(bmp);
        DeleteDC(mem);
        ReleaseDC(hwnd, hdc_win);

        // BGRA (Windows DIB) -> RGB for JPEG.
        let mut rgb = Vec::with_capacity((w as usize) * (h as usize) * 3);
        for p in px.chunks_exact(4) {
            rgb.push(p[2]);
            rgb.push(p[1]);
            rgb.push(p[0]);
        }
        Some((w as u32, h as u32, rgb))
    }
}
