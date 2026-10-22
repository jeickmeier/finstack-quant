//! Bermudan swaption pricing engines.
//!
//! This module provides specialized pricing engines for Bermudan swaptions
//! using tree-based and Monte Carlo methods.
//!
//! # Engines
//!
//! - `tree_valuator`: Hull-White tree backward induction
//!   (`BermudanSwaptionTreeValuator`, re-exported at `swaption::`)
//! - `monte_carlo_lsmc`: Hull-White Longstaff-Schwartz Monte Carlo
//! - `lmm_bermudan`: LIBOR market model Bermudan pricing
//! - `swap_rate_utils`: the Hull-White zero-coupon bond price they share

pub mod lmm_bermudan;
pub mod monte_carlo_lsmc;
pub mod swap_rate_utils;
pub(crate) mod tree_valuator;

#[cfg(test)]
mod numeraire_tests;

pub use tree_valuator::BermudanSwaptionTreeValuator;

pub(crate) mod hw_cashflows;
