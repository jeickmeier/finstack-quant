//! Metric registration for exchange-listed equity futures.

use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricRegistry};

struct FuturesPrice;

impl MetricCalculator for FuturesPrice {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let future: &crate::equity_future::EquityFuture = context.instrument_as()?;
        future.mark_price(&context.curves, context.as_of)
    }
}

struct Basis;

impl MetricCalculator for Basis {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let future: &crate::equity_future::EquityFuture = context.instrument_as()?;
        let futures_price = future
            .terms
            .quoted_price
            .map_or_else(|| future.fair_price(&context.curves, context.as_of), Ok)?;
        let spot =
            finstack_quant_valuations_core::instruments::common_impl::helpers::scalar_price_amount(
                context.curves.get_price(&future.spot_id)?,
                future.underlying_currency,
            )?;
        Ok(futures_price - spot)
    }
}

struct Delta;

impl MetricCalculator for Delta {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let future: &crate::equity_future::EquityFuture = context.instrument_as()?;
        future.spot_delta(&context.curves, context.as_of)
    }
}

/// Register fair price, basis, spot delta, and curve DV01 for equity futures.
pub(crate) fn register_equity_future_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: finstack_quant_valuations_core::pricer::InstrumentType::EquityFuture,
        metrics: [
            (FuturesPrice, FuturesPrice),
            (Basis, Basis),
            (Delta, Delta),
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}
