//! FX digital option metrics module.
//!
//! Registers option Greeks and risk metrics for FX digital options.

use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register FX digital option metrics with the registry.
pub(crate) fn register_fx_digital_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::FxDigitalOption,
        metrics: [
            (Delta, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::FxDigitalOption>::delta()),
            (Gamma, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::FxDigitalOption>::gamma()),
            (Vega, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::FxDigitalOption>::vega()),
            (Theta, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::FxDigitalOption>::theta()),
            (Rho, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::FxDigitalOption>::rho()),
            (Dv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}
