//! WASM bindings for the `finstack-quant-covenants` crate.
//!
//! This module holds the JSON wire surface (validators, `evaluateEngine`, the
//! `*Json` templates). The typed surface lives in the submodules: the
//! `CovenantEngine` handle, forecasting, and free functions over wire-shaped
//! covenant definitions.

pub mod engine;
pub mod forecast;
mod operations;
pub mod spec;

pub use operations::*;
