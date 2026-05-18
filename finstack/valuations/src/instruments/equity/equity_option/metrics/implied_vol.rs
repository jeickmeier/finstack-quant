//! Implied volatility calculator for equity options.
//!
//! Solves for σ such that model price(σ) equals a provided market price. The
//! market price can be supplied via instrument attributes:
//! - `market_price`: numeric value as string
//! - `market_price_id`: id of a scalar in `MarketContext`

use crate::instruments::equity::equity_option::EquityOption;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::Result;

pub(crate) struct ImpliedVolCalculator;

impl MetricCalculator for ImpliedVolCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &EquityOption = context.instrument_as()?;

        // Market price.
        //
        // A malformed `market_price` string must surface as an error: silently
        // defaulting to 0.0 would solve the implied volatility against a zero
        // target price, producing a meaningless (typically degenerate) IV with
        // no indication that the input was bad.
        let market_price: f64 = if let Some(p) = option.attributes.get_meta("market_price") {
            p.parse().map_err(|e| {
                finstack_core::Error::Validation(format!(
                    "EquityOption '{}': attribute 'market_price' = {:?} is not a valid \
                     number ({}); cannot solve implied volatility",
                    option.id, p, e
                ))
            })?
        } else if let Some(price_id) = option.attributes.get_meta("market_price_id") {
            let ms = context.curves.get_price(price_id).map_err(|e| {
                finstack_core::Error::Validation(format!(
                    "EquityOption '{}': failed to fetch 'market_price_id' = '{}' from \
                     the market context ({}); cannot solve implied volatility",
                    option.id, price_id, e
                ))
            })?;
            match ms {
                finstack_core::market_data::scalars::MarketScalar::Unitless(val) => *val,
                finstack_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
            }
        } else {
            return Err(finstack_core::Error::Validation(format!(
                "EquityOption '{}': implied volatility requires a market price — set \
                 either the 'market_price' or 'market_price_id' attribute",
                option.id
            )));
        };

        option.implied_vol(&context.curves, context.as_of, market_price)
    }
}
