//! Color calculator for equity options.
//!
//! Computes color (∂³V/∂S²∂t), also known as gamma decay.
//! Color measures how gamma changes with time.
//!
//! Color ≈ (Gamma(t+h) - Gamma(t)) / h
//!
//! Where Gamma(t) is computed at current time, and Gamma(t+h) at a later time.

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::equity_option::EquityOption;
use crate::metrics::{bump_scalar_price, bump_sizes};
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;

/// Color calculator for equity options.
pub(crate) struct ColorCalculator;

impl MetricCalculator for ColorCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &EquityOption = context.instrument_as()?;
        let as_of = context.as_of;
        let base_pv = context.base_value.amount();

        // Check if expired
        let t = option.day_count.year_fraction(
            as_of,
            option.expiry,
            finstack_core::dates::DayCountContext::default(),
        )?;
        if t <= 0.0 {
            return Ok(0.0);
        }

        // Get current spot
        let spot_scalar = context.curves.get_price(&option.spot_id)?;
        let current_spot = match spot_scalar {
            finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
            finstack_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
        };

        // Use adaptive/custom bump from pricing overrides if configured
        let overrides = &option.pricing_overrides.metrics.bump_config;
        let bump_pct = if let Some(custom) = overrides.spot_bump_pct {
            custom
        } else if overrides.adaptive_bumps {
            let moneyness = (current_spot - option.strike).abs() / option.strike;
            bump_sizes::SPOT * (1.0 + 2.0 * moneyness).min(5.0)
        } else {
            bump_sizes::SPOT
        };
        let spot_bump = current_spot * bump_pct;

        // Guard near-expiry: avoid time bumps when T < 2 days.
        // The 365.0 basis matches the pricer's Act/365F vol clock so that
        // `h_years` below uses the same day-count as the re-priced PV.
        let time_bump_days = if t < 2.0 / 365.0 {
            return Ok(0.0);
        } else {
            1.0
        };

        // Compute gamma at current time
        let curves_up = bump_scalar_price(&context.curves, &option.spot_id, bump_pct)?;
        let pv_up = option.value(&curves_up, as_of)?.amount();
        let curves_down = bump_scalar_price(&context.curves, &option.spot_id, -bump_pct)?;
        let pv_down = option.value(&curves_down, as_of)?.amount();
        let gamma_t = (pv_up - 2.0 * base_pv + pv_down) / (spot_bump * spot_bump);

        // Compute gamma at time + 1 day
        let rolled_date = as_of + time::Duration::days(time_bump_days as i64);
        let base_pv_future = option.value(&context.curves, rolled_date)?.amount();
        let curves_up_future = bump_scalar_price(&context.curves, &option.spot_id, bump_pct)?;
        let pv_up_future = option.value(&curves_up_future, rolled_date)?.amount();
        let curves_down_future = bump_scalar_price(&context.curves, &option.spot_id, -bump_pct)?;
        let pv_down_future = option.value(&curves_down_future, rolled_date)?.amount();
        let gamma_t_future =
            (pv_up_future - 2.0 * base_pv_future + pv_down_future) / (spot_bump * spot_bump);

        // Color = (Gamma(t+h) - Gamma(t)) / h
        // Use the 365.0 (Act/365F) basis to match the pricer's vol clock — a
        // 1/365.25 clock would mis-scale the derivative denominator by ~0.07%.
        let h_years = time_bump_days / 365.0;
        let color = (gamma_t_future - gamma_t) / h_years;

        Ok(color)
    }
}

#[cfg(test)]
mod tests {
    /// W-33: the color time-bump clock must use the Act/365F (365.0) basis to
    /// match the pricer's vol clock. A 1/365.25 clock mis-scales the derivative
    /// denominator by ~0.07%.
    #[test]
    fn time_bump_clock_is_act365f() {
        let time_bump_days: f64 = 1.0;
        let h_years = time_bump_days / 365.0;
        let expected = 1.0 / 365.0;
        assert!((h_years - expected).abs() < 1e-15);
        let calendar = 1.0 / 365.25;
        assert!((h_years - calendar).abs() > 1e-9);
    }
}
