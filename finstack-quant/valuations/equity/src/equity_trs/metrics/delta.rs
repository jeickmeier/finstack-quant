//! Equity delta calculator for equity TRS.

use crate::equity_trs::EquityTotalReturnSwap;
use finstack_quant_core::{Error, Result};
use finstack_quant_valuations_core::instruments::common_impl::traits::Instrument;
use finstack_quant_valuations_core::metrics::sensitivities::config as sens_config;
use finstack_quant_valuations_core::metrics::{bump_scalar_price, MetricCalculator, MetricContext};

/// Calculates delta to the underlying equity index level.
///
/// Delta measures the sensitivity of the actual lifecycle-aware TRS PV to the
/// underlying equity level. Contractual reset fixings are held constant while
/// the live spot is bumped.
pub(crate) struct EquityDeltaCalculator;

impl MetricCalculator for EquityDeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let spot_bump = sens_config::resolve(context)?.spot_bump_decimal;
        let trs: &EquityTotalReturnSwap = context.instrument_as()?;

        let scalar = context.curves.get_price(&trs.underlying.spot_id)?;
        let spot = finstack_quant_valuations_core::metrics::scalar_numeric_value(scalar);
        if !spot.is_finite() || spot.abs() < 1e-10 {
            return Err(Error::Validation(
                "Spot price too small for delta calculation".into(),
            ));
        }

        // If the trade is fresh and no initial fixing was supplied, freeze the
        // contractual level before bumping the live market spot. Otherwise a
        // naive reprice would move both the driver and the contract anchor.
        let mut frozen = trs.clone();
        if frozen.initial_level.is_none() {
            frozen.initial_level = Some(spot);
        }
        let up = bump_scalar_price(context.curves.as_ref(), &trs.underlying.spot_id, spot_bump)?;
        let down = bump_scalar_price(context.curves.as_ref(), &trs.underlying.spot_id, -spot_bump)?;
        let pv_up = frozen.value(&up, context.as_of)?.amount();
        let pv_down = frozen.value(&down, context.as_of)?.amount();
        Ok((pv_up - pv_down) / (2.0 * spot * spot_bump))
    }
}
