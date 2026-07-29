//! The Windows adapter's conformance tests. Both are `#[ignore]` because they
//! create and destroy real WSL2 workspaces (need Windows + WSL2, are slow, and
//! modify machine state). Run them explicitly:
//!
//!   cargo test -p wse-adapter-windows -- --ignored --nocapture
//!
//! Together they prove the RUNTIME boundary: the *same adapter* satisfies the
//! contract on two different runtimes, negotiating a different effective
//! capability set (adapter ∩ runtime) with no code change.

use wse_adapter_windows::{RuntimeSpec, WindowsAdapter};

/// On wse-linux-x11 v1.0.0 (display stack): Applications + Windows are provided,
/// so run_all == core + applications + windows.
#[test]
#[ignore = "requires Windows + WSL2 + the wse-linux-x11 image"]
fn windows_adapter_conforms_on_linux_x11_runtime() {
    let report = wse_conformance::run_all(WindowsAdapter::new);
    println!("linux-x11: {}", report.summary());
    report.assert_ok();
}

/// On wse-lite v1.0.0 (minimal/headless): NO capabilities are provided, so the
/// SAME adapter negotiates effective = {} and run_all == core only. Applications
/// and Windows are naturally unavailable — no special cases. This is the runtime
/// interchangeability proof.
#[test]
#[ignore = "requires Windows + WSL2 + the wse-lite image"]
fn windows_adapter_conforms_on_lite_runtime() {
    let make = || WindowsAdapter::with_runtime(RuntimeSpec::lite_v1());
    let report = wse_conformance::run_all(make);
    println!("lite: {}", report.summary());
    report.assert_ok();
}
