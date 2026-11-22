//! Common test utilities for cross-family TRS tests.
//!
//! Shared market fixtures live in `fixed-income/tests/support/trs_market.rs`,
//! FI-index TRS builders in `fixed-income/tests/support/trs.rs`, and equity TRS
//! builders in `equity/tests/support/trs.rs`.

#[allow(dead_code)]
#[path = "../../../fixed-income/tests/support/trs_market.rs"]
mod trs_market;

#[allow(dead_code)]
#[path = "../../../fixed-income/tests/support/trs.rs"]
mod fi_trs;

#[allow(dead_code)]
#[path = "../../../equity/tests/support/trs.rs"]
mod equity_trs;

#[allow(unused_imports)]
pub use equity_trs::*;
#[allow(unused_imports)]
pub use fi_trs::*;
#[allow(unused_imports)]
pub use trs_market::*;
