//! SPIKE (throwaway): can Windows' OWN AppContainer isolate a process's file
//! access — so a native runtime could provide real isolation WITHOUT Docker?
//!
//! It creates an AppContainer, then launches `cmd /c type <file>` twice: once
//! normally (control) and once inside the AppContainer. If the normal run can
//! read a file in the user profile but the AppContainer run is DENIED, then
//! Windows-native filesystem isolation is real and usable. See
//! docs/native-isolation-spike.md. Uses only documented Win32 APIs.

use std::ffi::c_void;

type Handle = *mut c_void;
type Psid = *mut c_void;

const EXTENDED_STARTUPINFO_PRESENT: u32 = 0x0008_0000;
const PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES: usize = 0x0002_0009;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const INFINITE: u32 = 0xFFFF_FFFF;
const ERROR_ALREADY_EXISTS_HR: i32 = -2_147_024_713; // HRESULT_FROM_WIN32(ERROR_ALREADY_EXISTS)

#[repr(C)]
struct StartupInfoW {
    cb: u32,
    lp_reserved: *mut u16,
    lp_desktop: *mut u16,
    lp_title: *mut u16,
    dw_x: u32,
    dw_y: u32,
    dw_x_size: u32,
    dw_y_size: u32,
    dw_x_count_chars: u32,
    dw_y_count_chars: u32,
    dw_fill_attribute: u32,
    dw_flags: u32,
    w_show_window: u16,
    cb_reserved2: u16,
    lp_reserved2: *mut u8,
    h_std_input: Handle,
    h_std_output: Handle,
    h_std_error: Handle,
}

#[repr(C)]
struct StartupInfoExW {
    info: StartupInfoW,
    attr_list: *mut c_void,
}

#[repr(C)]
struct ProcessInformation {
    h_process: Handle,
    h_thread: Handle,
    pid: u32,
    tid: u32,
}

#[repr(C)]
struct SecurityCapabilities {
    app_container_sid: Psid,
    capabilities: *mut c_void,
    capability_count: u32,
    reserved: u32,
}

#[link(name = "userenv")]
extern "system" {
    fn CreateAppContainerProfile(
        name: *const u16,
        display: *const u16,
        desc: *const u16,
        caps: *mut c_void,
        count: u32,
        sid: *mut Psid,
    ) -> i32;
    fn DeriveAppContainerSidFromAppContainerName(name: *const u16, sid: *mut Psid) -> i32;
    fn DeleteAppContainerProfile(name: *const u16) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn InitializeProcThreadAttributeList(
        list: *mut c_void,
        count: u32,
        flags: u32,
        size: *mut usize,
    ) -> i32;
    fn UpdateProcThreadAttribute(
        list: *mut c_void,
        flags: u32,
        attr: usize,
        value: *const c_void,
        size: usize,
        prev: *mut c_void,
        ret: *mut usize,
    ) -> i32;
    fn DeleteProcThreadAttributeList(list: *mut c_void);
    fn CreateProcessW(
        app: *const u16,
        cmd: *mut u16,
        pa: *const c_void,
        ta: *const c_void,
        inherit: i32,
        flags: u32,
        env: *const c_void,
        dir: *const u16,
        si: *const c_void,
        pi: *mut ProcessInformation,
    ) -> i32;
    fn WaitForSingleObject(h: Handle, ms: u32) -> u32;
    fn GetExitCodeProcess(h: Handle, code: *mut u32) -> i32;
    fn CloseHandle(h: Handle) -> i32;
    fn LocalFree(p: *mut c_void) -> *mut c_void;
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Run `cmd /c type <probe>` and return its exit code. If `sid` is Some, the
/// process runs inside that AppContainer. None = a normal (control) process.
fn run_type(probe: &str, sid: Option<Psid>) -> Option<u32> {
    let cmdline = format!("cmd.exe /c type \"{probe}\" >nul 2>nul");
    let mut cmd = wide(&cmdline);

    let mut si_ex: StartupInfoExW = unsafe { std::mem::zeroed() };
    si_ex.info.cb = std::mem::size_of::<StartupInfoExW>() as u32;
    let mut pi: ProcessInformation = unsafe { std::mem::zeroed() };

    // Attribute-list backing store must outlive CreateProcess.
    let mut attr_buf: Vec<u8> = Vec::new();
    let mut sec: SecurityCapabilities = unsafe { std::mem::zeroed() };
    let mut flags = CREATE_NO_WINDOW;

    unsafe {
        if let Some(sid) = sid {
            let mut size: usize = 0;
            InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut size);
            attr_buf = vec![0u8; size];
            if InitializeProcThreadAttributeList(attr_buf.as_mut_ptr() as *mut c_void, 1, 0, &mut size)
                == 0
            {
                return None;
            }
            sec.app_container_sid = sid;
            if UpdateProcThreadAttribute(
                attr_buf.as_mut_ptr() as *mut c_void,
                0,
                PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
                &sec as *const _ as *const c_void,
                std::mem::size_of::<SecurityCapabilities>(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            ) == 0
            {
                return None;
            }
            si_ex.attr_list = attr_buf.as_mut_ptr() as *mut c_void;
            flags |= EXTENDED_STARTUPINFO_PRESENT;
        }

        let ok = CreateProcessW(
            std::ptr::null(),
            cmd.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            flags,
            std::ptr::null(),
            std::ptr::null(),
            &si_ex as *const _ as *const c_void,
            &mut pi,
        );
        if !attr_buf.is_empty() {
            DeleteProcThreadAttributeList(attr_buf.as_mut_ptr() as *mut c_void);
        }
        if ok == 0 {
            return None; // couldn't launch (distinct from "launched but denied")
        }
        WaitForSingleObject(pi.h_process, INFINITE);
        let mut code: u32 = 0;
        GetExitCodeProcess(pi.h_process, &mut code);
        CloseHandle(pi.h_thread);
        CloseHandle(pi.h_process);
        Some(code)
    }
}

pub fn probe_appcontainer_fs_isolation() -> String {
    let name = wide("wse-spike-appcontainer");
    let mut sid: Psid = std::ptr::null_mut();

    unsafe {
        let hr = CreateAppContainerProfile(
            name.as_ptr(),
            wide("WSE Spike").as_ptr(),
            wide("WSE native-isolation spike").as_ptr(),
            std::ptr::null_mut(),
            0,
            &mut sid,
        );
        if hr != 0 && hr != ERROR_ALREADY_EXISTS_HR {
            return format!("CreateAppContainerProfile failed: hr=0x{:08x}", hr as u32);
        }
        if sid.is_null() {
            // Profile already existed — derive its SID.
            if DeriveAppContainerSidFromAppContainerName(name.as_ptr(), &mut sid) != 0 || sid.is_null()
            {
                return "could not obtain AppContainer SID".into();
            }
        }
    }

    // A file in the user profile a normal process reads fine.
    let probe = std::env::var("USERPROFILE").unwrap_or_default() + "\\wse-appc-probe.txt";
    let _ = std::fs::write(&probe, b"secret-host-file");

    let control = run_type(&probe, None);
    let contained = run_type(&probe, Some(sid));

    let _ = std::fs::remove_file(&probe);
    unsafe {
        DeleteAppContainerProfile(name.as_ptr());
        LocalFree(sid);
    }

    let verdict = match (control, contained) {
        (Some(0), Some(c)) if c != 0 => "ISOLATION WORKS (host read OK, AppContainer DENIED)",
        (Some(0), Some(0)) => "NO ISOLATION (AppContainer read the host file too)",
        (Some(0), None) => "AppContainer process FAILED TO LAUNCH (needs exe/capability grant)",
        _ => "INCONCLUSIVE (control read did not succeed)",
    };
    format!(
        "control_exit={control:?}  appcontainer_exit={contained:?}  => {verdict}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "creates a Windows AppContainer + launches processes (spike; --nocapture)"]
    fn spike_appcontainer_fs_isolation() {
        println!("SPIKE: {}", probe_appcontainer_fs_isolation());
    }
}
