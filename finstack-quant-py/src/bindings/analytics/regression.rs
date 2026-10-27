//! Python binding for equality-constrained least-squares regression.
//!
//! Binds `finstack_quant_analytics::regression::constrained_least_squares`
//! (Jeet & Partani 2023, Appendix A) as
//! `finstack_quant.analytics.constrained_least_squares`: float lists in, a
//! list of factor returns out.

use pyo3::prelude::*;
use pyo3::types::PyModule;

use crate::bindings::pydoc::pydoc;
use crate::errors::analytics_to_py;

#[doc = pydoc!("finstack_quant.analytics._regression", "constrained_least_squares")]
#[pyfunction]
#[pyo3(text_signature = "(exposures, n_factors, returns, weights)")]
fn constrained_least_squares(
    py: Python<'_>,
    exposures: Vec<f64>,
    n_factors: usize,
    returns: Vec<f64>,
    weights: Vec<f64>,
) -> PyResult<Vec<f64>> {
    py.detach(move || {
        finstack_quant_analytics::regression::constrained_least_squares(
            &exposures, n_factors, &returns, &weights,
        )
        .map_err(analytics_to_py)
    })
}

/// Register regression functions on the analytics submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(constrained_least_squares, m)?)?;
    Ok(())
}
