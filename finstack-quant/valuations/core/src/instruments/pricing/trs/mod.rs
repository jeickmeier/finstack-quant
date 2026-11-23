//! Total Return Swap (TRS) pricing engine.
//!
//! This module provides shared pricing infrastructure for equity and fixed income
//! total return swaps. It separates the common period iteration and discounting
//! logic from underlying-specific return calculations.
//!
//! # Architecture
//!
//! The TRS pricing engine uses a trait-based approach:
//! - [`TrsReturnModel`]: Trait for underlying-specific return calculations
//! - [`TrsEngine`]: Shared pricing logic for all TRS types
//!
//! This allows equity TRS and fixed income TRS to share the common infrastructure
//! while implementing their own return calculation logic.
//!
//! # Financing-leg rate compounding
//!
//! [`TrsEngine::pv_financing_leg`] projects each period's floating rate
//! according to the financing leg's `FinancingLegSpec.compounding` setting:
//!
//! - `simple` — the **discount-factor-implied simple forward** over the accrual
//!   period ([`ForwardCurve::rate_between_dates`]), correct for a term-rate-financed TRS
//!   (e.g. 3M Term SOFR) where the period length matches the index tenor.
//! - `compounded_*` — **daily-compounds** the overnight forward via
//!   [`crate::instruments::pricing::overnight::project_overnight_coupon`]
//!   with the leg's lookback / observation shift / rate cut-off
//!   (`compounded_in_arrears` with zero lookback is cleared-OIS style).
//! - `simple_average` — rejected.
//!   The simple average would drop the daily-compounding convexity (~12–15 bp
//!   of rate at current levels).
//!
//! [`TrsEngine::financing_annuity`] sums discounted accrual fractions. Term
//! legs use the schedule year fraction; overnight legs use the projector
//! year fraction so `pv_financing_leg = pv_financing_float_only + spread × annuity`.

pub(crate) mod engine;
#[cfg(test)]
mod tests;

#[cfg(test)]
use engine::{
    FinancingLegContext, PeriodReturnInputs, TotalReturnLegParams, TrsEngine, TrsReturnModel,
    financing_period_projection, financing_period_rate,
};
