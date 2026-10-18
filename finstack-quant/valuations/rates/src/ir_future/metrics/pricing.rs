//! Pricing diagnostics for interest-rate futures.

use crate::ir_future::InterestRateFuture;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

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
