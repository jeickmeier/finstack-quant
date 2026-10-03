//! Python bindings for the vintage / cohort buildup template.
//!
//! Wraps [`finstack_quant_statements_analytics::templates::vintage`].
//!
//! The binding reconstructs the canonical Rust builder from the supplied model,
//! applies `add_vintage_buildup`, and returns a typed model specification.

use crate::bindings::statements::types::PyFinancialModelSpec;
use crate::errors::statements_to_py;
use finstack_quant_statements_analytics::templates::vintage as rust_vintage;
use pyo3::prelude::*;

use super::templates_common::{extract_builder, finish_builder};

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
/// FinancialModelSpec
///     Typed model specification with the vintage node added.
#[pyfunction]
fn add_vintage_buildup(
    model: &Bound<'_, PyAny>,
    name: &str,
    new_volume_node: &str,
    decay_curve: Vec<f64>,
) -> PyResult<PyFinancialModelSpec> {
    let builder = rust_vintage::add_vintage_buildup(
        extract_builder(model)?,
        name,
        new_volume_node,
        &decay_curve,
    )
    .map_err(statements_to_py)?;
    finish_builder(builder)
}

/// Register the vintage template binding on the parent module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(add_vintage_buildup, m)?)?;
    Ok(())
}
