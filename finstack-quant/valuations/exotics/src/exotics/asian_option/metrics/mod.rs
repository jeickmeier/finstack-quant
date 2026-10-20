//! Asian option metrics module.
//!
//! Provides full greek coverage for Asian options using finite difference methods.
//! Delta and Gamma use generic FD calculators.

use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register Asian option metrics with the registry.
pub(crate) fn register_asian_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::sensitivities::fd_greeks::{
        GenericFdDelta, GenericFdGamma, GenericFdVanna, GenericFdVolga,
    };
    use finstack_quant_valuations_core::pricer::InstrumentType;

    {
        finstack_quant_valuations_core::register_metrics! {
            registry: registry,
            instrument: InstrumentType::AsianOption,
            metrics: [
                (Vega, finstack_quant_valuations_core::metrics::sensitivities::fd_greeks::GenericFdVega::<crate::exotics::asian_option::AsianOption>::default()),
                (Rho, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
                (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
                (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
                // Theta is now registered universally in crate::EXOTICS.metrics(),
                (Delta, GenericFdDelta::<crate::exotics::asian_option::AsianOption>::default()),
                (Gamma, GenericFdGamma::<crate::exotics::asian_option::AsianOption>::default()),
                (Vanna, GenericFdVanna::<crate::exotics::asian_option::AsianOption>::default()),
                (Volga, GenericFdVolga::<crate::exotics::asian_option::AsianOption>::default()),
            ]
        }
    }
    Ok(())
}
