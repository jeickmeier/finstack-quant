//! Par fixed rate from actual floating-leg PV divided by fixed-leg unit annuity.
//! The same contractual coupons, historical fixings, dates and payment lags
//! drive par quotation and pricing, including mismatched leg maturities.
use crate::InterestRateSwap;
use finstack_quant_valuations_core::instruments::common_impl::pricing::swap_legs::ANNUITY_EPSILON;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricId};

pub(crate) struct ParRateCalculator;

impl MetricCalculator for ParRateCalculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Annuity]
    }
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        par_rate_pv_based(context.instrument_as()?, context)
    }
}

/// Par rate calculation based on PV of the floating leg.
///
/// Refactored to reuse the pricer's own floating leg PV logic for perfect consistency.
fn par_rate_pv_based(
    irs: &InterestRateSwap,
    ctx: &MetricContext,
) -> finstack_quant_core::Result<f64> {
    let as_of = ctx.as_of;
    let annuity = ctx.computed.get(&MetricId::Annuity).copied().unwrap_or(0.0);

    if annuity.abs() < ANNUITY_EPSILON {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Par rate calculation failed: annuity ({:.2e}) is below numerical stability \
             threshold ({:.2e}).",
            annuity, ANNUITY_EPSILON
        )));
    }

    let pv_float = irs.pv_float_leg(&ctx.curves, as_of)?;

    // Par rate = float_pv / (notional * annuity)
    Ok(pv_float / (irs.notional.amount() * annuity))
}
