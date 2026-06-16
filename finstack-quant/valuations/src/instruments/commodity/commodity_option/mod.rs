//! Commodity option instrument module.
//!
//! Supports European options priced with Black-76 and American options priced
//! via a binomial tree on spot (or futures-implied spot).

/// Metrics for commodity options.
pub(crate) mod metrics;
/// Pricer for commodity options.
pub(crate) mod pricer;
pub(crate) mod traits;
mod types;

pub use pricer::CommodityOptionMcPricer;
pub use types::CommodityOption;
pub use types::{CommodityMcParams, CommodityPricingModel};
