//! Implied volatility calculator for equity options.
//!
//! Solves for σ such that model price(σ) equals the observed option premium in
//! `instrument_pricing_overrides.market_quotes.quoted_premium`.

use crate::equity_option::EquityOption;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

pub(crate) struct ImpliedVolCalculator;

impl MetricCalculator for ImpliedVolCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let option: &EquityOption = context.instrument_as()?;
        let target_price = option
            .instrument_pricing_overrides
            .market_quotes
            .required_quoted_premium()?;
        option.implied_vol(&context.curves, context.as_of, target_price)
    }
}
