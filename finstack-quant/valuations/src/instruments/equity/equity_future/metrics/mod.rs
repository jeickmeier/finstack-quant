//! Metric registration for exchange-listed equity futures.

use crate::metrics::{MetricCalculator, MetricContext, MetricRegistry};

struct FuturesPrice;

impl MetricCalculator for FuturesPrice {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let future: &crate::instruments::EquityFuture = context.instrument_as()?;
        future.mark_price(&context.curves, context.as_of)
    }
}

struct Basis;

impl MetricCalculator for Basis {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let future: &crate::instruments::EquityFuture = context.instrument_as()?;
        let futures_price = future
            .terms
            .quoted_price
            .map_or_else(|| future.fair_price(&context.curves, context.as_of), Ok)?;
        let spot = crate::instruments::common_impl::helpers::scalar_price_amount(
            context.curves.get_price(&future.spot_id)?,
            future.underlying_currency,
        )?;
        Ok(futures_price - spot)
    }
}

struct Delta;

impl MetricCalculator for Delta {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let future: &crate::instruments::EquityFuture = context.instrument_as()?;
        future.spot_delta(&context.curves, context.as_of)
    }
}

/// Register fair price, basis, spot delta, and curve DV01 for equity futures.
pub(crate) fn register_equity_future_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    crate::register_metrics! {
        registry: registry,
        instrument: crate::pricer::InstrumentType::EquityFuture,
        metrics: [
            (FuturesPrice, FuturesPrice),
            (Basis, Basis),
            (Delta, Delta),
            (Dv01, crate::metrics::UnifiedDv01Calculator::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}
