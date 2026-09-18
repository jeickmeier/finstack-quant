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

/// Present value of the upfront fee at `as_of`.
///
/// Only includes the fee when the commitment date is strictly after the
/// valuation date, consistent with "PV of remaining cashflows" semantics.
/// When `commitment_date <= as_of` the fee has already been paid and is
/// excluded from the mark-to-market valuation.
///
/// # Arguments
///
/// * `upfront_fee` - Fee amount in the facility currency (the percentage
///   form already resolved against the opening commitment).
/// * `commitment_date` - Date the fee is paid.
/// * `as_of` - Valuation date.
/// * `disc_curve` - Curve discounting the fee from the commitment date.
pub(crate) fn compute_upfront_fee_pv(
    upfront_fee: Money,
    commitment_date: Date,
    as_of: Date,
    disc_curve: &dyn Discounting,
) -> Result<f64> {
    if upfront_fee.amount() == 0.0 || commitment_date <= as_of {
        return Ok(0.0);
    }
    let df = disc_curve.df_between_dates(as_of, commitment_date)?;
    Ok(upfront_fee.amount() * df)
}
