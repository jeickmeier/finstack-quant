//! Shared components for revolving credit pricing.
//!
//! Currently exposes only `compute_upfront_fee_pv`. Earlier iterations of this
//! file carried a speculative `RateProjector` trait family (with `FixedRateProjector`,
//! `FloatingRateProjector`, `TermLockedRateProjector`) plus `DiscountFactors`,
//! `SurvivalWeights`, and `FeeCalculator` infrastructure. None of it was wired
//! into the live pricing path — the `unified.rs` engine and the MC path generator
//! resolve forward curves and survival probabilities directly. The dead scaffolding
//! has been removed; revive from git history if a future stochastic-fee path
//! genuinely needs it.

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

/// Helper to compute upfront fee present value.
///
/// Only includes the upfront fee when the commitment date is strictly after the
/// valuation date, consistent with "PV of remaining cashflows" semantics.
/// When `commitment_date <= as_of` the fee has already been paid and is excluded
/// from the mark-to-market valuation.
pub(crate) fn compute_upfront_fee_pv(
    upfront_fee_opt: Option<Money>,
    commitment_date: Date,
    as_of: Date,
    disc_curve: &dyn Discounting,
) -> Result<f64> {
    let Some(upfront_fee) = upfront_fee_opt else {
        return Ok(0.0);
    };

    if commitment_date > as_of {
        let df = disc_curve.df_between_dates(as_of, commitment_date)?;
        Ok(upfront_fee.amount() * df)
    } else {
        Ok(0.0)
    }
}
