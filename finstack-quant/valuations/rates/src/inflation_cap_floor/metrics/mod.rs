//! Inflation cap/floor metrics module.
//!
//! Provides metric calculators specific to `InflationCapFloor`.
//! Metrics are registered via `register_inflation_cap_floor_metrics`.
//!
//! # Available Metrics
//!
//! - **Inflation01**: Sensitivity to 1bp parallel shift in inflation curve
//! - **Gamma**: Second derivative (convexity) with respect to inflation rate
//! - **Vega**: Sensitivity to 1% absolute shift in volatility surface
//! - **Dv01**: Sensitivity to 1bp shift in discount curve
//! - **BucketedDv01**: Key-rate sensitivities to discount curve
//! - **Theta**: Time decay (1-day roll)

mod gamma;
mod inflation01;

use crate::inflation_cap_floor::InflationCapFloor;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::bump_surface_vol_absolute;
use finstack_quant_valuations_core::metrics::sensitivities::config as sens_config;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricRegistry;
use finstack_quant_valuations_core::metrics::VOL_POINTS_PER_ABSOLUTE_VOL;

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
            (Inflation01, inflation01::Inflation01Calculator),
            (Gamma, gamma::GammaCalculator),
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
        let vol_bump = sens_config::resolve(context)?.vol_bump_decimal;
        let option: &InflationCapFloor = context.instrument_as()?;
        let as_of = context.as_of;

        // Bump vol surface up
        let curves_up =
            bump_surface_vol_absolute(&context.curves, option.vol_surface_id.as_str(), vol_bump)?;
        let pv_up = context.reprice_raw(&curves_up, as_of)?;

        // Bump vol surface down
        let curves_down =
            bump_surface_vol_absolute(&context.curves, option.vol_surface_id.as_str(), -vol_bump)?;
        let pv_down = context.reprice_raw(&curves_down, as_of)?;

        // Central difference per unit vol, rescaled to per vol point (1% = 0.01
        // absolute vol) to match the workspace vega convention.
        Ok((pv_up - pv_down) / (2.0 * vol_bump * VOL_POINTS_PER_ABSOLUTE_VOL))
    }
}
