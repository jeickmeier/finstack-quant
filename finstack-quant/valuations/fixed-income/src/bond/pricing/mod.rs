//! Bond pricing engines and utilities.
//!
//! # Engines
//!
//! Each engine implements the core pricing math for one model; the shared
//! `BondPricer` registry adapter preserves the caller's explicit `ModelKey`
//! selection:
//! - `discount`: Non-callable PV = sum(CF_i * DF_i) from projected
//!   cashflows and discount curves; an attached credit curve is ignored
//! - `hazard`: Non-callable survival-weighted PV plus fractional recovery of
//!   par (FRP); an explicit hazard curve is required
//! - [`tree`]: Rates-only and joint rates-credit option rollback and OAS,
//!   including call, put, and return-floor rights
//! - [`merton_mc`]: Merton structural credit Monte Carlo for PIK bonds
//!
//! # Utilities
//!
//! - [`quote_conversions`]: Price/yield/spread conversion functions
//! - [`ytm_solver`]: Robust yield-to-maturity calculation
//! - `settlement`: Settlement date and accrued interest utilities
//!
//! [`tree`]: crate::bond::pricing::tree
//! [`merton_mc`]: crate::bond::pricing::merton_mc
//! [`quote_conversions`]: crate::bond::pricing::quote_conversions
//! [`ytm_solver`]: crate::bond::pricing::ytm_solver

/// Discount curve-based bond pricing (PV = sum CF_i * DF_i).
pub(crate) mod discount;
/// Non-callable hazard-rate pricing with fractional recovery of par.
pub(crate) mod hazard;
/// Merton structural credit Monte Carlo for PIK bonds.
pub mod merton_mc;
mod pricer;
pub mod quote_conversions;
pub(crate) mod return_floor;
pub(crate) mod settlement;
pub(crate) mod time_basis;
/// Short-rate and rates-credit option pricing and OAS.
pub mod tree;
pub mod ytm_solver;

pub(crate) use merton_mc::SimpleBondMertonMcPricer;

pub(crate) use pricer::BondPricer;
