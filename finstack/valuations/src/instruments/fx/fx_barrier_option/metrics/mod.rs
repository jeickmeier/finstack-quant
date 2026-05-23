//! FX barrier option metrics module.
//!
//! Provides full greek coverage for FX barrier options using finite difference methods.
//! Note: FX barrier options exhibit discontinuous greeks near the barrier level.
//! Delta represents FX spot sensitivity.

use crate::metrics::MetricRegistry;

/// Register FX barrier option metrics with the registry.
pub(crate) fn register_fx_barrier_option_metrics(registry: &mut MetricRegistry) {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::FxBarrierOption,
        metrics: [
            (Delta, crate::metrics::OptionGreekCalculator::<crate::instruments::FxBarrierOption>::delta()),
            (Gamma, crate::metrics::OptionGreekCalculator::<crate::instruments::FxBarrierOption>::gamma()),
            (Vega, crate::metrics::OptionGreekCalculator::<crate::instruments::FxBarrierOption>::vega()),
            (Rho, crate::metrics::OptionGreekCalculator::<crate::instruments::FxBarrierOption>::rho()),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::FxBarrierOption,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::FxBarrierOption,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
            (Vanna, crate::metrics::OptionGreekCalculator::<crate::instruments::FxBarrierOption>::vanna()),
            (Volga, crate::metrics::OptionGreekCalculator::<crate::instruments::FxBarrierOption>::volga()),
        ]
    }
}
