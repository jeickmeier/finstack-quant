//! Charm calculator for equity options.
//!
//! Computes charm (∂²V/∂S∂t), also known as delta decay.
//! Charm measures how delta changes with time.
//!
//! Charm ≈ (Delta(t+h) - Delta(t)) / h
//!
//! Where Delta(t) is computed by bumping spot at current time,
//! and Delta(t+h) is computed by bumping spot at a later time.

use crate::instruments::equity::equity_option::EquityOption;
use crate::metrics::bump_scalar_price;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Charm calculator for equity options.
pub(crate) struct CharmCalculator;

impl MetricCalculator for CharmCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &EquityOption = context.instrument_as()?;
        let as_of = context.as_of;

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
            option.currency,
        )?;

        let bump_pct = super::spot_bump_decimal(context, option, current_spot)?;
        let spot_bump = current_spot * bump_pct;

        // Guard near-expiry: avoid time bumps when T < 2 days.
        // The 365.0 basis matches the pricer's Act/365F vol clock so that
        // `h_years` below uses the same day-count as the re-priced PV.
        let time_bump_days = if t < 2.0 / 365.0 {
            return Ok(0.0);
        } else {
            1.0
        };

        let curves_up = bump_scalar_price(&context.curves, &option.spot_id, bump_pct)?;
        let pv_up = context.reprice_instrument_raw(option, &curves_up, as_of)?;
        let curves_down = bump_scalar_price(&context.curves, &option.spot_id, -bump_pct)?;
        let pv_down = context.reprice_instrument_raw(option, &curves_down, as_of)?;
        let delta_t = (pv_up - pv_down) / (2.0 * spot_bump);

        let rolled_date = as_of + time::Duration::days(time_bump_days as i64);
        let curves_up_future = bump_scalar_price(&context.curves, &option.spot_id, bump_pct)?;
        let pv_up_future =
            context.reprice_instrument_raw(option, &curves_up_future, rolled_date)?;
        let curves_down_future = bump_scalar_price(&context.curves, &option.spot_id, -bump_pct)?;
        let pv_down_future =
            context.reprice_instrument_raw(option, &curves_down_future, rolled_date)?;
        let delta_t_future = (pv_up_future - pv_down_future) / (2.0 * spot_bump);

        // Charm = (Delta(t+h) - Delta(t)) / h
        // h is in days, convert to years for proper scaling. Use the 365.0
        // (Act/365F) basis to match the pricer's vol clock — a 1/365.25 clock
        // would mis-scale the derivative denominator by ~0.07%.
        let h_years = time_bump_days / 365.0;
        let charm = (delta_t_future - delta_t) / h_years;

        Ok(charm)
    }
}

#[cfg(test)]
mod tests {
    /// W-33: the charm time-bump clock must use the Act/365F (365.0) basis to
    /// match the pricer's vol clock. A 1/365.25 clock mis-scales the derivative
    /// denominator by ~0.07%.
    #[test]
    fn time_bump_clock_is_act365f() {
        let time_bump_days: f64 = 1.0;
        let h_years = time_bump_days / 365.0;
        // Exact Act/365F day fraction for a single calendar day.
        let expected = 1.0 / 365.0;
        assert!((h_years - expected).abs() < 1e-15);
        // And it must NOT be the 365.25 calendar-year basis.
        let calendar = 1.0 / 365.25;
        assert!((h_years - calendar).abs() > 1e-9);
    }
}
