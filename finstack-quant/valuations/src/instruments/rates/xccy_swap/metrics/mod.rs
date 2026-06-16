//! XCCY swap metrics module.
//!
//! Registers standard rate risk metrics for cross-currency swaps.

use crate::metrics::MetricRegistry;

/// Register XCCY swap metrics with the registry.
pub(crate) fn register_xccy_swap_metrics(registry: &mut MetricRegistry) {
    use crate::metrics::{
        make_fx_bumper, make_rates_bumper, CrossFactorCalculator, CrossFactorPair, MetricId,
    };
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    registry.register_metric(
        MetricId::CrossGammaFxRates,
        Arc::new(CrossFactorCalculator::new(
            CrossFactorPair::FxRates,
            make_fx_bumper,
            make_rates_bumper,
        )),
        &[InstrumentType::XccySwap],
    );

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::XccySwap,
        metrics: [
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::XccySwap,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::XccySwap,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    };
}
