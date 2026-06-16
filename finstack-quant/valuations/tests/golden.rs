//! Golden tests: external-reference parity for pricing, calibration, analytics, and attribution.

#[allow(clippy::expect_used, clippy::unwrap_used, dead_code, unused_imports)]
mod finstack_quant_test_utils {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/test_utils.rs"
    ));
}

#[path = "golden/mod.rs"]
mod golden;
