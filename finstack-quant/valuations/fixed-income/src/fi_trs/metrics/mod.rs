//! Risk metrics for Fixed Income Index Total Return Swaps.
//!
//! This module provides FI TRS-specific risk metrics:
//! - **DurationDv01**: Duration-based yield sensitivity (`N × D × 1bp`)
//! - **ParSpread**: Spread that makes NPV = 0
//! - **FinancingAnnuity**: Discounted notional-weighted financing accrual sum
//! - **Dv01 / BucketedDv01**: Financing curve rate sensitivity

mod par_spread;
mod register;
#[cfg(test)]
mod tests;

pub(crate) use par_spread::ParSpreadCalculator;
#[cfg(test)]
use register::DurationDv01Calculator;
pub(crate) use register::register_fi_trs_metrics;
