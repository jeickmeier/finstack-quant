//! Python bindings for the finstack-quant library.
//!
//! All business logic stays in Rust core crates. These bindings handle only:
//! - Type conversion (Python <-> Rust)
//! - Wrapper construction and accessor methods
//! - Error mapping to Python exceptions

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
// PyO3 owners take their docs from the `.pyi` stubs (`bindings::pydoc`), which
// are NumPy-style Python docstrings, not Markdown: a doctest's `>>>` prompt and
// its output line read as a block quote with a lazy continuation.
#![allow(clippy::doc_lazy_continuation)]
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

/// Python bindings for the finstack-quant library.
#[pymodule]
fn finstack_quant(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    bindings::register_root(py, m)
}
