//! WASM bindings for the `finstack-quant-scenarios` crate.
//!
//! Exposes scenario specification parsing, validation, composition,
//! and built-in template access via structured JavaScript values.

mod engine;
pub mod kinds;
pub mod operation_spec;
pub mod results;
mod spec;
#[cfg(test)]
mod tests;

pub use engine::*;
pub use spec::*;
