//! Commodity swaption metrics module.
//!
//! Registers option greeks and rate sensitivities for commodity swaptions.
//! Uses the generic option metric calculators from the shared metrics framework.

use finstack_quant_valuations_core::metrics::MetricRegistry;
use finstack_quant_valuations_core::pricer::InstrumentType;
use std::sync::Arc;

/// Register commodity swaption metrics with the registry.
pub(crate) fn register_commodity_swaption_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::MetricId;

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CommoditySwaption,
        metrics: [
            (Delta, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::CommoditySwaption>::delta()),
            (Gamma, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::CommoditySwaption>::gamma()),
            (Vega, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::CommoditySwaption>::vega()),
        ]
    }

    registry.register_metric(
        MetricId::Dv01,
        Arc::new(
            finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(
                finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_combined(),
            ),
        ),
        &[InstrumentType::CommoditySwaption],
    )?;
    registry.register_metric(
        MetricId::BucketedDv01,
        Arc::new(
            finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(
                finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate(
                ),
            ),
        ),
        &[InstrumentType::CommoditySwaption],
    )?;
    Ok(())
}
