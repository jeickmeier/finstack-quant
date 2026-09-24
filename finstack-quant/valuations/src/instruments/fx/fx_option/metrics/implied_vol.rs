//! Implied volatility metric for FX options.
//!
//! Solves for σ such that model PV(σ) equals the observed option premium in
//! `instrument_pricing_overrides.market_quotes.quoted_premium`, using the
//! configured pricer (Hybrid solver under the hood) with log-σ
//! parameterization.

use crate::instruments::fx::fx_option::FxOption;

/// Implied volatility metric for FX options.
pub(crate) struct ImpliedVolCalculator;

impl crate::metrics::MetricCalculator for ImpliedVolCalculator {
    fn calculate(
        &self,
        context: &mut crate::metrics::MetricContext,
    ) -> finstack_quant_core::Result<f64> {
        let option: &FxOption = context.instrument_as()?;
        let target_price = option
            .instrument_pricing_overrides
            .market_quotes
            .required_quoted_premium()?;
        option.implied_vol(&context.curves, context.as_of, target_price)
    }
}
