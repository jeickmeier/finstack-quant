//! Range accrual metrics module.
//!
//! Provides full greek coverage for range accrual instruments using
//! finite difference methods. Delta and Gamma use generic FD calculators.
//! Includes bucketed DV01 for detailed interest rate risk analysis.

use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register range accrual metrics with the registry.
pub(crate) fn register_range_accrual_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::sensitivities::fd_greeks::{
        GenericFdDelta, GenericFdGamma, GenericFdVanna, GenericFdVolga,
    };
    use finstack_quant_valuations_core::pricer::InstrumentType;

    {
        finstack_quant_valuations_core::register_metrics! {
            registry: registry,
            instrument: InstrumentType::RangeAccrual,
            metrics: [
                (Vega, finstack_quant_valuations_core::metrics::sensitivities::fd_greeks::GenericFdVega::<crate::exotics::range_accrual::RangeAccrual>::default()),
                // Rho: discount-curve-only central bump, repriced with the requested model.
                (Rho, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_discount_only())),
                (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
                // Theta is now registered universally in crate::EXOTICS.metrics()
                (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
                (Delta, GenericFdDelta::<crate::exotics::range_accrual::RangeAccrual>::default()),
                (Gamma, GenericFdGamma::<crate::exotics::range_accrual::RangeAccrual>::default()),
                (Vanna, GenericFdVanna::<crate::exotics::range_accrual::RangeAccrual>::default()),
                (Volga, GenericFdVolga::<crate::exotics::range_accrual::RangeAccrual>::default()),
            ]
        }
    }
    Ok(())
}
