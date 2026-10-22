//! FX barrier option metrics module.
//!
//! Provides full greek coverage for FX barrier options using finite difference methods.
//! Note: FX barrier options exhibit discontinuous greeks near the barrier level.
//! Delta represents FX spot sensitivity.

use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register FX barrier option metrics with the registry.
pub(crate) fn register_fx_barrier_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::FxBarrierOption,
        metrics: [
            (Delta, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::fx_barrier_option::FxBarrierOption>::delta()),
            (Gamma, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::fx_barrier_option::FxBarrierOption>::gamma()),
            (Vega, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::fx_barrier_option::FxBarrierOption>::vega()),
            (Rho, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::fx_barrier_option::FxBarrierOption>::rho()),
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
            (Vanna, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::fx_barrier_option::FxBarrierOption>::vanna()),
            (Volga, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::fx_barrier_option::FxBarrierOption>::volga()),
        ]
    }
    Ok(())
}
