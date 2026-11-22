//! Real estate asset valuation instruments.
//!
//! Supports DCF and direct capitalization approaches for single-asset valuation.

/// Real estate asset metric calculators and shared helpers.
#[doc(hidden)]
pub mod metrics;
/// Real estate asset pricer helpers.
#[doc(hidden)]
pub mod pricer;
mod types;

pub use types::{
    RealEstateAsset, RealEstateAssetBuilder, RealEstatePropertyType, RealEstateValuationMethod,
};
