//! Python bindings for the roll-forward template.
//!
//! Wraps [`finstack_statements_analytics::templates::roll_forward::add_roll_forward`].
//!
//! Mirrors the JSON-in / JSON-out shape used for vintage: rebuild a Rust
//! `ModelBuilder` from a serialized [`FinancialModelSpec`], apply the
//! template, then serialize the resulting spec back out.

use crate::bindings::extract::extract_model_ref;
use crate::errors::display_to_py;
use finstack_statements::builder::ModelBuilder;
use finstack_statements::types::FinancialModelSpec;
use finstack_statements_analytics::templates::roll_forward as rust_roll_forward;
use pyo3::prelude::*;

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

fn apply_roll_forward(
    spec: FinancialModelSpec,
    name: &str,
    increases: &[&str],
    decreases: &[&str],
) -> PyResult<FinancialModelSpec> {
    let meta = spec.meta.clone();
    let capital_structure = spec.capital_structure.clone();
    let id = spec.id.clone();
    let periods = spec.periods.clone();
    let nodes = spec.nodes;

    let mut builder = ModelBuilder::new(id)
        .periods_explicit(periods)
        .map_err(display_to_py)?;
    for (node_id, node_spec) in nodes {
        builder.insert_node(node_id, node_spec);
    }
    let builder = rust_roll_forward::add_roll_forward(builder, name, increases, decreases)
        .map_err(display_to_py)?;
    let mut new_spec = builder.build().map_err(display_to_py)?;
    new_spec.meta = meta;
    new_spec.capital_structure = capital_structure;
    Ok(new_spec)
}

/// Register the roll-forward template binding on the parent module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(add_roll_forward, m)?)?;
    Ok(())
}
