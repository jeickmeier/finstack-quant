//! NDF metrics module.
//!
//! Provides metric calculators specific to `Ndf`, split into focused files.
//! The calculators compose with the shared metrics framework and are registered
//! via `register_ndf_metrics`.
//!
//! Exposed metrics:
//! - DV01 (interest rate sensitivity for settlement curve)
//! - Theta (time decay)

#[cfg(test)]
mod tests;

use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register all NDF metrics with the registry.
pub(crate) fn register_ndf_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::sensitivities::fx01::GenericFx01Calculator;
    use finstack_quant_valuations_core::pricer::InstrumentType;

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Ndf,
        metrics: [
            // The bump goes through the FX matrix and `Ndf::value` reads spot
            // in its own quote convention, so the generic calculator honours
            // BasePerSettlement vs SettlementPerBase quoting.
            (Fx01, GenericFx01Calculator),
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}
