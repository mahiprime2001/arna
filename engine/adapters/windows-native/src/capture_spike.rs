//! SPIKE (throwaway): can native GDI capture a window (a) foreground, and — the
//! one that matters for a native SurfaceProvider — (b) on a SEPARATE background
//! desktop (a WSE workspace), while the host stays on their own desktop?
//!
//! `PrintWindow(PW_RENDERFULLCONTENT)` is the modern per-window capture. The
//! foreground probe proved it grabs GPU apps. This adds a background-desktop
//! probe: create a desktop, launch Notepad on it (never shown), find its window,
//! and try to capture it. Black => background capture is the wall. See
//! docs/watch-and-control.md / native-surface-spike.md.

use std::ffi::c_void;
use std::time::{Duration, Instant};

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
    fn GetForegroundWindow() -> Handle;
    fn GetWindowRect(hwnd: Handle, r: *mut Rect) -> i32;
    fn GetWindowTextW(hwnd: Handle, s: *mut u16, n: i32) -> i32;
    fn GetWindowTextLengthW(hwnd: Handle) -> i32;
    fn GetWindowDC(hwnd: Handle) -> Handle;
    fn ReleaseDC(hwnd: Handle, hdc: Handle) -> i32;
    fn PrintWindow(hwnd: Handle, hdc: Handle, flags: u32) -> i32;
    fn CreateDesktopW(
        name: *const u16,
        device: *const u16,
        devmode: *const c_void,
        flags: u32,
        access: u32,
        sa: *const c_void,
    ) -> Handle;
    fn CloseDesktop(h: Handle) -> i32;
    fn EnumDesktopWindows(hdesk: Handle, cb: EnumProc, lparam: isize) -> i32;
    fn IsWindowVisible(hwnd: Handle) -> i32;
    fn GetWindowThreadProcessId(hwnd: Handle, pid: *mut u32) -> u32;
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

/// Capture a window with PrintWindow and return (print_ok, non-black %, w, h).
fn capture_window_nonblack(hwnd: Handle) -> (i32, f64, i32, i32) {
    unsafe {
        let mut r: Rect = std::mem::zeroed();
        GetWindowRect(hwnd, &mut r);
        let (w, h) = ((r.right - r.left).max(1), (r.bottom - r.top).max(1));

        let hdc_win = GetWindowDC(hwnd);
        let mem = CreateCompatibleDC(hdc_win);
        let bmp = CreateCompatibleBitmap(hdc_win, w, h);
        let old = SelectObject(mem, bmp);

        let printed = PrintWindow(hwnd, mem, PW_RENDERFULLCONTENT);

        let mut bi: BitmapInfo = std::mem::zeroed();
        bi.header.size = std::mem::size_of::<BitmapInfoHeader>() as u32;
        bi.header.width = w;
        bi.header.height = -h;
        bi.header.planes = 1;
        bi.header.bit_count = 32;
        let mut pixels = vec![0u8; (w as usize) * (h as usize) * 4];
        GetDIBits(mem, bmp, 0, h as u32, pixels.as_mut_ptr() as *mut c_void, &mut bi, 0);

        let total = (w as usize) * (h as usize);
        let nonblack = pixels
            .chunks_exact(4)
            .filter(|px| px[0] > 8 || px[1] > 8 || px[2] > 8)
            .count();
        let ratio = if total > 0 { nonblack as f64 / total as f64 * 100.0 } else { 0.0 };

        SelectObject(mem, old);
        DeleteObject(bmp);
        DeleteDC(mem);
        ReleaseDC(hwnd, hdc_win);
        (printed, ratio, w, h)
    }
}

/// (a) capture the current foreground window.
pub fn probe_foreground_capture() -> String {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return "no foreground window".into();
        }
        let mut buf = [0u16; 256];
        let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), 256);
        let title = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
        let (printed, ratio, w, h) = capture_window_nonblack(hwnd);
        format!(
            "foreground '{title}' {w}x{h} PrintWindow_ok={printed} non-black={ratio:.1}% => {}",
            if ratio > 5.0 { "FEASIBLE" } else { "FAILED (black)" }
        )
    }
}

extern "system" fn collect_cb(h: Handle, l: isize) -> i32 {
    unsafe {
        let out = &mut *(l as *mut Vec<Handle>);
        if IsWindowVisible(h) == 0 || GetWindowTextLengthW(h) <= 0 {
            return 1;
        }
        out.push(h);
    }
    1
}

/// (b) THE one that matters: capture a window on a SEPARATE background desktop —
/// what a native workspace surface needs. Uses the browser (a real workspace app,
/// GPU-composited) since that's the actual case; a fresh desktop has only it.
pub fn probe_background_desktop_capture() -> String {
    let name = "wse-spike-capture";
    let Some(browser) = super::find_browser() else {
        return "no browser installed to test with".into();
    };
    let profile = std::env::temp_dir().join("wse-spike-cap-profile");
    let _ = std::fs::create_dir_all(&profile);
    let cmd = format!(
        "\"{}\" --user-data-dir=\"{}\" --no-first-run --no-default-browser-check --new-window about:blank",
        browser.display(),
        profile.display()
    );

    unsafe {
        let hdesk = CreateDesktopW(
            super::wide(name).as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            GENERIC_ALL,
            std::ptr::null(),
        );
        if hdesk.is_null() {
            return "CreateDesktop failed".into();
        }

        // Launch the browser on the background desktop (never shown to the user).
        let pid = match super::create_process_on_desktop(name, &cmd, "C:\\") {
            Ok(p) => p,
            Err(_) => {
                CloseDesktop(hdesk);
                return "could not launch on the background desktop".into();
            }
        };

        // The desktop is fresh — the first visible titled window is the browser.
        let deadline = Instant::now() + Duration::from_secs(12);
        let mut hwnd: Handle = std::ptr::null_mut();
        while Instant::now() < deadline {
            let mut found: Vec<Handle> = Vec::new();
            EnumDesktopWindows(hdesk, collect_cb, &mut found as *mut _ as isize);
            if let Some(h) = found.first() {
                hwnd = *h;
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
        }

        let result = if hwnd.is_null() {
            "no window found on the background desktop (didn't open / not enumerable)".to_string()
        } else {
            let (printed, ratio, w, h) = capture_window_nonblack(hwnd);
            format!(
                "background-desktop window {w}x{h} PrintWindow_ok={printed} non-black={ratio:.1}% => {}",
                if ratio > 5.0 {
                    "CAPTURE WORKS on a background desktop (native surface is feasible)"
                } else {
                    "BLACK — background-desktop capture failed (the wall)"
                }
            )
        };

        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .output();
        CloseDesktop(hdesk);
        let _ = std::fs::remove_dir_all(&profile);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "captures the foreground window (spike; --nocapture)"]
    fn spike_capture_foreground() {
        println!("SPIKE: {}", probe_foreground_capture());
    }

    #[test]
    #[ignore = "creates a background desktop + Notepad and captures it (spike; --nocapture)"]
    fn spike_capture_background_desktop() {
        println!("SPIKE: {}", probe_background_desktop_capture());
    }
}
