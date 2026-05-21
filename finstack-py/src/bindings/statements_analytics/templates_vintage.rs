//! Python bindings for the vintage / cohort buildup template.
//!
//! Wraps [`finstack_statements_analytics::templates::vintage`].
//!
//! The Rust trait method takes a `ModelBuilder<Ready>` and threads it through
//! `add_vintage_buildup`. Because the Python `ModelBuilder` private state is
//! not accessible from this module, we expose a JSON-in / JSON-out form that
//! rebuilds the Rust builder from a serialized [`FinancialModelSpec`], applies
//! the template, and serializes the resulting spec back out.

use crate::bindings::extract::extract_model_ref;
use crate::errors::display_to_py;
use finstack_statements::builder::ModelBuilder;
use finstack_statements::types::FinancialModelSpec;
use finstack_statements_analytics::templates::vintage as rust_vintage;
use pyo3::prelude::*;

/// Apply the vintage (cohort) buildup template to a model spec.
///
/// Generates a calculated node whose value is the convolution of
/// ``new_volume_node`` with ``decay_curve``. The first decay-curve element is
/// the inception multiplier, the second is for the next period, and so on.
///
/// Parameters
/// ----------
/// model : FinancialModelSpec | str
///     Existing model spec (typed object or JSON).
/// name : str
///     Output node id (e.g. ``"revenue"``).
/// new_volume_node : str
///     Node id supplying the new-volume series.
/// decay_curve : list[float]
///     Per-period multipliers; element ``k`` weights the cohort that started ``k`` periods ago.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``FinancialModelSpec`` with the vintage node added.
#[pyfunction]
fn add_vintage_buildup(
    model: &Bound<'_, PyAny>,
    name: &str,
    new_volume_node: &str,
    decay_curve: Vec<f64>,
) -> PyResult<String> {
    let spec = extract_model_ref(model)?.into_owned();
    let updated = apply_vintage(spec, name, new_volume_node, &decay_curve)?;
    serde_json::to_string(&updated).map_err(display_to_py)
}

fn apply_vintage(
    spec: FinancialModelSpec,
    name: &str,
    new_volume_node: &str,
    decay_curve: &[f64],
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
    let builder = rust_vintage::add_vintage_buildup(builder, name, new_volume_node, decay_curve)
        .map_err(display_to_py)?;
    let mut new_spec = builder.build().map_err(display_to_py)?;
    new_spec.meta = meta;
    new_spec.capital_structure = capital_structure;
    Ok(new_spec)
}

/// Register the vintage template binding on the parent module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(add_vintage_buildup, m)?)?;
    Ok(())
}
