//! Pricing and valuation metrics for structured credit.

pub(crate) mod prices;
pub(crate) mod wal;

use crate::structured_credit::types::DealAccruals;
use crate::structured_credit::types::TrancheCashflows;
use crate::structured_credit::StructuredCredit;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
pub(crate) use prices::{CleanPriceCalculator, DirtyPriceCalculator};
pub use wal::calculate_tranche_wal;
pub(crate) use wal::WalCalculator;

/// Calculates current-period contractual accrued interest in note currency.
///
/// Opening balances precede projected defaults and principal distributions.
/// Prior deferred coupons and equity residual distributions do not create
/// additional current-period accrued interest.
pub(crate) struct AccruedCalculator;

impl MetricCalculator for AccruedCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let settlement = super::quote::settlement_date(
            context.instrument_as::<StructuredCredit>()?,
            context.as_of,
        )?;
        accrued_at(context, settlement)
    }
}

pub(crate) fn accrued_at(
    context: &mut MetricContext,
    settlement: finstack_quant_core::dates::Date,
) -> Result<f64> {
    if let Some(details) = context.ext::<TrancheCashflows>() {
        return details
            .accrual_periods
            .iter()
            .try_fold(0.0, |sum, period| Ok(sum + period.accrued(settlement)?));
    }
    if context.ext::<DealAccruals>().is_none() {
        let deal = context.instrument_as::<StructuredCredit>()?;
        let results = deal
            .run_simulation(&context.curves, context.as_of)?
            .tranches;
        context.set_ext(DealAccruals(
            results
                .into_values()
                .flat_map(|result| result.accrual_periods)
                .collect(),
        ));
    }
    context
        .ext::<DealAccruals>()
        .map(|accruals| accruals.0.as_slice())
        .unwrap_or_default()
        .iter()
        .try_fold(0.0, |sum, period| Ok(sum + period.accrued(settlement)?))
}
