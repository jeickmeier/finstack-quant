//! Python bindings for the roll-forward template.
//!
//! Wraps [`finstack_quant_statements_analytics::templates::roll_forward::add_roll_forward`].
//!
//! Mirrors the JSON-in / JSON-out shape used for vintage: rebuild a Rust
//! `ModelBuilder` from a serialized [`FinancialModelSpec`], apply the
//! template, then serialize the resulting spec back out.

use crate::bindings::extract::extract_model_ref;
use crate::errors::display_to_py;
use finstack_quant_statements::types::FinancialModelSpec;
use finstack_quant_statements_analytics::templates::roll_forward as rust_roll_forward;
use pyo3::prelude::*;

use super::templates_common::{finalize_spec, rebuild_builder};

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
/// str
///     JSON-serialized ``FinancialModelSpec`` with the roll-forward nodes added.
#[pyfunction]
fn add_roll_forward(
    model: &Bound<'_, PyAny>,
    name: &str,
    increases: Vec<String>,
    decreases: Vec<String>,
) -> PyResult<String> {
    let spec = extract_model_ref(model)?.into_owned();
    let increases_refs: Vec<&str> = increases.iter().map(String::as_str).collect();
    let decreases_refs: Vec<&str> = decreases.iter().map(String::as_str).collect();
    let updated = apply_roll_forward(spec, name, &increases_refs, &decreases_refs)?;
    serde_json::to_string(&updated).map_err(display_to_py)
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
/// str
///     JSON-serialized ``FinancialModelSpec`` with the roll-forward nodes added.
#[pyfunction]
fn add_roll_forward_with_opening(
    model: &Bound<'_, PyAny>,
    name: &str,
    increases: Vec<String>,
    decreases: Vec<String>,
    opening: f64,
) -> PyResult<String> {
    let spec = extract_model_ref(model)?.into_owned();
    let increases_refs: Vec<&str> = increases.iter().map(String::as_str).collect();
    let decreases_refs: Vec<&str> = decreases.iter().map(String::as_str).collect();
    let (builder, meta, capital_structure) = rebuild_builder(spec)?;
    let builder = rust_roll_forward::add_roll_forward_with_opening(
        builder,
        name,
        &increases_refs,
        &decreases_refs,
        opening,
    )
    .map_err(display_to_py)?;
    let updated = finalize_spec(builder, meta, capital_structure)?;
    serde_json::to_string(&updated).map_err(display_to_py)
}

fn apply_roll_forward(
    spec: FinancialModelSpec,
    name: &str,
    increases: &[&str],
    decreases: &[&str],
) -> PyResult<FinancialModelSpec> {
    let (builder, meta, capital_structure) = rebuild_builder(spec)?;
    let builder = rust_roll_forward::add_roll_forward(builder, name, increases, decreases)
        .map_err(display_to_py)?;
    finalize_spec(builder, meta, capital_structure)
}

/// Register the roll-forward template bindings on the parent module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(add_roll_forward, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(add_roll_forward_with_opening, m)?)?;
    Ok(())
}
