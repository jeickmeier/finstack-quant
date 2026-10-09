//! FRA quote-shock DV01.
//!
//! When the discount and forward curves carry rate-calibration metadata, this
//! reports quote-shock/rebootstrap DV01 through the shared
//! `bump_market_via_rate_quote_shock` helper. When metadata is unavailable, or
//! the forward curve was calibrated from tenor-basis quotes (which the shared
//! replay does not support), it falls back to the generic fitted-curve bump
//! path.

use crate::instruments::rates::fra::ForwardRateAgreement;
use crate::metrics::sensitivities::config as sens_config;
use crate::metrics::sensitivities::cs01::sensitivity_central_diff;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::market_data::term_structures::{
    RateCalibrationQuote, RateCalibrationRecipe,
};
use finstack_quant_core::Result;

/// FRA DV01 calculator. Prefers quote-shock/rebootstrap when replayable
/// calibration metadata is available, falling back to the generic fitted-curve
/// bump.
pub(crate) struct FraRateCurveDv01Calculator;

impl MetricCalculator for FraRateCurveDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let fra: &ForwardRateAgreement = context.instrument_as()?;
        let market = context.curves.as_ref();
        let discount = market.get_discount(fra.discount_curve_id.as_str())?;
        let forward = market.get_forward(fra.forward_curve_id.as_str())?;

        let replayable = discount.rate_calibration().is_some()
            && forward
                .rate_calibration()
                .is_some_and(|recipe| !uses_basis_quotes(recipe));
        if !replayable {
            return generic_fallback(context);
        }

        let bump_bp = sens_config::from_context_or_default(
            context.get_config(),
            context.get_metric_pricing_overrides(),
        )?
        .rate_bump_bp;

        let discount_id = &fra.discount_curve_id;
        let forward_id = &fra.forward_curve_id;

        let bumped_up = context.bump_rate_market_cached(discount_id, forward_id, bump_bp)?;
        let pv_up = context.reprice_raw(bumped_up.as_ref(), context.as_of)?;
        let bumped_down = context.bump_rate_market_cached(discount_id, forward_id, -bump_bp)?;
        let pv_down = context.reprice_raw(bumped_down.as_ref(), context.as_of)?;
        Ok(sensitivity_central_diff(pv_up, pv_down, bump_bp))
    }
}

fn generic_fallback(context: &mut MetricContext) -> Result<f64> {
    crate::metrics::UnifiedDv01Calculator::new(
        crate::metrics::Dv01CalculatorConfig::parallel_combined(),
    )
    .calculate(context)
}

fn uses_basis_quotes(calibration: &RateCalibrationRecipe) -> bool {
    calibration
        .quotes
        .iter()
        .any(|quote| matches!(quote, RateCalibrationQuote::Basis { .. }))
}
