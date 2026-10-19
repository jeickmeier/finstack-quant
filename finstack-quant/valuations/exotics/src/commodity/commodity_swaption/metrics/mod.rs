//! Commodity swaption metrics module.
//!
//! Registers option greeks and rate sensitivities for commodity swaptions.
//! Uses the generic option metric calculators from the shared metrics framework.

use finstack_quant_valuations_core::metrics::MetricRegistry;
use finstack_quant_valuations_core::pricer::InstrumentType;

/// Register commodity swaption metrics with the registry.
pub(crate) fn register_commodity_swaption_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CommoditySwaption,
        metrics: [
            (Delta, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::CommoditySwaption>::delta()),
            (Gamma, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::CommoditySwaption>::gamma()),
            (Vega, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::CommoditySwaption>::vega()),
            (Dv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }

    Ok(())
}
