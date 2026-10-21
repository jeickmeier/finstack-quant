//! Common pricing patterns and shared infrastructure.
//!
//! This module provides generic pricer implementations and shared pricing utilities
//! to eliminate duplication across instrument pricing modules.
//!
//! ## Sub-modules
//!
//! - `generic`: Generic pricers for instruments implementing the Instrument trait
//! - `trs`: Total Return Swap pricing engine

pub mod coupon_period;
pub mod floating_reset_descriptors;
mod generic;
pub mod overnight;
pub mod rates_credit;
pub(crate) mod trs;
pub mod variance_observations;

#[doc(hidden)]
pub use generic::GenericInstrumentPricer;

pub use trs::{FinancingLegFlows, PeriodReturnInputs};
