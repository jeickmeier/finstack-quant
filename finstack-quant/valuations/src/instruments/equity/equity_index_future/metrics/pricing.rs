//! Pricing diagnostics for equity index futures.

use crate::instruments::equity::equity_index_future::EquityIndexFuture;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::Result;

/// Fair or quoted futures price.
pub(crate) struct FuturesPriceCalculator;

impl MetricCalculator for FuturesPriceCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let future: &EquityIndexFuture = context.instrument_as()?;
        if let Some(quoted_price) = future.quoted_price {
            Ok(quoted_price)
        } else {
            future.fair_forward(&context.curves, context.as_of)
        }
    }
}

/// Futures basis, defined as futures price minus spot index level.
pub(crate) struct BasisCalculator;

impl MetricCalculator for BasisCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let future: &EquityIndexFuture = context.instrument_as()?;
        let futures_price = if let Some(quoted_price) = future.quoted_price {
            quoted_price
        } else {
            future.fair_forward(&context.curves, context.as_of)?
        };
        let spot = match context.curves.get_price(&future.spot_id)? {
            MarketScalar::Unitless(value) => *value,
            MarketScalar::Price(money) => money.amount(),
        };
        Ok(futures_price - spot)
    }
}
