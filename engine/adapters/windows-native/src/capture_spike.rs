//! SPIKE (throwaway): can native GDI capture a modern, GPU-composited window?
//!
//! The native `SurfaceProvider` needs to grab pixels of a workspace's apps
//! (Chrome/VS Code — all Chromium/DWM-composited). The old `BitBlt` trick returns
//! black for those. `PrintWindow` with `PW_RENDERFULLCONTENT` is the modern
//! per-window capture that *sometimes* works for GPU apps. This probes the current
//! foreground window and reports the non-black pixel ratio, so we get a real data
//! point before committing to a native capture path. See docs/watch-and-control.md.

use std::ffi::c_void;

type Handle = *mut c_void;

const PW_RENDERFULLCONTENT: u32 = 0x0000_0002;

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

#[link(name = "user32")]
extern "system" {
    fn GetForegroundWindow() -> Handle;
    fn GetWindowRect(hwnd: Handle, r: *mut Rect) -> i32;
    fn GetWindowTextW(hwnd: Handle, s: *mut u16, n: i32) -> i32;
    fn GetWindowDC(hwnd: Handle) -> Handle;
    fn ReleaseDC(hwnd: Handle, hdc: Handle) -> i32;
    fn PrintWindow(hwnd: Handle, hdc: Handle, flags: u32) -> i32;
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

/// Capture the current foreground window via PrintWindow and report what we got.
pub fn probe_foreground_capture() -> String {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return "no foreground window".into();
        }
        let mut buf = [0u16; 256];
        let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), 256);
        let title = String::from_utf16_lossy(&buf[..n.max(0) as usize]);

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
        bi.header.height = -h; // top-down
        bi.header.planes = 1;
        bi.header.bit_count = 32;
        bi.header.compression = 0; // BI_RGB
        let mut pixels = vec![0u8; (w as usize) * (h as usize) * 4];
        let lines = GetDIBits(mem, bmp, 0, h as u32, pixels.as_mut_ptr() as *mut c_void, &mut bi, 0);

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

        format!(
            "window='{title}'  {w}x{h}  PrintWindow_ok={printed}  GetDIBits_lines={lines}  non-black={ratio:.1}%  \
             => native per-window capture {}",
            if ratio > 5.0 { "LOOKS FEASIBLE (real pixels)" } else { "FAILED (black — GPU app not captured)" }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "captures the current foreground window (spike; run with --nocapture)"]
    fn spike_capture_foreground() {
        println!("SPIKE: {}", probe_foreground_capture());
    }
}
