//! Metric registration and calculators.

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
            (ParSpread, super::simple::ParSpreadCalculator),
            (Cs01, super::cs01::Cs01Calculator, NativeMoney),
            (BucketedCs01, super::cs01::CdsIndexBucketedCs01Calculator, NativeMoney),
            (ProtectionLegPv, super::simple::ProtectionLegPvCalculator),
            (PremiumLegPv, super::simple::PremiumLegPvCalculator),
            (ExpectedLoss, super::expected_loss::ExpectedLossCalculator),
            (JumpToDefault, super::jump_to_default::JumpToDefaultCalculator),
            (Dv01, super::dv01::CdsIndexDv01Calculator(false)),
            (BucketedDv01, super::dv01::CdsIndexDv01Calculator(true)),
            (RiskyPv01, super::simple::RiskyPv01Calculator),
            // Recovery01 (custom metric - recovery rate sensitivity)
            (Recovery01, super::recovery01::Recovery01Calculator),
        ]
    }
    Ok(())
}
