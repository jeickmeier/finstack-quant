//! Stateful `Performance` orchestrator over the analytics building blocks.
//!
//! Implementation is split across sibling files (`scalar`, `rolling`,
//! `benchmark`, `aggregation`); each adds `impl Performance` methods.
//! Public re-exports happen from `lib.rs`.

mod aggregation;
mod benchmark;
pub(crate) mod engine;
pub(crate) mod panel;
mod rolling;
pub(crate) mod scalar;
mod serialization;

pub(crate) use engine::{LookbackReturns, Performance};
use engine::{TickerSpan, invalid_return_series, lookback_starts};
#[cfg(test)]
pub(crate) use engine::{prices_panel, returns_panel};
