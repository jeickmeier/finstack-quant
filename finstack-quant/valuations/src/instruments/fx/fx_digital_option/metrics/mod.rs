//! FX digital option metrics module.
//!
//! Registers option Greeks and risk metrics for FX digital options.

use crate::metrics::MetricRegistry;

/// Register FX digital option metrics with the registry.
pub(crate) fn register_fx_digital_option_metrics(registry: &mut MetricRegistry) {
    use crate::pricer::InstrumentType;

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::FxDigitalOption,
        metrics: [
            (Delta, crate::metrics::OptionGreekCalculator::<crate::instruments::FxDigitalOption>::delta()),
            (Gamma, crate::metrics::OptionGreekCalculator::<crate::instruments::FxDigitalOption>::gamma()),
            (Vega, crate::metrics::OptionGreekCalculator::<crate::instruments::FxDigitalOption>::vega()),
            (Theta, crate::metrics::OptionGreekCalculator::<crate::instruments::FxDigitalOption>::theta()),
            (Rho, crate::metrics::OptionGreekCalculator::<crate::instruments::FxDigitalOption>::rho()),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::FxDigitalOption,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::FxDigitalOption,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
}
