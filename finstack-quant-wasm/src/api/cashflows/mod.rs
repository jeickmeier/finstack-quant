//! WASM bindings for the `finstack-quant-cashflows` crate.
//!
//! This module holds the JSON-first bridge (`*Json` functions and the
//! schedule-JSON analytics) and the prepayment/default rate conversions. The
//! typed surface lives in the submodules: the `CashFlowSchedule` /
//! `CashFlowBuilder` / `AccrualIndex` handles, and free functions over
//! wire-shaped spec and aggregation values.

pub mod accrual;
pub mod aggregation;
pub mod fixings;
mod operations;
pub mod schedule;
pub mod specs;

pub use operations::*;
