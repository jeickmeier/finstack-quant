//! Quanto option metrics module.
//!
//! Provides full greek coverage for quanto options including FX-specific
//! sensitivities (FX delta, FX vega) and correlation risk.

mod correlation01;
mod fx_vega;

use crate::quanto_option::QuantoOption;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::bump_scalar_price;
use finstack_quant_valuations_core::metrics::sensitivities::config as sens_config;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::{MetricId, MetricRegistry};
use std::sync::Arc;

/// Register quanto option metrics with the registry.
pub(crate) fn register_quanto_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;

    // Theta is one of the universal metrics every family registry carries.
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::QuantoOption,
        metrics: [
            (Delta, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::quanto_option::QuantoOption>::delta()),
            (Gamma, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::quanto_option::QuantoOption>::gamma()),
            (Vega, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::quanto_option::QuantoOption>::vega()),
            (Rho, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::quanto_option::QuantoOption>::rho()),
            (ForeignRho, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::quanto_option::QuantoOption>::foreign_rho()),
            (Dv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate())),
            (Vanna, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::quanto_option::QuantoOption>::vanna()),
            (Volga, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::quanto_option::QuantoOption>::volga()),
        ]
    }

    // FX-specific and correlation metrics (custom MetricIds).
    for (id, calculator) in [
        (
            MetricId::FxDelta,
            Arc::new(FxDeltaCalculator)
                as Arc<dyn finstack_quant_valuations_core::metrics::MetricCalculator>,
        ),
        (MetricId::FxVega, Arc::new(fx_vega::FxVegaCalculator)),
        (
            MetricId::Correlation01,
            Arc::new(correlation01::Correlation01Calculator),
        ),
    ] {
        registry.register_metric(id, calculator, &[InstrumentType::QuantoOption])?;
    }
    Ok(())
}

/// FX Delta calculator for quanto options.
///
/// Computes FX delta (FX rate sensitivity) using a central finite difference
/// against a relative SPOT bump. Quanto options have payoffs dependent on an
/// equity in one currency but settled in another; FX delta measures sensitivity
/// to changes in the FX exchange rate between those currencies.
pub struct FxDeltaCalculator;

impl MetricCalculator for FxDeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let spot_bump = sens_config::resolve(context)?.spot_bump_decimal;
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

        let fx_spot_id = option.fx_spot_id.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "QuantoOption {}: fx_spot_id is required to compute FX Delta",
                option.id
            ))
        })?;

        let market_up = bump_scalar_price(context.curves.as_ref(), fx_spot_id, spot_bump)?;
        let market_down = bump_scalar_price(context.curves.as_ref(), fx_spot_id, -spot_bump)?;
        let pv_up = option.value(&market_up, as_of)?.amount();
        let pv_down = option.value(&market_down, as_of)?.amount();

        // FxDelta is the cash P&L for a 1% relative FX move, not a derivative
        // per one unit of spot: normalise the ±`spot_bump` central difference
        // by the bump width expressed in 1% units (2.0 at the 1% default).
        Ok((pv_up - pv_down) / (2.0 * spot_bump * 100.0))
    }
}
