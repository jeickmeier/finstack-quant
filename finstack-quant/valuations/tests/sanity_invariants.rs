//! Sanity / invariant tests -- verify internal consistency of pricing.
//!
//! These tests do NOT compare against external references (QuantLib, Bloomberg).
//! They assert internal properties: par-rate self-consistency, pay/receive symmetry,
//! DV01 magnitude bands. For external-reference parity, see `tests/golden/`.

#[allow(clippy::expect_used, clippy::unwrap_used, dead_code, unused_imports)]
mod finstack_quant_test_utils {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/test_utils.rs"
    ));
}

#[path = "sanity_invariants/mod.rs"]
mod sanity_invariants_tests;
