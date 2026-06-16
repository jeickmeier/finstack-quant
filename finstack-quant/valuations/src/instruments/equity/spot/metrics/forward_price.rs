//! Equity metric: forward price per share.
//!
//! Uses the standard approximation F(t) = S0 * exp((r - q) * t).

use crate::instruments::equity::Equity;
use crate::metrics::{MetricCalculator, MetricContext};

/// Computes the forward price per share over a horizon in years.
///
/// Horizon resolution order:
/// 1) Try `MarketContext::price("{ticker}-FWD_T")` as a unitless scalar (years)
/// 2) Fallback to 0.0 (spot)
pub(crate) struct ForwardPricePerShareCalculator;

impl MetricCalculator for ForwardPricePerShareCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let equity: &Equity = context.instrument_as()?;
        let key = format!("{}-FWD_T", equity.ticker);
        let t = context
            .curves
            .get_price(&key)
            .map(|s| match s {
                finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
                finstack_quant_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
            })
            .unwrap_or(0.0);
        let money = equity.forward_price_per_share(&context.curves, context.as_of, t)?;
        Ok(money.amount())
    }
}
