//! Implied volatility metric for `CdsOption`.
//!
//! Computes the Black-on-spreads implied volatility that matches the observed
//! option premium in `instrument_pricing_overrides.market_quotes.quoted_premium`
//! using the CDS option pricer and core math solvers (HybridSolver).

use crate::instruments::credit_derivatives::cds_option::CdsOption;

/// Implied volatility metric for credit options on CDS spreads.
pub(crate) struct ImpliedVolCalculator;

impl crate::metrics::MetricCalculator for ImpliedVolCalculator {
    fn calculate(
        &self,
        context: &mut crate::metrics::MetricContext,
    ) -> finstack_quant_core::Result<f64> {
        let option: &CdsOption = context.instrument_as()?;
        let target_price = option
            .instrument_pricing_overrides
            .market_quotes
            .required_quoted_premium()?;
        option.implied_vol(&context.curves, context.as_of, target_price, None)
    }
}
