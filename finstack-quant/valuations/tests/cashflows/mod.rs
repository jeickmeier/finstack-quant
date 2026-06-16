//! Cashflows test suite modules.
//!
//! This module consolidates:
//! - `provider_contract`: `CashflowProvider` trait contract compliance
//! - `bridge_smoke`: compatibility smoke tests for `finstack_quant_cashflows::*`

pub(crate) mod helpers;

#[allow(clippy::expect_used, clippy::unwrap_used, dead_code, unused_imports)]
pub(crate) mod finstack_quant_test_utils {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/test_utils.rs"
    ));
}

mod bridge_smoke;
mod instrument_bridge;
mod provider_contract;
