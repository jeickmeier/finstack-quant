//! Lookback option metrics module.
//!
//! Provides full greek coverage for lookback options using finite difference methods.
//! Delta and Gamma use generic FD calculators.

use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register lookback option metrics with the registry.
pub(crate) fn register_lookback_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::sensitivities::fd_greeks::{
        GenericFdDelta, GenericFdGamma, GenericFdVanna, GenericFdVolga,
    };
    use finstack_quant_valuations_core::pricer::InstrumentType;

    {
        finstack_quant_valuations_core::register_metrics! {
            registry: registry,
            instrument: InstrumentType::LookbackOption,
            metrics: [
                (Vega, finstack_quant_valuations_core::metrics::sensitivities::fd_greeks::GenericFdVega::<crate::exotics::lookback_option::LookbackOption>::default()),
                // Rho: discount-curve-only central bump, repriced with the requested model.
                (Rho, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_discount_only())),
                (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
                (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
                // Theta is now registered universally in crate::exotics::EXOTICS.metrics(),
                (Delta, GenericFdDelta::<crate::LookbackOption>::default()),
                (Gamma, GenericFdGamma::<crate::LookbackOption>::default()),
                (Vanna, GenericFdVanna::<crate::LookbackOption>::default()),
                (Volga, GenericFdVolga::<crate::LookbackOption>::default()),
            ]
        }
    }
    Ok(())
}
