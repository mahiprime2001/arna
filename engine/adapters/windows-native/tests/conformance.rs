//! The native Windows adapter's conformance test. `#[ignore]` because it creates
//! and destroys real Windows desktops on the host. Run explicitly:
//!
//!   cargo test -p wse-adapter-windows-native -- --ignored --nocapture
//!
//! Core-only milestone: this adapter declares no capabilities yet, so
//! `run_all` == `run_core` — real desktop+profile lifecycle, honest DesktopProfile
//! isolation, and every capability honestly unavailable.

use wse_adapter_windows_native::WindowsNativeAdapter;

#[test]
#[ignore = "creates and destroys real Windows desktops on the host"]
fn windows_native_adapter_is_conformant() {
    let report = wse_conformance::run_all(WindowsNativeAdapter::new);
    println!("windows-native: {}", report.summary());
    report.assert_ok();
}
