//! Metric registration for equity total-return futures.

use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricRegistry};

struct FuturesPrice;

impl MetricCalculator for FuturesPrice {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let future: &crate::equity_total_return_future::EquityTotalReturnFuture =
            context.instrument_as()?;
        future.mark_price(&context.curves, context.as_of)
    }
}

struct Delta;

impl MetricCalculator for Delta {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let future: &crate::equity_total_return_future::EquityTotalReturnFuture =
            context.instrument_as()?;
        future.spot_delta(&context.curves, context.as_of)
    }
}

struct Spread01;

impl MetricCalculator for Spread01 {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let future: &crate::equity_total_return_future::EquityTotalReturnFuture =
            context.instrument_as()?;
        future.spread01(&context.curves, context.as_of)
    }
}

/// Register clearing price, spot delta, and financing-spread risk for equity total-return futures.
pub(crate) fn register_equity_total_return_future_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: finstack_quant_valuations_core::pricer::InstrumentType::EquityTotalReturnFuture,
        metrics: [
            (FuturesPrice, FuturesPrice),
            (Delta, Delta),
            (Spread01, Spread01),
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
    fn registers_financing_spread_risk() {
        let mut registry = MetricRegistry::new();
        register_equity_total_return_future_metrics(&mut registry)
            .expect("equity TRF metric registration");
        assert!(registry
            .metrics_for_instrument(InstrumentType::EquityTotalReturnFuture)
            .contains(&MetricId::Spread01));
    }
}
