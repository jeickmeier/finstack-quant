//! CDS Tranche metrics module.
//!
//! Provides metric calculators specific to `CDSTranche`, split into focused
//! files. The calculators compose with the shared metrics framework and are
//! registered via `register_cds_tranche_metrics`.
//!
//! Exposed metrics:
//! - Upfront (currency units)
//! - Spread DV01 (per 1bp change in running coupon)
//! - Expected loss (currency units)
//! - Jump-to-default (currency units)
//! - CS01 (parallel par-spread rebootstrap)
//! - Correlation01 (per 1% correlation change)
//! - Recovery01 (per 1% recovery change)
//! - TailDependence (copula tail dependence coefficient)

mod correlation01;
mod cs01;
mod expected_loss;
mod jump_to_default;
mod par_spread;
mod recovery01;
mod spread_dv01;
mod tail_dependence;
mod upfront;

use crate::metrics::MetricRegistry;

/// Register all CDS Tranche metrics with the registry
pub(crate) fn register_cds_tranche_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    use crate::metrics::MetricId;
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    for (id, calculator) in [
        (
            MetricId::custom("upfront"),
            Arc::new(upfront::UpfrontCalculator) as Arc<dyn crate::metrics::MetricCalculator>,
        ),
        (
            MetricId::SpreadDv01,
            Arc::new(spread_dv01::SpreadDv01Calculator),
        ),
        (
            MetricId::Correlation01,
            Arc::new(correlation01::Correlation01Calculator),
        ),
        (
            MetricId::Recovery01,
            Arc::new(recovery01::Recovery01Calculator),
        ),
        (
            MetricId::custom("tail_dependence"),
            Arc::new(tail_dependence::TailDependenceCalculator),
        ),
    ] {
        registry.replace_metric(id, calculator, &[InstrumentType::CdsTranche])?;
    }

    // Standard metrics using macro
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CdsTranche,
        metrics: [
            (Cs01, cs01::CdsTrancheCs01Calculator),
            (BucketedCs01, cs01::CdsTrancheBucketedCs01Calculator),
            (ParSpread, par_spread::ParSpreadCalculator),
            (ExpectedLoss, expected_loss::ExpectedLossCalculator),
            (JumpToDefault, jump_to_default::JumpToDefaultCalculator),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::CDSTranche,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            // Theta is now registered universally in metrics::standard_registry()
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::CDSTranche,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}
