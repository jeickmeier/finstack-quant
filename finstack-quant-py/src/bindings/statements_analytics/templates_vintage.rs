//! Typed vintage and cohort template composition.

use crate::bindings::pydoc::pydoc;
use crate::bindings::statements::builder::PyModelBuilder;
use crate::errors::statements_to_py;
use finstack_quant_statements_analytics::templates::vintage as rust_vintage;
use pyo3::prelude::*;

#[doc = pydoc!("finstack_quant.statements_analytics._templates_vintage", "add_vintage_buildup")]
#[pyfunction]
fn add_vintage_buildup(
    mut builder: PyRefMut<'_, PyModelBuilder>,
    name: &str,
    new_volume_node: &str,
    decay_curve: Vec<f64>,
) -> PyResult<()> {
    rust_vintage::add_vintage_buildup(builder.builder_mut()?, name, new_volume_node, &decay_curve)
        .map_err(statements_to_py)
}

/// Register the vintage template binding on the parent module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(add_vintage_buildup, m)?)?;
    Ok(())
}
