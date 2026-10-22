//! Bond price, yield, spread, duration, and risk metric calculations.
//!
use crate::bond::pricing::quote_conversions::i_spread_par_rate;
use crate::bond::pricing::settlement::QuoteDateContext;
use crate::bond::Bond;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricId};

/// I-Spread: bond yield minus the interpolated swap par rate at the same horizon.
///
/// ```text
/// I-Spread = yield − par_swap_rate(quote_date → horizon)
/// ```
///
/// Both rates compound at the bond's coupon frequency.
///
/// For a callable or puttable bond with a quoted price the yield is the
/// yield-to-worst and the horizon is the workout date; otherwise the yield is
/// the Street YTM and the horizon is maturity. The par rate comes from
/// [`i_spread_par_rate`], which the quote engine's inverse also uses.
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
        let workout = crate::bond::metrics::context_workout_path(context)?;
        let bond: &Bond = context.instrument_as()?;

        let ytm = context.computed_metric(&MetricId::Ytm)?;

        let disc = context.curves.get_discount(&bond.discount_curve_id)?;
        let quote_ctx = QuoteDateContext::new(bond, &context.curves, context.as_of)?;
        let (yield_rate, horizon) = match workout {
            Some(path) => (
                path.yield_rate,
                path.flows.last().map_or(bond.maturity, |(date, _)| *date),
            ),
            None => (ytm, bond.maturity),
        };
        Ok(yield_rate - i_spread_par_rate(bond, disc.as_ref(), quote_ctx.quote_date, horizon)?)
    }
}
