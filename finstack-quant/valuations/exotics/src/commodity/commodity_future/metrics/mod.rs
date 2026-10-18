//! Metric registration for exchange-listed commodity futures.

use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricRegistry};

struct FuturesPrice;

impl MetricCalculator for FuturesPrice {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let future: &crate::CommodityFuture = context.instrument_as()?;
        future.mark_price(&context.curves, context.as_of)
    }
}

struct Delta;

impl MetricCalculator for Delta {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let future: &crate::CommodityFuture = context.instrument_as()?;
        future.price_curve_delta(context.as_of)
    }
}

/// Register price and projected-curve delta for listed commodity futures.
pub(crate) fn register_commodity_future_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: finstack_quant_valuations_core::pricer::InstrumentType::CommodityFuture,
        metrics: [
            (FuturesPrice, FuturesPrice),
            (Delta, Delta),
        ]
    }
    Ok(())
}
