//! Windows "bubble" proof-of-concept (no VM / no Sandbox needed).
//!
//! Creates a separate **desktop object**, launches an app on it (the owner never
//! sees it on their real desktop), captures the app's window with `PrintWindow`,
//! and posts keystrokes into it with `PostMessage`. This is the make-or-break for
//! a VM-less app bubble on Windows Home: can we run + capture + drive an app in
//! isolation from the owner's desktop?
//!
//! Run:  cargo run -p arna-agent --example bubble_poc -- [exe] [out_dir]
//!   e.g. cargo run -p arna-agent --example bubble_poc -- notepad.exe D:/tmp
//!
//! Writes bubble_before.png / bubble_after.png and prints a non-black pixel ratio
//! (proves real content was captured) plus whether typed text landed.

#[cfg(not(windows))]
fn main() {
    eprintln!("bubble_poc is Windows-only.");
}

#[cfg(windows)]
fn main() {
    unsafe { win::run() }
}

#[cfg(windows)]
mod win {
    use std::ffi::OsStr;
    use std::iter::once;
    use std::mem::{size_of, zeroed};
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::{null, null_mut};

    use winapi::shared::minwindef::{BOOL, DWORD, LPARAM, TRUE, UINT};
    use winapi::shared::windef::{HBITMAP, HDC, HWND, RECT};
    use winapi::um::processthreadsapi::{CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW};
    use winapi::um::wingdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, SelectObject,
        BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use winapi::um::winuser::{
        CloseDesktop, CreateDesktopW, EnumChildWindows, EnumDesktopWindows, GetClassNameW, GetDC,
        GetWindowRect, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, PrintWindow,
        ReleaseDC, PW_RENDERFULLCONTENT, WM_CHAR,
    };

    const GENERIC_ALL: DWORD = 0x1000_0000;
    const STARTF_USESHOWWINDOW: DWORD = 0x0000_0001;
    const SW_SHOW: u16 = 5;

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(once(0)).collect()
    }

    struct Find {
        pid: DWORD,
        // Largest visible window matching the launched pid …
        best_pid: HWND,
        best_pid_area: i64,
        // … and, as a fallback, the largest visible window on the (fresh) desktop
        // regardless of pid (handles apps whose window lives in a child process).
        best_any: HWND,
        best_any_area: i64,
    }

    unsafe extern "system" fn enum_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let f = &mut *(lparam as *mut Find);
        if IsWindowVisible(hwnd) != TRUE {
            return TRUE;
        }
        let mut r: RECT = zeroed();
        if GetWindowRect(hwnd, &mut r) == 0 {
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

    unsafe extern "system" fn child_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let out = &mut *(lparam as *mut HWND);
        let mut cls = [0u16; 64];
        let n = GetClassNameW(hwnd, cls.as_mut_ptr(), cls.len() as i32);
        let name = String::from_utf16_lossy(&cls[..n as usize]).to_lowercase();
        if name.contains("edit") {
            *out = hwnd;
            return 0; // stop
        }
        TRUE
    }

    /// Find a child Edit/RichEdit control to type into, or null.
    unsafe fn find_edit(parent: HWND) -> HWND {
        let mut found: HWND = null_mut();
        EnumChildWindows(parent, Some(child_cb), &mut found as *mut _ as LPARAM);
        found
    }

    /// Capture a window via PrintWindow into a tight BGRA buffer. Returns
    /// (width, height, pixels) or None.
    unsafe fn capture(hwnd: HWND) -> Option<(i32, i32, Vec<u8>)> {
        let mut r: RECT = zeroed();
        if GetWindowRect(hwnd, &mut r) == 0 {
            return None;
        }
        let (w, h) = (r.right - r.left, r.bottom - r.top);
        if w <= 0 || h <= 0 {
            return None;
        }
        let screen: HDC = GetDC(null_mut());
        let mem = CreateCompatibleDC(screen);
        let bmp: HBITMAP = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(mem, bmp as _);
        let ok = PrintWindow(hwnd, mem, PW_RENDERFULLCONTENT);
        SelectObject(mem, old);

        let mut bmi: BITMAPINFO = zeroed();
        bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as DWORD;
        bmi.bmiHeader.biWidth = w;
        bmi.bmiHeader.biHeight = -h; // top-down
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
            eprintln!("  PrintWindow ok={ok}, GetDIBits lines={got}");
        }
        Some((w, h, buf))
    }

    fn nonblack_ratio(buf: &[u8]) -> f64 {
        let mut n = 0usize;
        let total = buf.len() / 4;
        for px in buf.chunks_exact(4) {
            if px[0] > 16 || px[1] > 16 || px[2] > 16 {
                n += 1;
            }
        }
        if total == 0 {
            0.0
        } else {
            n as f64 / total as f64
        }
    }

    fn save_png(path: &str, w: i32, h: i32, bgra: &[u8]) {
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for px in bgra.chunks_exact(4) {
            rgb.push(px[2]);
            rgb.push(px[1]);
            rgb.push(px[0]);
        }
        if let Some(img) = image::RgbImage::from_raw(w as u32, h as u32, rgb) {
            let _ = img.save(path);
        }
    }

    pub unsafe fn run() {
        let args: Vec<String> = std::env::args().collect();
        let exe = args.get(1).cloned().unwrap_or_else(|| "notepad.exe".into());
        let out = args.get(2).cloned().unwrap_or_else(|| ".".into());

        let desk_name = wide("ArnaBubblePoc");
        let hdesk = CreateDesktopW(
            desk_name.as_ptr(),
            null(),
            null_mut(),
            0,
            GENERIC_ALL,
            null_mut(),
        );
        if hdesk.is_null() {
            eprintln!("CreateDesktopW failed (err {})", err());
            return;
        }
        println!("[bubble] created hidden desktop 'ArnaBubblePoc'");

        // Launch the app *on that desktop* — it won't appear on the real one.
        let mut si: STARTUPINFOW = zeroed();
        si.cb = size_of::<STARTUPINFOW>() as DWORD;
        let mut deskbuf = wide("ArnaBubblePoc");
        si.lpDesktop = deskbuf.as_mut_ptr();
        si.dwFlags = STARTF_USESHOWWINDOW;
        si.wShowWindow = SW_SHOW;
        let mut pi: PROCESS_INFORMATION = zeroed();
        let mut cmd = wide(&exe);
        let ok = CreateProcessW(
            null(),
            cmd.as_mut_ptr(),
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
            eprintln!("CreateProcessW('{exe}') failed (err {})", err());
            CloseDesktop(hdesk);
            return;
        }
        println!("[bubble] launched '{exe}' on the bubble desktop (pid {})", pi.dwProcessId);

        std::thread::sleep(std::time::Duration::from_millis(3500));

        // Find its main window on the bubble desktop (prefer the launched pid,
        // else the largest window on this otherwise-empty desktop).
        let mut find = Find {
            pid: pi.dwProcessId,
            best_pid: null_mut(),
            best_pid_area: 0,
            best_any: null_mut(),
            best_any_area: 0,
        };
        EnumDesktopWindows(hdesk, Some(enum_cb), &mut find as *mut _ as LPARAM);
        let target = if !find.best_pid.is_null() { find.best_pid } else { find.best_any };
        if target.is_null() {
            eprintln!("[bubble] no window found on the bubble desktop (packaged app? launched elsewhere?)");
            CloseDesktop(hdesk);
            return;
        }
        println!(
            "[bubble] window {:p} (pid-match: {}, area {})",
            target,
            !find.best_pid.is_null(),
            find.best_any_area.max(find.best_pid_area)
        );

        match capture(target) {
            Some((w, h, buf)) => {
                let ratio = nonblack_ratio(&buf);
                let p = format!("{out}/bubble_before.png");
                save_png(&p, w, h, &buf);
                println!("[bubble] captured {w}x{h}, non-black {:.1}%  -> {p}", ratio * 100.0);
                if ratio < 0.005 {
                    println!("[bubble] WARNING: capture is essentially black (GPU app? occluded?)");
                }
            }
            None => println!("[bubble] capture failed"),
        }

        // Type into the bubble app via PostMessage (no global cursor needed).
        // Target a child Edit control if there is one (text shows up there).
        let edit = find_edit(target);
        let dest = if !edit.is_null() { edit } else { target };
        let text = "ARNA-BUBBLE-OK";
        for ch in text.encode_utf16() {
            PostMessageW(dest, WM_CHAR, ch as usize, 0);
        }
        println!(
            "[bubble] posted {} chars into {} ",
            text.len(),
            if edit.is_null() { "the window" } else { "an Edit control" }
        );
        std::thread::sleep(std::time::Duration::from_millis(1200));

        if let Some((w, h, buf)) = capture(target) {
            let ratio = nonblack_ratio(&buf);
            let p = format!("{out}/bubble_after.png");
            save_png(&p, w, h, &buf);
            println!("[bubble] after-typing capture non-black {:.1}%  -> {p}", ratio * 100.0);
        }

        // Leave the app + desktop for inspection; in the real feature we'd clean up.
        println!("[bubble] done. (desktop + app left running; pid {})", pi.dwProcessId);
        CloseDesktop(hdesk);
    }

    fn err() -> u32 {
        unsafe { winapi::um::errhandlingapi::GetLastError() }
    }
}
