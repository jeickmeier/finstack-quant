//! Unit tests for Total Return Swap (TRS) instruments.
//!
//! This test module provides comprehensive coverage of TRS functionality including:
//! - Core types and enums (TrsSide, TrsScheduleSpec)
//! - Equity TRS instrument creation, pricing, and risk metrics
//! - Fixed Income Index TRS instrument creation, pricing, and risk metrics
//! - Risk metrics (Par Spread, Financing Annuity, IR01, Index Delta, Theta, Bucketed DV01)
//! - Shared pricing engine logic
//! - Edge cases and error handling
//!
//! # Test Organization
//!
//! Tests are organized into logical modules:
//! - `test_utils` - Common fixtures and helper functions
//! - `test_trs_types` - Core types and enums
//! - `test_equity_trs` - Equity TRS specific tests
//! - `test_fi_index_trs` - Fixed Income Index TRS specific tests
//! - `test_trs_metrics` - All risk metrics calculations
//! - `test_trs_pricing_engine` - Shared pricing engine logic
//! - `test_trs_edge_cases` - Boundary conditions and error handling

mod test_utils;

mod test_equity_trs;
mod test_fi_index_trs;
mod test_trs_curve_dependencies;
mod test_trs_edge_cases;
mod test_trs_metrics;
mod test_trs_pricing_engine;
mod test_trs_types;
