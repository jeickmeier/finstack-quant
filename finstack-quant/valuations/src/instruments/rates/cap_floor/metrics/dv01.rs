//! Market DV01 calculator for caps/floors.
//!
//! When the discount and forward curves carry rate-calibration metadata, this
//! reports quote-shock/rebootstrap DV01 via the shared
//! [`bump_market_via_rate_quote_shock`] helper. Otherwise it falls back to the
//! generic fitted-curve bump path provided by [`UnifiedDv01Calculator`].

use crate::calibration::bumps::rates::bump_market_via_rate_quote_shock;
use crate::instruments::rates::cap_floor::CapFloor;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::sensitivities::cs01::sensitivity_central_diff;
use crate::metrics::{
    Dv01CalculatorConfig, MetricCalculator, MetricContext, UnifiedDv01Calculator,
};
use finstack_quant_core::Result;

/// Cap/floor DV01.
pub(crate) struct Dv01Calculator;

impl MetricCalculator for Dv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let cap_floor: &CapFloor = context.instrument_as()?;
        let market = context.curves.as_ref();
        let discount = market.get_discount(cap_floor.discount_curve_id.as_str())?;
        let forward = market.get_forward(cap_floor.forward_curve_id.as_str())?;

        let discount_has_replay =
            discount.rate_calibration().is_some() || discount.rate_calibration_recipe().is_some();
        let forward_has_replay =
            forward.rate_calibration().is_some() || forward.rate_calibration_recipe().is_some();
        if !discount_has_replay || !forward_has_replay {
            return UnifiedDv01Calculator::<CapFloor>::new(
                Dv01CalculatorConfig::parallel_combined(),
            )
            .calculate(context);
        }

        let bump_bp =
            sens_config::from_context_or_default(context.config(), context.get_metric_overrides())?
                .rate_bump_bp;

        let discount_id = &cap_floor.discount_curve_id;
        let forward_id = &cap_floor.forward_curve_id;

        let bumped_up = bump_market_via_rate_quote_shock(market, discount_id, forward_id, bump_bp)?;
        let pv_up = context.reprice_raw(&bumped_up, context.as_of)?;

        let bumped_down =
            bump_market_via_rate_quote_shock(market, discount_id, forward_id, -bump_bp)?;
        let pv_down = context.reprice_raw(&bumped_down, context.as_of)?;

        Ok(sensitivity_central_diff(pv_up, pv_down, bump_bp))
    }
}
