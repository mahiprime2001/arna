//! The Windows adapter's conformance test is ONE line — identical to the mock's.
//! It is `#[ignore]` because it creates and destroys real WSL2 workspaces
//! (needs Windows + WSL2, is slow, and modifies machine state). Run it
//! explicitly:
//!
//!   cargo test -p wse-adapter-windows -- --ignored --nocapture
//!
//! v1 expectation: run_all == run_core (this adapter declares no capabilities),
//! so it passes lifecycle + isolation and honestly skips every capability suite.

use wse_adapter_windows::WindowsAdapter;

#[test]
#[ignore = "requires Windows + WSL2; creates and destroys real workspaces"]
fn windows_adapter_is_conformant() {
    let report = wse_conformance::run_all(WindowsAdapter::new);
    println!("{}", report.summary());
    report.assert_ok();
}
