//! IR Future metrics module.
//!
//! Provides metric calculators specific to `InterestRateFuture`. The
//! calculators compose with the shared metrics framework and are registered via
//! `register_ir_future_metrics`.
//!
//! Exposed metrics:
//! - FuturesPrice, ImpliedForward, ConvexityAdjustment (pricing diagnostics)
//! - DV01 (parallel rate sensitivity via generic calculator)
//! - Bucketed DV01 (key-rate sensitivity)
//!
//! Note: PV is available in `ValuationResult.value`, not as a metric in measures.

use crate::ir_future::InterestRateFuture;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::MetricRegistry;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};
use std::sync::Arc;

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
            (FuturesPrice, FuturesPriceCalculator),
            (ImpliedForward, ImpliedForwardCalculator),
            (ConvexityAdjustment, ConvexityAdjustmentCalculator),
            (Dv01, IrFutureDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, IrFutureDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}

/// DV01 wrapper for `InterestRateFuture` that evaluates rate-curve sensitivity
/// through the model price even when a live `terms.quoted_price` is set.
///
/// A quoted future the model cannot price (no convexity source) keeps its
/// quote-based DV01.
pub(crate) struct IrFutureDv01Calculator {
    inner: finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator,
}

impl IrFutureDv01Calculator {
    pub(crate) fn new(
        config: finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig,
    ) -> Self {
        Self {
            inner: finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(
                config,
            ),
        }
    }
}

impl MetricCalculator for IrFutureDv01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let future: &InterestRateFuture = context.instrument_as()?;
        let live_quote =
            context.as_of <= future.terms.last_trading_date && future.terms.quoted_price.is_some();
        if !live_quote || !future.can_price_from_model() {
            return self.inner.calculate(context);
        }
        let mut model_future = future.clone();
        model_future.terms.quoted_price = None;
        model_future.validate_for_pricing()?;
        let model_pv = model_future.base_value(&context.curves, context.as_of)?;
        context.with_instrument(Arc::new(model_future), |ctx| {
            ctx.set_base_value(model_pv);
            self.inner.calculate(ctx)
        })
    }
}

// Pricing diagnostics for interest-rate futures.

/// Lifecycle-aware current or final futures price.
pub(crate) struct FuturesPriceCalculator;

impl MetricCalculator for FuturesPriceCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let future: &InterestRateFuture = context.instrument_as()?;
        future.mark_price(&context.curves, context.as_of)
    }
}

/// Model forward rate over the futures accrual period.
pub(crate) struct ImpliedForwardCalculator;

impl MetricCalculator for ImpliedForwardCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let future: &InterestRateFuture = context.instrument_as()?;
        future.model_settlement_rate(&context.curves, context.as_of)
    }
}

/// Convexity adjustment applied to the model forward rate.
pub(crate) struct ConvexityAdjustmentCalculator;

impl MetricCalculator for ConvexityAdjustmentCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let future: &InterestRateFuture = context.instrument_as()?;
        future.convexity_adjustment(&context.curves, context.as_of)
    }
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
