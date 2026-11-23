//! Typed instrument classes for the `finstack_quant.valuations.instruments`
//! namespace.
//!
//! Thin wrappers over the canonical Rust structs
//! `finstack_quant_valuations::instruments::fixed_income::bond::Bond` and
//! `finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan`. Construction and
//! validation stay in Rust; the wrappers only convert to and from the canonical
//! `finstack_quant.instrument/1` envelope accepted by the JSON loader, expose
//! one getter per public Rust field, and route `price` / `metric` through the
//! same pricer entry points as `price_instrument`.

mod bond;
mod term_loan;

pub use bond::{PyBond, PyBondBuilder};
use pyo3::prelude::*;
pub use term_loan::{PyTermLoan, PyTermLoanBuilder};

/// Register the typed instrument classes on the instruments submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyBond>()?;
    m.add_class::<PyBondBuilder>()?;
    m.add_class::<PyTermLoan>()?;
    m.add_class::<PyTermLoanBuilder>()?;
    Ok(())
}

/// Names this module contributes to `finstack_quant.valuations.instruments.__all__`.
///
/// Extend this list (sorted) when adding a class or function here; `mod.rs`
/// merges every submodule list so registration stays in one place per file.
pub(crate) const EXPORTS: &[&str] = &["Bond", "BondBuilder", "TermLoan", "TermLoanBuilder"];
