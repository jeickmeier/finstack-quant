//! Metric registration for options on commodity futures.

use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register delta, gamma, vega, and theta for commodity futures options.
pub(crate) fn register_commodity_future_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: finstack_quant_valuations_core::pricer::InstrumentType::CommodityFutureOption,
        metrics: [
            (Delta, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::CommodityFutureOption>::delta()),
            (Gamma, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::CommodityFutureOption>::gamma()),
            (Vega, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::CommodityFutureOption>::vega()),
            (Theta, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::CommodityFutureOption>::theta()),
        ]
    }
    Ok(())
}
