//! Inflation cap/floor metrics module.
//!
//! Provides metric calculators specific to `InflationCapFloor`, split into focused files.
//! Metrics are registered via `register_inflation_cap_floor_metrics`.
//!
//! # Available Metrics
//!
//! - **Inflation01**: Sensitivity to 1bp parallel shift in inflation curve
//! - **Gamma**: Second derivative (convexity) with respect to inflation rate
//! - **Vega**: Sensitivity to 1% absolute shift in volatility surface
//! - **Dv01**: Sensitivity to 1bp shift in discount curve
//! - **BucketedDv01**: Key-rate sensitivities to discount curve
//! - **Theta**: Time decay (1-day roll)

mod gamma;
mod inflation01;
mod vega;

use crate::metrics::MetricRegistry;

/// Register all inflation cap/floor metrics with the registry.
pub(crate) fn register_inflation_cap_floor_metrics(registry: &mut MetricRegistry) {
    use crate::metrics::MetricId;
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    registry.register_metric(
        MetricId::Inflation01,
        Arc::new(inflation01::Inflation01Calculator),
        &[InstrumentType::InflationCapFloor],
    );

    registry.register_metric(
        MetricId::Gamma,
        Arc::new(gamma::GammaCalculator),
        &[InstrumentType::InflationCapFloor],
    );

    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::InflationCapFloor,
        metrics: [
            (Vega, vega::VegaCalculator),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::InflationCapFloor,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::InflationCapFloor,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    };
}
