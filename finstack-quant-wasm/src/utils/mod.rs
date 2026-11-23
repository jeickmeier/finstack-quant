//! Shared conversion helpers for WASM bindings.
//!
//! Utilities for error mapping, JSON serialization, and decimal conversion
//! used across all domain binding modules.

pub mod date;
pub mod input;
pub mod wire;

pub use date::{date_to_iso, parse_iso_date, parse_iso_dates};
mod error;
mod serialize;
#[cfg(test)]
mod tests;

pub use error::{
    IntoJsError, contract_to_js_err, materialization_to_js_error, structured_js_error, to_js_err,
};
pub use serialize::to_js_value;
pub(crate) use serialize::{
    restore_non_finite_rows, to_js_rows_numeric, to_js_table_numeric, to_js_value_numeric,
    to_js_value_with_bigints,
};
