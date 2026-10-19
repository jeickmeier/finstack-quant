//! Autocallable metrics module.
//!
//! Provides full greek coverage for autocallable structured products using
//! finite difference methods. Delta and Gamma use generic FD calculators.
//! Note: Autocallables exhibit discontinuities near autocall barrier levels.

use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register autocallable metrics with the registry.
pub(crate) fn register_autocallable_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::{
        GenericFdDelta, GenericFdGamma, GenericFdVanna, GenericFdVolga,
    };
    use finstack_quant_valuations_core::pricer::InstrumentType;

    {
        finstack_quant_valuations_core::register_metrics! {
            registry: registry,
            instrument: InstrumentType::Autocallable,
            metrics: [
                (Vega, finstack_quant_valuations_core::metrics::GenericFdVega::<crate::Autocallable>::default()),
                (Rho, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_combined())),
                (Dv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_combined())),
                (BucketedDv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate())),
                // Theta is now registered universally in crate::EQUITY.metrics(),
                (Delta, GenericFdDelta::<crate::Autocallable>::default()),
                (Gamma, GenericFdGamma::<crate::Autocallable>::default()),
                (Vanna, GenericFdVanna::<crate::Autocallable>::default()),
                (Volga, GenericFdVolga::<crate::Autocallable>::default()),
            ]
        }
    }
    Ok(())
}
