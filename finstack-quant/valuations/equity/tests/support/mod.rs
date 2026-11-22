//! Shared helpers for equity integration tests (also `#[path]`-included by the
//! umbrella crate where cross-family tests need equity builders or direct Greeks).

pub mod greeks;
pub mod options;
pub mod trs;
#[allow(dead_code)]
#[path = "../../../fixed-income/tests/support/trs_market.rs"]
pub mod trs_market;
