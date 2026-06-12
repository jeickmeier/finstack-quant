//! Python bindings for the finstack library.
//!
//! All business logic stays in Rust core crates. These bindings handle only:
//! - Type conversion (Python <-> Rust)
//! - Wrapper construction and accessor methods
//! - Error mapping to Python exceptions

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::float_cmp,
    )
)]

use pyo3::prelude::*;

mod bindings;
mod errors;

/// Python bindings for the finstack library.
#[pymodule]
fn finstack(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    bindings::register_root(py, m)
}
