//! Metric registration for options on equity futures.

use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register delta, gamma, vega, and theta for equity futures options.
pub(crate) fn register_equity_future_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: finstack_quant_valuations_core::pricer::InstrumentType::EquityFutureOption,
        metrics: [
            (Delta, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::EquityFutureOption>::delta()),
            (Gamma, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::EquityFutureOption>::gamma()),
            (Vega, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::EquityFutureOption>::vega()),
            (Theta, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::EquityFutureOption>::theta()),
        ]
    }
    Ok(())
}
