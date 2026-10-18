//! Common pricing patterns and shared infrastructure.
//!
//! This module provides generic pricer implementations and shared pricing utilities
//! to eliminate duplication across instrument pricing modules.
//!
//! ## Sub-modules
//!
//! - `generic`: Generic pricers for instruments implementing the Instrument trait
//! - `trs`: Total Return Swap pricing engine
//! - [`swap_legs`]: Shared floating/fixed leg pricing for swaps
//! - [`hw1f_params`]: Hull-White 1F κ/σ resolution from pricing overrides or calibrated market scalars

pub mod coupon_period;
pub mod floating_reset_descriptors;
mod generic;
pub mod hw1f_params;
pub mod overnight;
pub mod overnight_conventions;
pub mod rates_credit;
pub mod swap_legs;
mod trs;
pub mod variance_observations;

#[doc(hidden)]
pub use generic::GenericInstrumentPricer;

pub use trs::{PeriodReturnInputs, TotalReturnLegParams, TrsEngine, TrsReturnModel};
