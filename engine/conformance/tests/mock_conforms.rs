//! Proof that the harness works: the mock adapter runs the SAME suite every
//! real adapter will. When the Windows adapter lands, its conformance test is
//! one line identical to this — no special-casing.

use wse_adapter_mock::MockAdapter;

#[test]
fn mock_adapter_is_conformant() {
    let report = wse_conformance::run_core(MockAdapter::new);
    println!("{}", report.summary());
    report.assert_ok();
}
