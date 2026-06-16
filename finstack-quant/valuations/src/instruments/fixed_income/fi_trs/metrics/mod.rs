//! Risk metrics for Fixed Income Index Total Return Swaps.
//!
//! This module provides FI TRS-specific risk metrics:
//! - **DurationDv01**: Duration-based yield sensitivity (`N × D × 1bp`)
//! - **ParSpread**: Spread that makes NPV = 0
//! - **FinancingAnnuity**: PV01 of the financing leg
//! - **Dv01 / BucketedDv01**: Financing curve rate sensitivity

mod annuity;
mod duration_dv01;
mod par_spread;

pub(crate) use annuity::FinancingAnnuityCalculator;
pub(crate) use duration_dv01::DurationDv01Calculator;
pub(crate) use par_spread::ParSpreadCalculator;

use crate::metrics::MetricRegistry;
use crate::pricer::InstrumentType;

/// Registers all fixed income TRS metrics with the metric registry.
///
/// # Arguments
/// * `registry` — Metric registry to add FI TRS metrics to
pub(crate) fn register_fi_trs_metrics(registry: &mut MetricRegistry) {
    use crate::metrics::MetricId;
    use std::sync::Arc;

    let instruments = [InstrumentType::FIIndexTotalReturnSwap];

    // FI TRS specific metrics
    //
    // Duration-based yield sensitivity. Registered under DurationDv01 (not IndexDelta)
    // because this measures yield sensitivity (N × D × 1bp), which is conceptually
    // distinct from equity IndexDelta (dV/dS per unit of index level change).
    registry.register_metric(
        MetricId::DurationDv01,
        Arc::new(DurationDv01Calculator),
        &instruments,
    );
    registry.register_metric(
        MetricId::ParSpread,
        Arc::new(ParSpreadCalculator),
        &instruments,
    );
    registry.register_metric(
        MetricId::FinancingAnnuity,
        Arc::new(FinancingAnnuityCalculator),
        &instruments,
    );

    // DV01 for financing leg sensitivity
    registry.register_metric(
        MetricId::Dv01,
        Arc::new(crate::metrics::UnifiedDv01Calculator::<
            crate::instruments::fixed_income::fi_trs::FIIndexTotalReturnSwap,
        >::new(
            crate::metrics::Dv01CalculatorConfig::parallel_combined()
        )),
        &instruments,
    );
    registry.register_metric(
        MetricId::BucketedDv01,
        Arc::new(crate::metrics::UnifiedDv01Calculator::<
            crate::instruments::fixed_income::fi_trs::FIIndexTotalReturnSwap,
        >::new(
            crate::metrics::Dv01CalculatorConfig::triangular_key_rate(),
        )),
        &instruments,
    );
}
