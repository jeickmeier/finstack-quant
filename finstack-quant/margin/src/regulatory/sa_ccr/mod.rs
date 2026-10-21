//! SA-CCR (Standardized Approach for Counterparty Credit Risk).
//!
//! Implements BCBS 279 for computing Exposure at Default (EAD)
//! on derivative portfolios.

mod add_on;
pub(crate) mod engine;
mod maturity_factor;
mod pfe;
mod replacement_cost;
pub(crate) mod types;

pub use engine::{saccr_ead, SaCcrEngine};
pub use types::{
    EadResult, SaCcrAssetClass, SaCcrNettingSetConfig, SaCcrOptionType, SaCcrSupervisoryCategory,
    SaCcrTrade,
};
