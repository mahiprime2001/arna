//! Monitor enumeration + virtual-desktop pointer positioning.
//!
//! The agent streams one chosen monitor and must land clicks on *that* monitor.
//! enigo's absolute mouse path normalizes against the **primary** monitor only
//! (`SM_CXSCREEN`, no `VIRTUALDESK` flag), so it can't reach a second screen. On
//! Windows we therefore enumerate monitors ourselves (origin, size, primary) and
//! move the pointer with `SendInput` over the whole virtual desktop. Other
//! platforms get a no-op fallback — primary-only until they grow their own impl.

/// One physical monitor in desktop coordinates.
#[derive(Clone, Debug)]
pub struct Monitor {
    /// Top-left in the virtual desktop (can be negative for monitors left/above
    /// the primary).
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub primary: bool,
}

/// True if [`move_to_global`] can actually reach all monitors on this platform.
pub const CAN_TARGET_ANY_MONITOR: bool = cfg!(windows);

#[cfg(windows)]
mod imp {
    use super::Monitor;
    use std::mem::{size_of, zeroed};
    use winapi::shared::minwindef::{BOOL, DWORD, LPARAM, TRUE};
    use winapi::shared::windef::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, HDC, HMONITOR, LPRECT};
    use winapi::um::winuser::{
        EnumDisplayMonitors, GetMonitorInfoW, GetSystemMetrics, SendInput,
        SetProcessDpiAwarenessContext, INPUT, INPUT_MOUSE, MONITORINFOEXW, MONITORINFOF_PRIMARY,
        MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_MOVE, MOUSEEVENTF_VIRTUALDESK, SM_CXVIRTUALSCREEN,
        SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
    };

    /// Become per-monitor DPI aware so scrap's physical capture sizes and our
    /// monitor rects agree in the same pixel space (otherwise a scaled 4K screen
    /// reports different sizes to each API and resolution-matching breaks).
    pub fn make_dpi_aware() {
        unsafe {
            SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
    }

    unsafe extern "system" fn collect(h: HMONITOR, _: HDC, _: LPRECT, data: LPARAM) -> BOOL {
        let out = &mut *(data as *mut Vec<Monitor>);
        let mut mi: MONITORINFOEXW = zeroed();
        mi.cbSize = size_of::<MONITORINFOEXW>() as DWORD;
        if GetMonitorInfoW(h, &mut mi as *mut _ as *mut _) != 0 {
            let r = mi.rcMonitor;
            out.push(Monitor {
                x: r.left,
                y: r.top,
                width: r.right - r.left,
                height: r.bottom - r.top,
                primary: mi.dwFlags & MONITORINFOF_PRIMARY != 0,
            });
        }
        TRUE
    }

    pub fn monitors() -> Vec<Monitor> {
        let mut out: Vec<Monitor> = Vec::new();
        unsafe {
            EnumDisplayMonitors(
                std::ptr::null_mut(),
                std::ptr::null(),
                Some(collect),
                &mut out as *mut _ as LPARAM,
            );
        }
        out
    }

    /// Move the pointer to a global virtual-desktop pixel coordinate.
    pub fn move_to_global(gx: i32, gy: i32) {
        unsafe {
            let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let vw = (GetSystemMetrics(SM_CXVIRTUALSCREEN) - 1).max(1) as i64;
            let vh = (GetSystemMetrics(SM_CYVIRTUALSCREEN) - 1).max(1) as i64;
            // SendInput absolute coords are 0..65535 across the whole virtual desktop.
            let nx = ((gx - vx) as i64 * 65535 / vw) as i32;
            let ny = ((gy - vy) as i64 * 65535 / vh) as i32;
            let mut input: INPUT = zeroed();
            input.type_ = INPUT_MOUSE;
            {
                let mi = input.u.mi_mut();
                mi.dx = nx;
                mi.dy = ny;
                mi.dwFlags = MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK;
            }
            SendInput(1, &mut input, size_of::<INPUT>() as i32);
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Monitor;
    pub fn make_dpi_aware() {}
    pub fn monitors() -> Vec<Monitor> {
        Vec::new()
    }
    pub fn move_to_global(_gx: i32, _gy: i32) {}
}

pub use imp::{make_dpi_aware, monitors, move_to_global};
