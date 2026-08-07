//! A Windows **Job Object** as a workspace's process-ownership boundary.
//!
//! The workspace owns one Job. Every process WSE launches for it is assigned to
//! that Job, and the Job is configured `KILL_ON_JOB_CLOSE` — so closing it (on
//! `drop`, or explicit `terminate`) tears down the entire process tree, children
//! included. This is the OS-supported, application-layer way to own and clean up
//! a process group: no Toolhelp walking, no orphaned handles/processes. It is the
//! native runtime's honest answer to "the workspace owns its process tree".

use std::ffi::c_void;

type Handle = *mut c_void;

const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;
// JOBOBJECTINFOCLASS::JobObjectExtendedLimitInformation
const JOB_EXTENDED_LIMIT_INFO_CLASS: i32 = 9;

#[repr(C)]
struct JobBasicLimitInformation {
    per_process_user_time_limit: i64,
    per_job_user_time_limit: i64,
    limit_flags: u32,
    minimum_working_set_size: usize,
    maximum_working_set_size: usize,
    active_process_limit: u32,
    affinity: usize,
    priority_class: u32,
    scheduling_class: u32,
}

#[repr(C)]
struct IoCounters {
    read_operation_count: u64,
    write_operation_count: u64,
    other_operation_count: u64,
    read_transfer_count: u64,
    write_transfer_count: u64,
    other_transfer_count: u64,
}

#[repr(C)]
struct JobExtendedLimitInformation {
    basic: JobBasicLimitInformation,
    io: IoCounters,
    process_memory_limit: usize,
    job_memory_limit: usize,
    peak_process_memory_used: usize,
    peak_job_memory_used: usize,
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateJobObjectW(sa: *const c_void, name: *const u16) -> Handle;
    fn SetInformationJobObject(job: Handle, class: i32, info: *const c_void, len: u32) -> i32;
    fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
    fn TerminateJobObject(job: Handle, exit_code: u32) -> i32;
    fn CloseHandle(h: Handle) -> i32;
}

/// Owns a Job handle. Dropping it closes the Job; because the Job is created with
/// `KILL_ON_JOB_CLOSE`, that terminates every process still assigned to it.
pub struct Job(Handle);

// The handle is only ever assigned/terminated/closed; safe to hold in the
// runtime's per-workspace state (which lives on one thread anyway).
unsafe impl Send for Job {}

impl Job {
    /// Create a Job that kills all its processes when the Job is closed.
    pub(crate) fn create() -> Option<Job> {
        unsafe {
            let h = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if h.is_null() {
                return None;
            }
            let mut info: JobExtendedLimitInformation = std::mem::zeroed();
            info.basic.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                h,
                JOB_EXTENDED_LIMIT_INFO_CLASS,
                &info as *const _ as *const c_void,
                std::mem::size_of::<JobExtendedLimitInformation>() as u32,
            );
            Some(Job(h))
        }
    }

    /// Assign a process (by its handle) to this Job. Do this before the process
    /// resumes, so processes it spawns at startup are captured too.
    pub(crate) fn assign(&self, process: Handle) -> bool {
        unsafe { AssignProcessToJobObject(self.0, process) != 0 }
    }

    /// Terminate every process in the Job now (explicit teardown, before drop).
    #[allow(dead_code)]
    pub(crate) fn terminate(&self) {
        unsafe {
            TerminateJobObject(self.0, 1);
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
