//! Unit tests for Total Return Swap (TRS) instruments.
//!
//! This test module provides comprehensive coverage of TRS functionality including:
//! - Core types and enums (TrsScheduleSpec, PayReceive side)
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
//! - `test_trs_metrics` - All risk metrics calculations
//! - `test_trs_edge_cases` - Boundary conditions and error handling
//!
//! Single-family TRS tests live in `fixed-income/tests/fi_trs/` and
//! `equity/tests/equity_trs/`.

mod test_utils;

mod test_trs_curve_dependencies;
mod test_trs_edge_cases;
mod test_trs_metrics;
mod test_trs_types;
