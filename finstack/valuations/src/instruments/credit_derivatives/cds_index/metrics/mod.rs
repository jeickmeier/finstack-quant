//! CDS Index metrics module.
//!
//! Provides metric calculators specific to `CDSIndex`, split into focused
//! files. The calculators compose with the shared metrics framework and are
//! registered via `register_cds_index_metrics`.
//!
//! Exposed metrics:
//! - Par spread (bps)
//! - Risky PV01
//! - CS01 (approximate)
//! - Bucketed CS01 (key-rate, per-tenor par-spread)
//! - Protection leg PV
//! - Premium leg PV
//! - Expected loss
//! - Jump to default

mod cs01;
mod expected_loss;
mod jump_to_default;
mod recovery01;
mod simple;

use crate::metrics::MetricRegistry;

/// Register all CDS Index metrics with the registry
pub(crate) fn register_cds_index_metrics(registry: &mut MetricRegistry) {
    use crate::metrics::MetricId;
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    registry.register_metric(
        MetricId::RiskyPv01,
        Arc::new(simple::RiskyPv01Calculator),
        &[InstrumentType::CDSIndex],
    );

    // Recovery01 (custom metric - recovery rate sensitivity)
    registry.register_metric(
        MetricId::Recovery01,
        Arc::new(recovery01::Recovery01Calculator),
        &[InstrumentType::CDSIndex],
    );

    // Standard metrics using macro
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CDSIndex,
        metrics: [
            (ParSpread, simple::ParSpreadCalculator),
            (Cs01, cs01::Cs01Calculator),
            (BucketedCs01, cs01::CdsIndexBucketedCs01Calculator),
            (Cs01Hazard, cs01::Cs01HazardCalculator),
            (ProtectionLegPv, simple::ProtectionLegPvCalculator),
            (PremiumLegPv, simple::PremiumLegPvCalculator),
            (ExpectedLoss, expected_loss::ExpectedLossCalculator),
            (JumpToDefault, jump_to_default::JumpToDefaultCalculator),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::CDSIndex,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            // Theta is now registered universally in metrics::standard_registry()
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::CDSIndex,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
}
