//! Barrier option metrics module.
//!
//! Provides full greek coverage for barrier options using finite difference methods.
//! Delta and Gamma use generic FD calculators.
//! Note: Barrier options exhibit discontinuous greeks near the barrier level.

use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register barrier option metrics with the registry.
pub(crate) fn register_barrier_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::sensitivities::fd_greeks::{
        GenericFdDelta, GenericFdGamma, GenericFdVanna, GenericFdVolga,
    };
    use finstack_quant_valuations_core::pricer::InstrumentType;

    {
        finstack_quant_valuations_core::register_metrics! {
            registry: registry,
            instrument: InstrumentType::BarrierOption,
            metrics: [
                (Vega, finstack_quant_valuations_core::metrics::sensitivities::fd_greeks::GenericFdVega::<crate::BarrierOption>::default()),
                (Rho, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
                (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
                (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
                // Theta is now registered universally in crate::exotics::EXOTICS.metrics(),
                (Delta, GenericFdDelta::<crate::BarrierOption>::default()),
                (Gamma, GenericFdGamma::<crate::BarrierOption>::default()),
                (Vanna, GenericFdVanna::<crate::BarrierOption>::default()),
                (Volga, GenericFdVolga::<crate::BarrierOption>::default()),
            ]
        }
    }
    Ok(())
}
