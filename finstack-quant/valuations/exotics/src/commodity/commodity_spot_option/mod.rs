//! Commodity option instrument module.
//!
//! Supports European options priced with Black-76 and American options priced
//! via a binomial tree on observed spot with forward-calibrated carry.

/// Metrics for commodity options.
pub(crate) mod metrics;
/// Pricer for commodity options.
pub(crate) mod pricer;
pub(crate) mod types;

pub use pricer::CommoditySpotOptionMcPricer;
pub use types::CommodityMcParams;
pub use types::{CommoditySpotOption, CommoditySpotOptionExercise};
