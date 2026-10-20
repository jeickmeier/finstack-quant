//! IR Future metrics module.
//!
//! Provides metric calculators specific to `InterestRateFuture`, split into
//! focused files. The calculators compose with the shared metrics framework
//! and are registered via `register_ir_future_metrics`.
//!
//! Exposed metrics:
//! - DV01 (parallel rate sensitivity via generic calculator)
//! - Bucketed DV01 (key-rate sensitivity)
//!
//! Note: PV is available in `ValuationResult.value`, not as a metric in measures.

use finstack_quant_valuations_core::metrics::MetricRegistry;

mod pricing;

/// Register IR Future metrics with the registry
pub(crate) fn register_ir_future_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;
    // Standard metrics using macro
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::InterestRateFuture,
        metrics: [
            (FuturesPrice, pricing::FuturesPriceCalculator),
            (ImpliedForward, pricing::ImpliedForwardCalculator),
            (ConvexityAdjustment, pricing::ConvexityAdjustmentCalculator),
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_valuations_core::metrics::MetricId;
    use finstack_quant_valuations_core::pricer::InstrumentType;

    #[test]
    fn registers_contract_quote_and_forward_metrics() {
        let mut registry = MetricRegistry::new();
        register_ir_future_metrics(&mut registry).expect("IR future metric registration");
        let metrics = registry.metrics_for_instrument(InstrumentType::InterestRateFuture);

        assert!(metrics.contains(&MetricId::FuturesPrice));
        assert!(metrics.contains(&MetricId::ImpliedForward));
        assert!(metrics.contains(&MetricId::ConvexityAdjustment));
    }
}
