//! CDS Index metrics module.
//!
//! Provides metric calculators specific to `CdsIndex`, split into focused
//! files. The calculators compose with the shared metrics framework and are
//! registered via `register_cds_index_metrics`.
//!
//! Exposed metrics:
//! - Par spread (bp)
//! - Risky PV01
//! - CS01 (approximate)
//! - Bucketed CS01 (key-rate, per-tenor par-spread)
//! - Protection leg PV
//! - Premium leg PV
//! - Expected loss
//! - Jump to default

mod cs01;
mod dv01;
mod expected_loss;
mod jump_to_default;
mod recovery01;
mod simple;

use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register all CDS Index metrics with the registry
pub(crate) fn register_cds_index_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;

    // Standard metrics using macro
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CdsIndex,
        metrics: [
            (ParSpread, simple::ParSpreadCalculator),
            (Cs01, cs01::Cs01Calculator),
            (BucketedCs01, cs01::CdsIndexBucketedCs01Calculator),
            (ProtectionLegPv, simple::ProtectionLegPvCalculator),
            (PremiumLegPv, simple::PremiumLegPvCalculator),
            (ExpectedLoss, expected_loss::ExpectedLossCalculator),
            (JumpToDefault, jump_to_default::JumpToDefaultCalculator),
            (Dv01, dv01::CdsIndexDv01Calculator),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
            (RiskyPv01, simple::RiskyPv01Calculator),
            // Recovery01 (custom metric - recovery rate sensitivity)
            (Recovery01, recovery01::Recovery01Calculator),
        ]
    }
    Ok(())
}
