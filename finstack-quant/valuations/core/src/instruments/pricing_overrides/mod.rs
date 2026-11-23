//! Pricing overrides for market-quoted instruments.

mod instrument_overrides;
mod market_quotes;
mod metric_overrides;
mod model_config;

#[cfg(test)]
mod tests;

pub use market_quotes::MarketQuoteOverrides;
pub use metric_overrides::{
    BondRiskBasis, BumpConfig, MetricPricingOverrides, ScenarioPricingOverrides, ThetaDayBasis,
};
pub use model_config::{ModelConfig, ShortRateTreeModel, VolSurfaceExtrapolation};

pub use instrument_overrides::InstrumentPricingOverrides;
use instrument_overrides::check_finite_fields;
