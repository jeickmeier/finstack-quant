//! Metric registration for options on interest-rate futures.

use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricRegistry};

#[derive(Debug, Clone, Default)]
struct InterestRateFutureOptionDv01;

impl MetricCalculator for InterestRateFutureOptionDv01 {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let option: &crate::InterestRateFutureOption = context.instrument_as()?;
        option.rate_dv01(&context.curves, context.as_of)
    }
}

/// Register price Greeks and mapped rate DV01 for interest-rate futures options.
pub(crate) fn register_interest_rate_future_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: finstack_quant_valuations_core::pricer::InstrumentType::InterestRateFutureOption,
        metrics: [
            (Delta, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::InterestRateFutureOption>::delta()),
            (Gamma, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::InterestRateFutureOption>::gamma()),
            (Vega, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::InterestRateFutureOption>::vega()),
            (Theta, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::InterestRateFutureOption>::theta()),
            (Dv01, InterestRateFutureOptionDv01),
        ]
    }
    Ok(())
}
