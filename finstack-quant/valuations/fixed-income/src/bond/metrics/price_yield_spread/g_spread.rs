//! G-spread: the bond's yield over the par yield of a named government curve.

use super::i_spread::yield_over_curve_par_rate;
use crate::bond::Bond;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricId};

/// G-spread: bond yield minus the par yield of a named government discount curve.
///
/// ```text
/// G-Spread = yield − par_yield(government curve, quote_date → horizon)
/// ```
///
/// This convention is a working choice, not a settled desk standard: it
/// interpolates the named curve at the horizon rather than selecting the
/// nearest on-the-run bond, and it shares its yield and horizon with I-spread
/// (worst exit on a bond with exercise rights and a quoted price, maturity
/// otherwise). The curve is
/// `instrument_pricing_overrides.model_config.government_curve_id`, the home of
/// a curve one spread metric reads, as `asw_forward_curve_id` is.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] when the bond names no
/// government curve; the metric never falls back to the discount curve. A named
/// curve missing from the market is the market's usual not-found error.
///
/// # Dependencies
///
/// Requires `Ytm` metric to be computed first.
#[derive(Debug, Clone, Default)]
pub(crate) struct GSpreadCalculator;

impl MetricCalculator for GSpreadCalculator {
    fn dependencies(&self) -> &[MetricId] {
        &[MetricId::Ytm]
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;
        let curve_id = bond
            .instrument_pricing_overrides
            .model_config
            .government_curve_id
            .clone()
            .ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "g_spread for bond '{}' requires a government curve: set \
                     instrument_pricing_overrides.model_config.government_curve_id",
                    bond.id.as_str()
                ))
            })?;
        yield_over_curve_par_rate(context, &curve_id)
    }
}
