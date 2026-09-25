//! Speed calculator for equity options.
//!
//! Computes speed (∂³V/∂S³), which measures how gamma changes with spot.
//!
//! Speed ≈ (Gamma(S+h) - Gamma(S-h)) / (2h)
//!
//! Where Gamma(S) is computed at current spot, and Gamma(S±h) at bumped spots.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::equity_option::EquityOption;
use crate::metrics::bump_scalar_price;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Speed calculator for equity options.
pub(crate) struct SpeedCalculator;

impl MetricCalculator for SpeedCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &EquityOption = context.instrument_as()?;
        let as_of = context.as_of;
        let base_pv = context.base_value.amount();

        let t = option.day_count.year_fraction(
            as_of,
            option.expiry,
            finstack_quant_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(0.0);
        }

        let current_spot = crate::instruments::common_impl::helpers::scalar_price_amount(
            context.curves.get_price(&option.spot_id)?,
            option.notional.currency(),
        )?;

        let bump_pct = super::spot_bump_decimal(context, option, current_spot)?;
        let spot_bump = current_spot * bump_pct;

        let curves_up_up = bump_scalar_price(&context.curves, &option.spot_id, 2.0 * bump_pct)?;
        let pv_up_up = option.value(&curves_up_up, as_of)?.amount();
        let curves_up = bump_scalar_price(&context.curves, &option.spot_id, bump_pct)?;
        let pv_up = option.value(&curves_up, as_of)?.amount();
        let gamma_up = (pv_up_up - 2.0 * pv_up + base_pv) / (spot_bump * spot_bump);

        let curves_down = bump_scalar_price(&context.curves, &option.spot_id, -bump_pct)?;
        let pv_down = option.value(&curves_down, as_of)?.amount();
        let curves_down_down =
            bump_scalar_price(&context.curves, &option.spot_id, -2.0 * bump_pct)?;
        let pv_down_down = option.value(&curves_down_down, as_of)?.amount();
        let gamma_down = (base_pv - 2.0 * pv_down + pv_down_down) / (spot_bump * spot_bump);

        // Speed = (Gamma(S+h) - Gamma(S-h)) / (2h)
        let speed = (gamma_up - gamma_down) / (2.0 * spot_bump);

        Ok(speed)
    }
}
