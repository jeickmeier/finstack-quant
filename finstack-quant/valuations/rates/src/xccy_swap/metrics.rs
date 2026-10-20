//! XCCY swap metrics module.
//!
//! Registers rate and FX risk metrics for cross-currency swaps.

use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register XCCY swap metrics with the registry.
pub(crate) fn register_xccy_swap_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::sensitivities::cross_factor::{
        make_fx_bumper, make_rates_bumper, CrossFactorCalculator,
    };
    use finstack_quant_valuations_core::metrics::sensitivities::fx01::GenericFx01Calculator;
    use finstack_quant_valuations_core::pricer::InstrumentType;

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::XccySwap,
        metrics: [
            // PV change per 1% relative move in the swap's FX pair. A
            // fixed-notional swap carries roughly its foreign notional of FX
            // exposure, so this is a first-order risk, not only a cross-gamma
            // input.
            (Fx01, GenericFx01Calculator),
            (FxDelta, GenericFx01Calculator),
            // Combined DV01 sums every rate curve of both currencies, so the
            // two legs' offsetting curve risks net. `Pv01` reports each curve
            // separately as `pv01::{curve}`.
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (Pv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_per_curve()
                .with_series_id(finstack_quant_valuations_core::metrics::MetricId::Pv01))),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
            (CrossGammaFxRates, CrossFactorCalculator::new(make_fx_bumper, make_rates_bumper)),
        ]
    };
    Ok(())
}
