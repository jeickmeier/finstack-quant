//! WASM bindings for `finstack-quant-analytics`.
//!
//! The only entry point exposed to JS is the [`Performance`] class. Every
//! analytic — returns/risk metrics, periodic returns, benchmark comparisons,
//! basic factor models — is reachable as a `Performance` method.

mod performance;
mod support;

pub use performance::WasmPerformance;
