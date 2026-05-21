//! Commodity swap instrument module.
//!
//! This module provides the [`CommoditySwap`] instrument for modeling
//! fixed-for-floating commodity price exchange contracts.
//!
//! # Overview
//!
//! Commodity swaps are derivatives where one party pays a fixed price per unit
//! of a commodity while the other pays a floating price based on an index or
//! average of spot prices.
//!
//! # Pricing
//!
//! The swap is valued as the difference between the floating and fixed legs:
//! ```text
//! NPV (for payer of fixed) = Floating Leg PV - Fixed Leg PV
//! ```
//!
//! # Examples
//!
//! ```rust
//! use finstack_valuations::instruments::commodity::commodity_swap::CommoditySwap;
//! use finstack_core::currency::Currency;
//!
//! // Create a natural gas swap
//! let swap = CommoditySwap::example();
//! assert_eq!(swap.underlying.ticker, "NG");
//! ```

mod types;

pub use types::CommoditySwap;

/// Metrics submodule for commodity swap risk measures.
pub(crate) mod metrics;
