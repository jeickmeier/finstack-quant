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

use crate::metrics::MetricRegistry;

/// Register all NDF metrics with the registry.
pub(crate) fn register_ndf_metrics(registry: &mut MetricRegistry) {
    use crate::metrics::MetricId;
    use crate::pricer::InstrumentType;

    // Fx01 uses the shared `GenericFx01Calculator` (1% relative spot move).
    // The previous per-NDF calculator had bespoke quote-convention logic
    // (BasePerSettlement vs SettlementPerBase). Routing through MarketBump
    // + the canonical NDF pricer preserves that quote-convention awareness
    // automatically — the bump goes through the FX matrix, and `Ndf::value`
    // already reads spot in its own convention. The previous calculator's
    // regression tests are subsumed by the generic + canonical pricer path.
    registry.register_metric(
        MetricId::Fx01,
        crate::metrics::sensitivities::fx01::arc_generic_fx01(),
        &[InstrumentType::Ndf],
    );
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Ndf,
        metrics: [
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::fx::ndf::Ndf,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::fx::ndf::Ndf,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
}
