//! Metric registration and calculators.

use crate::inflation_cap_floor::InflationCapFloor;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricRegistry;
use finstack_quant_valuations_core::metrics::VOL_POINTS_PER_ABSOLUTE_VOL;
use finstack_quant_valuations_core::metrics::bump_surface_vol_absolute;
use finstack_quant_valuations_core::metrics::sensitivities::bump_config;

/// Register all inflation cap/floor metrics with the registry.
pub(crate) fn register_inflation_cap_floor_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::InflationCapFloor,
        metrics: [
            (Vega, VegaCalculator),
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
            (Inflation01, crate::inflation_fd::Inflation01Calculator::<InflationCapFloor>::new()),
            (Gamma, crate::inflation_fd::InflationGammaCalculator::<InflationCapFloor>::new()),
        ]
    };
    Ok(())
}

/// Vega calculator for inflation cap/floor options.
///
/// Uses central differences for improved accuracy on curved vol surfaces.
///
/// Computes vega using central finite differences for O(h²) accuracy:
///
/// ```text
/// Vega = (PV_vol_up - PV_vol_down) / (2 × bump_size × VOL_POINTS_PER_ABSOLUTE_VOL)
/// ```
///
/// This avoids the O(h) bias that one-sided differences can introduce,
/// especially important for curved volatility surfaces.
///
/// # Output Convention
///
/// **Vega is expressed per 1 volatility point (0.01 absolute vol change)**,
/// matching the workspace-wide convention used by swaption
/// (`VOL_PCT_SCALE`), nominal cap/floor, CMS, and `fd_greeks` vegas. The raw
/// central difference yields sensitivity per *unit* absolute vol; dividing by
/// `VOL_POINTS_PER_ABSOLUTE_VOL` (= 100) converts it to per-vol-point.
pub(crate) struct VegaCalculator;

impl MetricCalculator for VegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let vol_bump = bump_config::resolve(context)?.vol_bump_decimal;
        let option: &InflationCapFloor = context.instrument_as()?;
        let as_of = context.as_of;
        let denom_points = vol_bump * VOL_POINTS_PER_ABSOLUTE_VOL;

        if let Some(sigma) = option
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility
        {
            let mut bumped = option.clone();
            bumped
                .instrument_pricing_overrides
                .market_quotes
                .implied_volatility = Some(sigma + vol_bump);
            let pv_up = context.reprice_instrument_raw(&bumped, &context.curves, as_of)?;
            if sigma > vol_bump {
                bumped
                    .instrument_pricing_overrides
                    .market_quotes
                    .implied_volatility = Some(sigma - vol_bump);
                let pv_down = context.reprice_instrument_raw(&bumped, &context.curves, as_of)?;
                return Ok((pv_up - pv_down) / (2.0 * denom_points));
            }
            return Ok((pv_up - context.base_value.amount()) / denom_points);
        }

        // Bump vol surface up
        let curves_up =
            bump_surface_vol_absolute(&context.curves, option.vol_surface_id.as_str(), vol_bump)?;
        let pv_up = context.reprice_raw(&curves_up, as_of)?;

        // Prefer central difference; fall back to forward difference if bumping
        // down pushes a low-volatility surface below zero or is otherwise invalid.
        if let Ok(curves_down) =
            bump_surface_vol_absolute(&context.curves, option.vol_surface_id.as_str(), -vol_bump)
            && let Ok(pv_down) = context.reprice_raw(&curves_down, as_of)
        {
            return Ok((pv_up - pv_down) / (2.0 * denom_points));
        }
        Ok((pv_up - context.base_value.amount()) / denom_points)
    }
}
