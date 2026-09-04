//! Tree-based pricing engine for bonds with embedded options and OAS calculations.
//!
//! This module provides option rollback and option-adjusted spread (OAS)
//! calculations for two explicitly selected model families:
//! - **`tree`**: Rates-only pricing. An attached credit curve does not switch
//!   the model family or affect value.
//! - **`rates_credit`**: Joint rates-credit pricing. The bond must name a
//!   hazard curve; positive rate or hazard volatility uses Monte Carlo and
//!   reports sampling diagnostics.
//!
//! # Pricing Models
//!
//! ## Short-Rate Tree
//! The tree models interest-rate evolution and applies call, put, and
//! deterministic return-floor constraints by backward induction.
//!
//! ## Rates+Credit Tree
//! Selected only by `ModelKey::RatesCredit`. It models both interest-rate and
//! credit-risk evolution, including default events and recovery payments.
//!
//! # See Also
//!
//! - `TreePricer` for OAS calculation
//! - tree-valuator implementation details in this module
//! - `TreePricerConfig` for configuration options

mod bond_valuator;
mod config;
pub(crate) mod lsmc;
#[cfg(test)]
mod tests;
mod tree_pricer;

pub use bond_valuator::BondValuator;
pub use config::{bond_tree_config, TreeModelChoice, TreePricerConfig};
pub(crate) use tree_pricer::TreePriceOutcome;
pub use tree_pricer::TreePricer;
