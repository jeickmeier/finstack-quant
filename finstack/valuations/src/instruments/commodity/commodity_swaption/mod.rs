//! Commodity swaption instrument module.
//!
//! This module provides the [`CommoditySwaption`] instrument for modeling
//! options on fixed-for-floating commodity price swap contracts.
//!
//! # Overview
//!
//! A commodity swaption gives the holder the right to enter a commodity swap
//! at a predetermined fixed price. It is priced using the Black-76 model
//! applied to the forward swap rate.
//!
//! # Pricing
//!
//! ```text
//! Call: annuity * [F * N(d1) - K * N(d2)]
//! Put:  annuity * [K * N(-d2) - F * N(-d1)]
//! ```
//!
//! where F is the forward swap rate and annuity captures discounting.
//!
//! # Examples
//!
//! ```rust
//! use finstack_valuations::instruments::commodity::commodity_swaption::CommoditySwaption;
//! use finstack_core::currency::Currency;
//!
//! let swaption = CommoditySwaption::example();
//! assert_eq!(swaption.underlying.ticker, "NG");
//! ```

/// Metrics submodule for commodity swaption risk measures.
pub(crate) mod metrics;
mod types;

pub use types::CommoditySwaption;
