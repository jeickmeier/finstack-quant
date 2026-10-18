//! Commodity swap metrics module.
//!
//! Provides metric calculators specific to `CommoditySwap`, split into focused files.
//! The calculators compose with the shared metrics framework and are registered
//! via `register_commodity_swap_metrics`.
//!
//! Exposed metrics:
//! - commodity delta (sensitivity to floating index)
//! - theta (time decay)
//! - DV01 (interest rate sensitivity)

mod delta;

use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register all CommoditySwap metrics with the registry.
pub(crate) fn register_commodity_swap_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CommoditySwap,
        metrics: [
            (Delta, delta::DeltaCalculator),
            (Dv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}
