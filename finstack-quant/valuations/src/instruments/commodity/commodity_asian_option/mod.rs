//! Commodity Asian option instrument module.
//!
//! Asian options on commodity forward prices with analytical pricing via
//! Turnbull-Wakeman (arithmetic) and Kemna-Vorst (geometric) methods.
//! Uses forward prices from a price curve for each fixing date.

/// Pricer for commodity Asian options.
pub(crate) mod pricer;
mod types;

/// Metrics for commodity Asian options.
pub(crate) mod metrics;

pub use pricer::CommodityAsianOptionAnalyticalPricer;
pub use types::CommodityAsianOption;
