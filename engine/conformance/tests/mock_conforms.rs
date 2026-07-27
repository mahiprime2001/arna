//! Proof that the harness works: the mock adapter runs the SAME suite every
//! real adapter will. When the Windows adapter lands, its conformance test is
//! one line identical to this — no special-casing.

use wse_adapter_mock::MockAdapter;

#[test]
fn mock_adapter_is_conformant() {
    // run_all runs the mandatory core PLUS the suites for every capability the
    // mock declares (Applications, Windows) -- and nothing it doesn't.
    let report = wse_conformance::run_all(MockAdapter::new);
    println!("{}", report.summary());
    report.assert_ok();
}
