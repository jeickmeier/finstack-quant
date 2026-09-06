//! Equity spot metric calculators and registry registration.
//!
//! Computes price per share, share exposure, dividend yield, forward price,
//! and delta using the shared metrics framework.

use crate::instruments::equity::Equity;
use crate::metrics::{MetricCalculator, MetricContext, MetricId, MetricRegistry};
use finstack_quant_core::Result;

/// Computes the price per share for an `Equity`.
pub(crate) struct PricePerShareCalculator;

impl MetricCalculator for PricePerShareCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let equity: &Equity = context.instrument_as()?;
        let m = equity.price_per_share(&context.curves, context.as_of)?;
        Ok(m.amount())
    }
}

/// Computes the effective number of shares for an `Equity`.
pub(crate) struct SharesCalculator;

impl MetricCalculator for SharesCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let equity: &Equity = context.instrument_as()?;
        Ok(equity.effective_shares())
    }
}

/// Computes the dividend yield using `{ticker}-DIVYIELD` if present, or 0.0.
pub(crate) struct DividendYieldCalculator;

impl MetricCalculator for DividendYieldCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let equity: &Equity = context.instrument_as()?;
        equity.dividend_yield(&context.curves)
    }
}

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

/// Delta calculator for equity spot.
pub(crate) struct DeltaCalculator;

impl MetricCalculator for DeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let equity: &Equity = context.instrument_as()?;
        let delta = equity.shares.unwrap_or(1.0);

        context.computed.insert(
            MetricId::composite(&MetricId::Delta, &[equity.ticker.as_str()]),
            delta,
        );

        Ok(delta)
    }
}

/// Register all Equity metrics with the registry
pub(crate) fn register_equity_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), crate::metrics::MetricRegistryError> {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Equity,
        metrics: [
            (EquityPricePerShare, PricePerShareCalculator),
            (EquityShares, SharesCalculator),
            (EquityDividendYield, DividendYieldCalculator),
            (EquityForwardPrice, ForwardPricePerShareCalculator),
            (Delta, DeltaCalculator),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::Equity,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::Equity,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}
