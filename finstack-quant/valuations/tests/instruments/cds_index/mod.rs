//! CDS Index comprehensive test suite.
//!
//! Test organization:
//! - `construction`: Instrument construction, builders, and validation
//! - `pricing_single_curve`: Single-curve pricing mode tests
//! - `pricing_constituents`: Constituents pricing mode tests
//! - `pricing_parity`: Cross-mode consistency validation
//! - `metrics_basic`: Basic metrics (NPV, leg PVs, par spread)
//! - `metrics_risk`: Risk metrics (DV01, CS01, Risky PV01)
//! - `metrics_advanced`: Advanced metrics (JTD, Expected Loss, Theta, Bucketed DV01)
//! - `market_conventions`: CDX/iTraxx convention tests
//! - `edge_cases`: Edge cases and error handling
//! - `test_utils`: Shared test utilities and helpers

mod construction;
mod edge_cases;
mod market_conventions;
mod metrics_advanced;
mod metrics_basic;
mod metrics_risk;
mod pricing_constituents;
mod pricing_parity;
mod pricing_single_curve;
mod test_imm_factor_cs01;
mod test_utils;
