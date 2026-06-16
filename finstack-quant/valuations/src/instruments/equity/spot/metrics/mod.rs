//! Equity metrics module.
//!
//! Provides metric calculators specific to `Equity`, split into focused files.
//! The calculators compose with the shared metrics framework and are registered
//! via `register_equity_metrics`.
//!
//! Exposed metrics:
//! - price per share
//! - shares
//! - dividend yield
//! - forward price

mod delta;
mod dividend_yield;
mod forward_price;
mod price_per_share;
mod shares;

use crate::metrics::MetricRegistry;

/// Register all Equity metrics with the registry
pub(crate) fn register_equity_metrics(registry: &mut MetricRegistry) {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Equity,
        metrics: [
            (EquityPricePerShare, price_per_share::PricePerShareCalculator),
            (EquityShares, shares::SharesCalculator),
            (EquityDividendYield, dividend_yield::DividendYieldCalculator),
            (EquityForwardPrice, forward_price::ForwardPricePerShareCalculator),
            (Delta, delta::DeltaCalculator),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::Equity,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::Equity,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
}
