//! Pricing diagnostics for interest-rate futures.

use crate::instruments::rates::ir_future::InterestRateFuture;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::Result;

/// Quoted futures price.
pub(crate) struct FuturesPriceCalculator;

impl MetricCalculator for FuturesPriceCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let future: &InterestRateFuture = context.instrument_as()?;
        Ok(future.quoted_price)
    }
}

/// Model forward rate over the futures accrual period.
pub(crate) struct ImpliedForwardCalculator;

impl MetricCalculator for ImpliedForwardCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let future: &InterestRateFuture = context.instrument_as()?;
        future.model_forward_rate(&context.curves)
    }
}

/// Convexity adjustment applied to the model forward rate.
pub(crate) struct ConvexityAdjustmentCalculator;

impl MetricCalculator for ConvexityAdjustmentCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let future: &InterestRateFuture = context.instrument_as()?;
        future.convexity_adjustment(&context.curves)
    }
}
