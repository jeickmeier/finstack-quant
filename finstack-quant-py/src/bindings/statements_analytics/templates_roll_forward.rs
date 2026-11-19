//! Typed roll-forward template composition.

use crate::bindings::pydoc::pydoc;
use crate::bindings::statements::builder::PyModelBuilder;
use finstack_quant_statements_analytics::templates::roll_forward as rust_roll_forward;
use pyo3::prelude::*;

#[doc = pydoc!("finstack_quant.statements_analytics._templates_roll_forward", "add_roll_forward")]
#[pyfunction]
fn add_roll_forward(
    mut builder: PyRefMut<'_, PyModelBuilder>,
    name: &str,
    increases: Vec<String>,
    decreases: Vec<String>,
) -> PyResult<()> {
    let increases: Vec<_> = increases.iter().map(String::as_str).collect();
    let decreases: Vec<_> = decreases.iter().map(String::as_str).collect();
    rust_roll_forward::add_roll_forward(builder.builder_mut()?, name, &increases, &decreases);
    Ok(())
}

#[doc = pydoc!("finstack_quant.statements_analytics._templates_roll_forward", "add_roll_forward_with_opening")]
#[pyfunction]
fn add_roll_forward_with_opening(
    mut builder: PyRefMut<'_, PyModelBuilder>,
    name: &str,
    increases: Vec<String>,
    decreases: Vec<String>,
    opening: f64,
) -> PyResult<()> {
    let increases: Vec<_> = increases.iter().map(String::as_str).collect();
    let decreases: Vec<_> = decreases.iter().map(String::as_str).collect();
    rust_roll_forward::add_roll_forward_with_opening(
        builder.builder_mut()?,
        name,
        &increases,
        &decreases,
        opening,
    );
    Ok(())
}

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(add_roll_forward, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(add_roll_forward_with_opening, m)?)?;
    Ok(())
}
