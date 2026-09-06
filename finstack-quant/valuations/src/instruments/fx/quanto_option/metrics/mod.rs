//! Quanto option metrics module.
//!
//! Provides full greek coverage for quanto options including FX-specific
//! sensitivities (FX delta, FX vega) and correlation risk.

mod correlation01;
mod fx_delta;
mod fx_vega;

use crate::metrics::{MetricId, MetricRegistry};
use std::sync::Arc;

/// Register quanto option metrics with the registry.
pub(crate) fn register_quanto_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    use crate::pricer::InstrumentType;

    // Theta is registered universally in `metrics::standard_registry`.
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::QuantoOption,
        metrics: [
            (Delta, crate::metrics::OptionGreekCalculator::<crate::instruments::fx::quanto_option::QuantoOption>::delta()),
            (Gamma, crate::metrics::OptionGreekCalculator::<crate::instruments::fx::quanto_option::QuantoOption>::gamma()),
            (Vega, crate::metrics::OptionGreekCalculator::<crate::instruments::fx::quanto_option::QuantoOption>::vega()),
            (Rho, crate::metrics::OptionGreekCalculator::<crate::instruments::fx::quanto_option::QuantoOption>::rho()),
            (ForeignRho, crate::metrics::OptionGreekCalculator::<crate::instruments::fx::quanto_option::QuantoOption>::foreign_rho()),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::fx::quanto_option::QuantoOption,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::fx::quanto_option::QuantoOption,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
            (Vanna, crate::metrics::OptionGreekCalculator::<crate::instruments::fx::quanto_option::QuantoOption>::vanna()),
            (Volga, crate::metrics::OptionGreekCalculator::<crate::instruments::fx::quanto_option::QuantoOption>::volga()),
        ]
    }

    // FX-specific and correlation metrics (custom MetricIds).
    for (id, calculator) in [
        (
            MetricId::FxDelta,
            Arc::new(fx_delta::FxDeltaCalculator) as Arc<dyn crate::metrics::MetricCalculator>,
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
