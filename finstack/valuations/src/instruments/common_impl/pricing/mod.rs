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

mod generic;
pub mod swap_legs;
pub mod time;
mod trs;
pub mod variance_replication;

// Re-export generic pricer types
#[doc(hidden)]
pub use generic::GenericInstrumentPricer;

// Re-export TRS types
pub use trs::{TotalReturnLegParams, TrsEngine, TrsReturnModel};
