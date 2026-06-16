//! Commodity spread option instrument module.
//!
//! Spread options on two correlated commodity prices, priced via Kirk's
//! approximation (1995). The payoff is max(S1 - S2 - K, 0) for calls.

/// Metrics for commodity spread options.
pub(crate) mod metrics;
/// Pricer for commodity spread options.
pub(crate) mod pricer;
mod types;

pub use types::CommoditySpreadOption;
