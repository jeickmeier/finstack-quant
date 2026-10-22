//! Exotic and path-dependent options.
//!
//! The family lives in `finstack-quant-valuations-exotics` and is re-exported
//! here unchanged. [`Basket`](crate::instruments::exotics::basket::Basket) stays in this crate: its legs are
//! [`InstrumentJson`](crate::instruments::json_loader::InstrumentJson) values of any
//! family, which is the closed world by definition, so it registers with
//! [`STANDARD`](crate::pricer::STANDARD) rather than with
//! [`EXOTICS`](finstack_quant_valuations_exotics::EXOTICS).

pub use finstack_quant_valuations_exotics::exotics::*;

/// Basket module - Multi-underlying basket instruments.
pub mod basket;
