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
//! - `oas`: Option-adjusted spread
//! - `ytw`: Yield to worst
//! - `theta`: Time decay
//!
//! Note: DV01 tests are consolidated in `tests/metrics/tests/dv01.rs` which provides
//! comprehensive coverage across Bond, IRS, and Deposit instruments.

mod accrued;
mod asw;
mod bucketed_dv01;
mod convexity;
mod credit_quote_risk;
mod cs01;
mod dm;
mod duration;
mod oas;
mod prices;
mod quote_engine;
mod spreads;
mod theta;
mod wal;
mod ytm;
mod ytw;
