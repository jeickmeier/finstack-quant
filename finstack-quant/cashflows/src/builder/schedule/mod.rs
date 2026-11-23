//! Schedule generation from the builder state.
//!
//! Provides the canonical `CashFlowSchedule` type and helpers for sorting and
//! deriving schedule metadata. Downstream pricing/risk code consumes this shape.

pub(crate) mod merge;
mod ordering;
#[cfg(test)]
mod tests;
pub(crate) mod types;

pub(crate) use types::{CashFlowMeta, CashFlowSchedule, CashflowRepresentation};
