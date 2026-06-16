//! IR Future Option metrics module.
//!
//! Registers standard option greeks (Delta, Gamma, Vega, Theta) and DV01
//! with the metric registry for `IrFutureOption`.

use crate::metrics::MetricRegistry;

/// Register IR Future Option metrics with the registry.
pub(crate) fn register_ir_future_option_metrics(registry: &mut MetricRegistry) {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::IrFutureOption,
        metrics: [
            (Delta, crate::metrics::OptionGreekCalculator::<
                crate::instruments::IrFutureOption,
            >::delta()),
            (Gamma, crate::metrics::OptionGreekCalculator::<
                crate::instruments::IrFutureOption,
            >::gamma()),
            (Vega, crate::metrics::OptionGreekCalculator::<
                crate::instruments::IrFutureOption,
            >::vega()),
            (Theta, crate::metrics::OptionGreekCalculator::<
                crate::instruments::IrFutureOption,
            >::theta()),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::IrFutureOption,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::IrFutureOption,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
}
