//! Quanto option metrics module.
//!
//! Provides full greek coverage for quanto options including FX-specific
//! sensitivities (FX delta, FX vega) and correlation risk.

mod correlation01;
mod fx_delta;
mod fx_vega;

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
            Arc::new(fx_delta::FxDeltaCalculator)
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
