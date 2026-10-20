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
            (Delta, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::fx_digital_option::FxDigitalOption>::delta()),
            (Gamma, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::fx_digital_option::FxDigitalOption>::gamma()),
            (Vega, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::fx_digital_option::FxDigitalOption>::vega()),
            (Theta, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::fx_digital_option::FxDigitalOption>::theta()),
            (Rho, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::fx_digital_option::FxDigitalOption>::rho()),
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}
