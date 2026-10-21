//! Metric registration for options on FX futures.

use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register delta, gamma, vega, and theta for FX futures options.
pub(crate) fn register_fx_future_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: finstack_quant_valuations_core::pricer::InstrumentType::FxFutureOption,
        metrics: [
            (Delta, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::fx_future_option::FxFutureOption>::delta()),
            (Gamma, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::fx_future_option::FxFutureOption>::gamma()),
            (Vega, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::fx_future_option::FxFutureOption>::vega()),
            (Theta, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::fx_future_option::FxFutureOption>::theta()),
        ]
    }
    Ok(())
}
