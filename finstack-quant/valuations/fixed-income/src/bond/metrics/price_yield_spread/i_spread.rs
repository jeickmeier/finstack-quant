//! I-spread, and the yield-over-a-curve rule it shares with G-spread.

use crate::bond::Bond;
use crate::bond::pricing::quote_conversions::{curve_par_rate, i_spread_horizon};
use crate::bond::pricing::settlement::QuoteDateContext;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricId};

/// The bond's yield less the par rate of `curve_id` at the same horizon.
///
/// For a callable or puttable bond with a quoted price the yield is the
/// yield-to-worst and the horizon is the workout date; otherwise the yield is
/// the Street YTM and the horizon is maturity. Both rates compound at the
/// bond's coupon frequency. The par rate comes from [`curve_par_rate`], which
/// interpolates the curve's swap calibration quotes when it has them and
/// otherwise computes the par rate of a proxy leg off its discount factors.
/// Reads the computed `Ytm`, so a calculator using it must depend on it.
///
/// # Arguments
///
/// * `context` - Metric context of the bond, with `Ytm` computed.
/// * `curve_id` - Discount curve in the context market to measure the yield over.
///
/// # Returns
///
/// The spread as a decimal (0.01 = 100 bp). A non-finite yield gives a
/// non-finite spread, which JSON results carry as `null`.
///
/// # Errors
///
/// Returns the market's not-found error when `curve_id` is missing, and any
/// error from the workout path or par-rate calculation. A workout on the quote
/// date has no par rate to run to, so it is an invalid-date-range error.
pub(super) fn yield_over_curve_par_rate(
    context: &mut MetricContext,
    curve_id: &CurveId,
) -> finstack_quant_core::Result<f64> {
    let workout = crate::bond::metrics::context_workout_path(context)?;
    let bond: &Bond = context.instrument_as()?;
    let ytm = context.computed_metric(&MetricId::Ytm)?;
    let curve = context.curves.get_discount(curve_id)?;
    let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;
    let (yield_rate, horizon) = match workout {
        Some(path) => (
            path.yield_rate,
            i_spread_horizon(bond, path.flows.last().map(|(date, _)| *date)),
        ),
        None => (ytm, i_spread_horizon(bond, None)),
    };
    Ok(yield_rate - curve_par_rate(bond, curve.as_ref(), quote_ctx.quote_date, horizon)?)
}

/// I-Spread: bond yield minus the interpolated swap par rate at the same horizon.
///
/// ```text
/// I-Spread = yield − par_swap_rate(quote_date → horizon)
/// ```
///
/// The swap curve is the bond's discount curve; the yield and horizon rule are
/// [`yield_over_curve_par_rate`]'s. The quote engine's inverse uses the same
/// par rate.
///
/// # Dependencies
///
/// Requires `Ytm` metric to be computed first.
#[derive(Debug, Clone, Default)]
pub(crate) struct ISpreadCalculator;

impl MetricCalculator for ISpreadCalculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Ytm]
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let curve_id = context.instrument_as::<Bond>()?.discount_curve_id.clone();
        yield_over_curve_par_rate(context, &curve_id)
    }
}
