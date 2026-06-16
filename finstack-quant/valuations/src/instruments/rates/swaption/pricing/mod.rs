//! Bermudan swaption pricing engines.
//!
//! This module provides specialized pricing engines for Bermudan swaptions
//! using tree-based and Monte Carlo methods.
//!
//! # Available Engines
//!
//! - [`tree_valuator`] - Hull-White tree-based pricing (industry standard)
//!
//! # Example
//!
//! ```text
//! use finstack_quant_valuations::instruments::rates::swaption::BermudanSwaption;
//! use finstack_quant_valuations::instruments::rates::swaption::pricing::BermudanSwaptionTreeValuator;
//!
//! let swaption = BermudanSwaption::example();
//! // Use with HullWhiteTree for backward induction pricing
//! ```

pub mod lmm_bermudan;
pub mod monte_carlo_lsmc;
pub mod monte_carlo_payoff;
pub mod swap_rate_utils;
pub(crate) mod tree_valuator;

#[cfg(test)]
mod numeraire_tests;

pub use tree_valuator::BermudanSwaptionTreeValuator;
