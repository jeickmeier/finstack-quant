//! FX Delta calculator for quanto options.
//!
//! Computes FX delta (FX rate sensitivity) using a central finite difference
//! against a relative SPOT bump. Quanto options have payoffs dependent on an
//! equity in one currency but settled in another; FX delta measures sensitivity
//! to changes in the FX exchange rate between those currencies.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fx::quanto_option::QuantoOption;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::{bump_scalar_price, MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// FX Delta calculator for quanto options.
pub struct FxDeltaCalculator;

impl MetricCalculator for FxDeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let spot_bump = sens_config::resolve(context)?.spot_bump_pct;
        let option: &QuantoOption = context.instrument_as()?;
        let as_of = context.as_of;

        let t = option.day_count.year_fraction(
            as_of,
            option.expiry,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(0.0);
        }

        let fx_rate_id = option.fx_rate_id.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "QuantoOption {}: fx_rate_id is required to compute FX Delta",
                option.id
            ))
        })?;

        let market_up = bump_scalar_price(context.curves.as_ref(), fx_rate_id, spot_bump)?;
        let market_down = bump_scalar_price(context.curves.as_ref(), fx_rate_id, -spot_bump)?;
        let pv_up = option.value(&market_up, as_of)?.amount();
        let pv_down = option.value(&market_down, as_of)?.amount();

        // FxDelta is the cash P&L for a 1% relative FX move, not a derivative
        // per one unit of spot: normalise the ±`spot_bump` central difference
        // by the bump width expressed in 1% units (2.0 at the 1% default).
        Ok((pv_up - pv_down) / (2.0 * spot_bump * 100.0))
    }
}
