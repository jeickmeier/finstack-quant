//! Range accrual metrics module.
//!
//! Provides full greek coverage for range accrual instruments using
//! finite difference methods. Delta and Gamma use generic FD calculators.
//! Includes bucketed DV01 for detailed interest rate risk analysis.

use finstack_quant_valuations_core::metrics::{MetricId, MetricRegistry};
use std::sync::Arc;

/// Register range accrual metrics with the registry.
pub(crate) fn register_range_accrual_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::{
        GenericFdDelta, GenericFdGamma, GenericFdVanna, GenericFdVolga,
    };
    use finstack_quant_valuations_core::pricer::InstrumentType;

    registry.register_metric(
        MetricId::Delta,
        Arc::new(GenericFdDelta::<crate::RangeAccrual>::default()),
        &[InstrumentType::RangeAccrual],
    )?;

    registry.register_metric(
        MetricId::Gamma,
        Arc::new(GenericFdGamma::<crate::RangeAccrual>::default()),
        &[InstrumentType::RangeAccrual],
    )?;

    registry.register_metric(
        MetricId::Vanna,
        Arc::new(GenericFdVanna::<crate::RangeAccrual>::default()),
        &[InstrumentType::RangeAccrual],
    )?;

    registry.register_metric(
        MetricId::Volga,
        Arc::new(GenericFdVolga::<crate::RangeAccrual>::default()),
        &[InstrumentType::RangeAccrual],
    )?;

    {
        finstack_quant_valuations_core::register_metrics! {
            registry: registry,
            instrument: InstrumentType::RangeAccrual,
            metrics: [
                (Vega, finstack_quant_valuations_core::metrics::GenericFdVega::<crate::RangeAccrual>::default()),
                // Rho: discount-curve-only central bump, repriced with the requested model.
                (Rho, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_discount_only())),
                (Dv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_combined())),
                // Theta is now registered universally in crate::exotics::EXOTICS.metrics()
                (BucketedDv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate())),
            ]
        }
    }
    Ok(())
}
