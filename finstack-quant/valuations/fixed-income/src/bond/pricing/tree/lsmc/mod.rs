//! Least-squares Monte Carlo for option-bearing credit-risky bonds.
//!
//! Training and pricing use disjoint Philox seed domains. The regression
//! policy is fitted on standardized rate, hazard, balance, locked-coupon,
//! accrued-interest, and cumulative-distribution state, then frozen before
//! the pricing paths are sampled.
//!
//! Floating PIK changes principal at the economic accrual boundary while cash
//! settles independently on the adjusted payment date. Stochastic floating PIK
//! combined with amortization is rejected: the replay template does not carry
//! canonical runtime amortization targets, so projected repayment amounts would
//! violate remaining-balance targets on sampled paths.

pub(super) mod build;
pub(super) mod engine;
pub(super) mod policy;
mod replay;
pub(super) mod state;
#[cfg(test)]
mod tests;

pub(crate) use engine::{BondLsmcConfig, BondLsmcResult, price_bond_lsmc};
