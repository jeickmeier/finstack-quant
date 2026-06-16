//! FX touch option metrics module.
//!
//! Provides risk metrics for FX touch options using finite difference methods.
//! Touch options exhibit discontinuous Greeks near the barrier, so finite
//! differences are preferred over analytical formulas.

use crate::metrics::MetricRegistry;

/// Register FX touch option metrics with the registry.
pub(crate) fn register_fx_touch_option_metrics(registry: &mut MetricRegistry) {
    use crate::pricer::InstrumentType;

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::FxTouchOption,
        metrics: [
            (Delta, crate::metrics::OptionGreekCalculator::<crate::instruments::FxTouchOption>::delta()),
            (Gamma, crate::metrics::OptionGreekCalculator::<crate::instruments::FxTouchOption>::gamma()),
            (Vega, crate::metrics::OptionGreekCalculator::<crate::instruments::FxTouchOption>::vega()),
            (Rho, crate::metrics::OptionGreekCalculator::<crate::instruments::FxTouchOption>::rho()),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::FxTouchOption,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::FxTouchOption,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
}
