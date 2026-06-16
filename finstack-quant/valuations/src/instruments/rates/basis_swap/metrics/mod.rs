//! Basis swap specific metrics.
//!
//! Provides calculators for leg present values, discounted accrual sums (annuities),
//! DV01s, and par spread calculations for basis swap instruments.

/// Annuity (discounted accrual sum) calculators for basis swap legs
mod annuity;
/// Par spread calculator for basis swaps
mod par_spread;
/// Present value calculators for basis swap legs
mod pv;

pub(crate) use annuity::AnnuityCalculator;
pub(crate) use par_spread::{IncrementalParSpreadCalculator, ParSpreadCalculator};
pub(crate) use pv::PvCalculator;

use crate::metrics::{MetricId, MetricRegistry};
use crate::pricer::InstrumentType;
use std::sync::Arc;

/// Registers all basis swap metrics in the standard metric registry.
///
/// This function adds the basis swap specific metric calculators to the provided
/// registry, making them available for use in valuation calculations.
///
/// # Arguments
/// * `registry` — The metric registry to register the calculators with
pub(crate) fn register_basis_swap_metrics(registry: &mut MetricRegistry) {
    // Leg-specific metrics with primary/reference constructors
    registry
        .register_metric(
            MetricId::AnnuityPrimary,
            Arc::new(AnnuityCalculator::primary()),
            &[InstrumentType::BasisSwap],
        )
        .register_metric(
            MetricId::AnnuityReference,
            Arc::new(AnnuityCalculator::reference()),
            &[InstrumentType::BasisSwap],
        )
        .register_metric(
            MetricId::PvPrimary,
            Arc::new(PvCalculator::primary()),
            &[InstrumentType::BasisSwap],
        )
        .register_metric(
            MetricId::PvReference,
            Arc::new(PvCalculator::reference()),
            &[InstrumentType::BasisSwap],
        );

    // DV01 using GenericParallelDv01 in PerCurve mode
    // This bumps each curve individually (discount, primary forward, reference forward)
    // and stores the results in a bucketed series under BucketedDv01
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::BasisSwap,
        metrics: [
            (Dv01, crate::metrics::UnifiedDv01Calculator::<crate::instruments::BasisSwap>::new(crate::metrics::Dv01CalculatorConfig::parallel_per_curve())),
            (BasisParSpread, ParSpreadCalculator),
            (IncrementalParSpread, IncrementalParSpreadCalculator),
            // Theta is now registered universally in metrics::standard_registry()
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::BasisSwap,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
}
