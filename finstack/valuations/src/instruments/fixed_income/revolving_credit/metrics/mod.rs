//! Metrics module for revolving credit facilities.
//!
//! Provides both standard metrics (PV, DV01, Theta, BucketedDV01, CS01) and
//! facility-specific metrics (utilization rate, available capacity, weighted average cost, IRR).

pub(crate) mod available_capacity;
pub(crate) mod cs01;
pub(crate) mod irr;
pub(crate) mod utilization_rate;
pub(crate) mod weighted_average_cost;

pub(crate) use available_capacity::AvailableCapacityCalculator;
pub(crate) use utilization_rate::UtilizationRateCalculator;
pub(crate) use weighted_average_cost::ApproxWeightedAverageCostCalculator;

use crate::metrics::MetricRegistry;

/// Register all revolving credit metrics with the registry.
///
/// Registers both standard metrics (PV, DV01, Theta, BucketedDV01, CS01) and
/// facility-specific metrics (utilization rate, available capacity, weighted average cost).
pub(crate) fn register_revolving_credit_metrics(registry: &mut MetricRegistry) {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::RevolvingCredit,
        metrics: [
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::RevolvingCredit,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            // CS01: when a credit curve is present the pricer survival-weights
            // cashflows, so a par-spread bump moves PV — delegate to the
            // canonical hazard CS01. With no credit curve, survival is 1.0 and
            // the canonical CS01 is zero, so fall back to the market-standard
            // z-spread bump. See `metrics::sensitivities::cs01_z_spread`.
            (Cs01, crate::metrics::ZSpreadParallelCs01::<
                crate::instruments::RevolvingCredit,
            >::hazard_when_credit_curve()),
            (BucketedCs01, crate::metrics::ZSpreadBucketedCs01::<
                crate::instruments::RevolvingCredit,
            >::hazard_when_credit_curve()),
            (Cs01Hazard, crate::metrics::GenericParallelCs01Hazard::<
                crate::instruments::RevolvingCredit,
            >::with_empty_credit_curve_zero()),
            (BucketedCs01Hazard, crate::metrics::GenericBucketedCs01Hazard::<
                crate::instruments::RevolvingCredit,
            >::with_empty_credit_curve_zero()),
            // Theta is now registered universally in metrics::standard_registry()
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::RevolvingCredit,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }

    // Register facility-specific metrics with custom IDs
    use crate::metrics::MetricId;
    use std::sync::Arc;

    registry.register_metric(
        MetricId::custom("utilization_rate"),
        Arc::new(UtilizationRateCalculator),
        &[InstrumentType::RevolvingCredit],
    );

    registry.register_metric(
        MetricId::custom("available_capacity"),
        Arc::new(AvailableCapacityCalculator),
        &[InstrumentType::RevolvingCredit],
    );

    registry.register_metric(
        MetricId::custom("weighted_average_cost"),
        Arc::new(ApproxWeightedAverageCostCalculator),
        &[InstrumentType::RevolvingCredit],
    );
}
