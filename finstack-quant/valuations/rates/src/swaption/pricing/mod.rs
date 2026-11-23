//! Swaption pricing engines (European and Bermudan).
//!
//! Owns every pricing engine registered for `Swaption` and `BermudanSwaption`:
//!
//! - `black`: European Black-76 and Bachelier normal pricers
//! - `hull_white`: European Hull-White 1-factor tree pricer
//! - `bermudan`: Bermudan orchestration (`BermudanSwaptionPricer`, `PreparedHullWhiteModel`)
//! - `lmm`: Bermudan LIBOR market model adapter (`BermudanSwaptionLmmPricer`)
//! - `tree_valuator`: Hull-White tree backward induction (`BermudanSwaptionTreeValuator`)
//! - `monte_carlo_lsmc`: Hull-White Longstaff-Schwartz Monte Carlo
//! - `lmm_bermudan`: LIBOR market model Bermudan kernel
//! - `hw_cashflows`: Hull-White exercise cashflow projection
//! - `swap_rate_utils`: shared Hull-White zero-coupon bond pricing

pub(crate) mod bermudan;
pub(crate) mod black;
mod hull_white;
pub(crate) mod hw_cashflows;
pub(crate) mod lmm;
mod lmm_bermudan;
mod monte_carlo_lsmc;
mod swap_rate_utils;
pub(crate) mod tree_valuator;

#[cfg(test)]
#[path = "tests/numeraire.rs"]
mod numeraire_tests;

pub(crate) use hull_white::SwaptionHullWhitePricer;
pub(crate) use tree_valuator::BermudanSwaptionTreeValuator;
