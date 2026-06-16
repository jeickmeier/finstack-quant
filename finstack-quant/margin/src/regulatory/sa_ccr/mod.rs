//! SA-CCR (Standardized Approach for Counterparty Credit Risk).
//!
//! Implements BCBS 279 for computing Exposure at Default (EAD)
//! on derivative portfolios.

pub mod add_on;
pub mod engine;
pub mod maturity_factor;
pub mod params;
pub mod pfe;
pub mod replacement_cost;
pub mod types;

pub use engine::{SaCcrEngine, SaCcrEngineBuilder};
pub use types::{EadResult, SaCcrAssetClass, SaCcrNettingSetConfig, SaCcrOptionType, SaCcrTrade};
