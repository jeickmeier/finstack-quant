//! AssetPool characteristic metrics for structured credit.

use crate::structured_credit::StructuredCredit;
use crate::structured_credit::types::pool::weighted_avg_rating_factor;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// CLO WARF calculator - Moody's methodology
pub struct CloWarfCalculator;

impl finstack_quant_valuations_core::metrics::MetricCalculator for CloWarfCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let clo = context
            .instrument_as::<crate::structured_credit::StructuredCredit>()?
            .resolved_for_pricing()?;

        // Performing par only: defaulted assets are carried at recovery in
        // OC haircuts, not in the rating-factor average. A metric is a number: no
        // performing balance reports zero.
        Ok(weighted_avg_rating_factor(
            clo.pool
                .assets
                .iter()
                .filter(|asset| !asset.defaulted)
                .map(|asset| (asset.balance.amount(), asset.rating)),
        )?
        .unwrap_or(0.0))
    }
}

/// CLO WAS calculator - in basis points
///
/// Market standard: WAS uses the **spread component only**, not the all-in
/// coupon, over **performing** assets only. Fixed-rate assets without an
/// explicit `spread_bp` are excluded (no all-in-rate fallback), as are
/// defaulted assets — see [`AssetPool::weighted_avg_spread_bp`].
///
/// [`AssetPool::weighted_avg_spread_bp`]: crate::structured_credit::types::AssetPool::weighted_avg_spread_bp
pub(crate) struct CloWasCalculator;

impl finstack_quant_valuations_core::metrics::MetricCalculator for CloWasCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let clo = context
            .instrument_as::<crate::structured_credit::StructuredCredit>()?
            .resolved_for_pricing()?;

        // A metric is a number: no spread-bearing collateral reports zero.
        Ok(clo.pool.weighted_avg_spread_bp().unwrap_or(0.0))
    }
}

// AssetPool characteristic metrics for structured credit.

/// Calculates WAM (Weighted Average Maturity) for the underlying pool.
///
/// WAM measures the weighted average time until assets in the pool mature,
/// based on the original legal maturities (not expected prepayment life like WAL).
///
/// # Formula
///
/// WAM = Σ(Balance_i × Years_to_Maturity_i) / Σ(Balance_i)
///
/// # Typical Values
///
/// - **CLO**: 5-7 years
/// - **ABS**: 3-5 years
/// - **RMBS**: 25-30 years (legal maturity, much longer than WAL)
/// - **CMBS**: 7-10 years
///
pub(crate) struct WamCalculator;

impl MetricCalculator for WamCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let deal = context
            .instrument_as::<StructuredCredit>()?
            .resolved_for_pricing()?;
        // A metric is a number: a zero-balance pool reports zero.
        Ok(deal
            .pool
            .weighted_avg_maturity(context.as_of)?
            .unwrap_or(0.0))
    }
}

/// Calculates CPR (Constant Prepayment Rate) assumption.
///
/// CPR is the annualized rate at which the pool is assumed to prepay.
/// This is an assumption/input rather than a calculated output.
///
/// # Formula
///
/// CPR = annualized prepayment rate (e.g., 0.15 = 15% CPR)
///
/// # Sources
///
/// - **RMBS**: Derived from PSA speed
/// - **ABS**: From ABS speed assumption
/// - **CMBS**: From open period CPR
/// - **CLO**: Default prepayment assumption
///
pub struct CprCalculator;

impl MetricCalculator for CprCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let deal = context
            .instrument_as::<StructuredCredit>()?
            .resolved_for_pricing()?;
        let age = deal
            .pool
            .seasoning_months(context.as_of, deal.closing_date)
            .max(1);
        finstack_quant_cashflows::builder::monthly_to_annual(
            deal.credit_model.prepayment_spec.smm(age)?,
        )
    }
}

/// Calculates CDR (Constant Default Rate) assumption.
///
/// CDR is the annualized rate at which the pool is assumed to default.
/// This is an assumption/input rather than a calculated output.
///
/// # Formula
///
/// CDR = annualized default rate (e.g., 0.02 = 2% CDR)
///
/// # Typical Values
///
/// - **CLO**: 2-3% base case, 5-10% stress
/// - **ABS (auto)**: 1-2% base case
/// - **RMBS**: 0.5-1% base case (agency)
/// - **CMBS**: 0.5-1% base case
///
pub struct CdrCalculator;

impl MetricCalculator for CdrCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let deal = context
            .instrument_as::<StructuredCredit>()?
            .resolved_for_pricing()?;
        let age = deal
            .pool
            .seasoning_months(context.as_of, deal.closing_date)
            .max(1);
        finstack_quant_cashflows::builder::monthly_to_annual(
            deal.credit_model.default_spec.mdr(age)?,
        )
    }
}
