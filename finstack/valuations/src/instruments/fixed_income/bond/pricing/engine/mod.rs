//! Bond pricing engines.
//!
//! Each engine implements the core pricing math for a specific model,
//! co-located with its thin registry adapter (the `Simple*` `Pricer` struct):
//!
//! - [`self::discount`]: PV = sum(CF_i * DF_i) using discount curves
//! - [`self::hazard`]: Survival-weighted PV + fractional recovery of par (FRP)
//! - [`self::tree`]: Binomial/trinomial tree for callable/putable bonds and OAS
//! - `merton_mc` + `merton_mc_pricer`: Merton structural credit Monte Carlo for PIK bonds

/// Discount curve-based bond pricing (PV = sum CF_i * DF_i).
pub mod discount;
/// Hazard-rate pricing with fractional recovery of par (FRP).
pub mod hazard;
/// Merton structural credit Monte Carlo for PIK bonds.
pub mod merton_mc;
/// Registry adapter for the Merton MC pricer.
pub(crate) mod merton_mc_pricer;
/// Binomial tree pricing for callable/putable bonds and OAS.
pub mod tree;

pub(crate) use hazard::SimpleBondHazardPricer;
pub(crate) use merton_mc_pricer::SimpleBondMertonMcPricer;
pub(crate) use tree::SimpleBondOasPricer;
