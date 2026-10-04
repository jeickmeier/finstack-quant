//! Python bindings for the roll-forward template.
//!
//! Wraps [`finstack_quant_statements_analytics::templates::roll_forward::add_roll_forward`].
//!
//! Reconstructs the canonical Rust builder, applies the selected template,
//! and returns a typed model specification.

use crate::bindings::statements::types::PyFinancialModelSpec;
use finstack_quant_statements_analytics::templates::roll_forward as rust_roll_forward;
use pyo3::prelude::*;

use super::templates_common::{extract_builder, finish_builder};

/// Apply the roll-forward template to a model spec.
///
/// Generates a beginning-balance node and an ending-balance node for ``name``:
///
/// - ``{name}_beg`` = ``lag({name}_end, 1)`` (zeroed in the first period)
/// - ``{name}_end`` = ``{name}_beg + sum(increases) - sum(decreases)``
///
/// Parameters
/// ----------
/// model : FinancialModelSpec | str
///     Existing model spec (typed object or JSON).
/// name : str
///     Base name for the roll-forward (e.g. ``"inventory"``).
/// increases : list[str]
///     Node ids that add to the balance.
/// decreases : list[str]
///     Node ids that subtract from the balance.
///
/// Returns
/// -------
/// FinancialModelSpec
///     Typed model specification with the roll-forward nodes added.
#[pyfunction]
fn add_roll_forward(
    model: &Bound<'_, PyAny>,
    name: &str,
    increases: Vec<String>,
    decreases: Vec<String>,
) -> PyResult<PyFinancialModelSpec> {
    let increases_refs: Vec<&str> = increases.iter().map(String::as_str).collect();
    let decreases_refs: Vec<&str> = decreases.iter().map(String::as_str).collect();
    let builder = rust_roll_forward::add_roll_forward(
        extract_builder(model)?,
        name,
        &increases_refs,
        &decreases_refs,
    );
    finish_builder(builder)
}

/// Apply the roll-forward template with an explicit opening balance.
///
/// Same as ``add_roll_forward`` except the first period's beginning balance
/// is ``opening`` instead of zero.
///
/// Parameters
/// ----------
/// model : FinancialModelSpec | str
///     Existing model spec (typed object or JSON).
/// name : str
///     Base name for the roll-forward (e.g. ``"inventory"``).
/// increases : list[str]
///     Node ids that add to the balance.
/// decreases : list[str]
///     Node ids that subtract from the balance.
/// opening : float
///     Opening balance for the first period.
///
/// Returns
/// -------
/// FinancialModelSpec
///     Typed model specification with the roll-forward nodes added.
#[pyfunction]
fn add_roll_forward_with_opening(
    model: &Bound<'_, PyAny>,
    name: &str,
    increases: Vec<String>,
    decreases: Vec<String>,
    opening: f64,
) -> PyResult<PyFinancialModelSpec> {
    let increases_refs: Vec<&str> = increases.iter().map(String::as_str).collect();
    let decreases_refs: Vec<&str> = decreases.iter().map(String::as_str).collect();
    let builder = rust_roll_forward::add_roll_forward_with_opening(
        extract_builder(model)?,
        name,
        &increases_refs,
        &decreases_refs,
        opening,
    );
    finish_builder(builder)
}

/// Register the roll-forward template bindings on the parent module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(add_roll_forward, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(add_roll_forward_with_opening, m)?)?;
    Ok(())
}
