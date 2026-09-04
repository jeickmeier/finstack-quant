//! Common pricing patterns and shared infrastructure.
//!
//! This module provides generic pricer implementations and shared pricing utilities
//! to eliminate duplication across instrument pricing modules.
//!
//! ## Sub-modules
//!
//! - [`generic`]: Generic pricers for instruments implementing the Instrument trait
//! - [`trs`]: Total Return Swap pricing engine
//! - [`swap_legs`]: Shared floating/fixed leg pricing for swaps
//! - [`time`]: Shared time-mapping and discount factor helpers for consistent curve usage

pub(crate) mod floating_reset_descriptors;
mod generic;
pub(crate) mod overnight;
pub mod overnight_conventions;
pub(crate) mod rates_credit;
pub mod swap_legs;
pub mod time;
mod trs;
pub(crate) mod variance_observations;
pub mod variance_replication;

#[doc(hidden)]
pub use generic::GenericInstrumentPricer;

pub use trs::{PeriodReturnInputs, TotalReturnLegParams, TrsEngine, TrsReturnModel};
