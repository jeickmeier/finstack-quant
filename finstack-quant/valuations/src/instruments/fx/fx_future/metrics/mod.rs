//! Metric registration for exchange-listed FX futures.

use crate::metrics::{MetricCalculator, MetricContext, MetricRegistry};

struct FuturesPrice;

impl MetricCalculator for FuturesPrice {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let future: &crate::instruments::FxFuture = context.instrument_as()?;
        future.mark_price(&context.curves, context.as_of)
    }
}

struct Delta;

impl MetricCalculator for Delta {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let future: &crate::instruments::FxFuture = context.instrument_as()?;
        future.futures_price_delta()
    }
}

/// Register fair price, point delta, and curve DV01 for listed FX futures.
pub(crate) fn register_fx_future_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    crate::register_metrics! {
        registry: registry,
        instrument: crate::pricer::InstrumentType::FxFuture,
        metrics: [
            (FuturesPrice, FuturesPrice),
            (Delta, Delta),
            (Dv01, crate::metrics::UnifiedDv01Calculator::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}
