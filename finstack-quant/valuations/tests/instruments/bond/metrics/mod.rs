//! Bond metrics calculator tests.
//!
//! Comprehensive tests for all bond metrics calculators organized by category:
//!
//! - `accrued`: Accrued interest calculation
//! - `prices`: Clean and dirty price
//! - `ytm`: Yield to maturity
//! - `duration`: Macaulay and modified duration
//! - `convexity`: Bond convexity
//! - `spreads`: Z-spread and I-spread
//! - `asw`: Asset swap spreads (par, market, forward variants)
//! - `dm`: Discount margin for FRNs
//! - `cs01`: Credit spread sensitivity
//! - `spread_duration`: CS01 normalized to years
//! - `oas`: Option-adjusted spread
//! - `ytw`: Yield to worst
//! - `theta`: Time decay
//!
//! Note: DV01 tests are consolidated in `tests/metrics/tests/dv01.rs` which provides
//! comprehensive coverage across Bond, IRS, and Deposit instruments.

mod asw;
mod callable_credit_risk;
mod credit_quote_risk;
mod cs01;
mod spread_duration;
