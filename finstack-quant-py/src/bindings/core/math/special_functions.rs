//! Python bindings for `finstack_quant_core::math::special_functions`.

use finstack_quant_core::math::special_functions;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

// ---------------------------------------------------------------------------
// Module-level functions
// ---------------------------------------------------------------------------

/// Standard normal cumulative distribution function Φ(x).
///
/// Returns the probability P(Z ≤ x) where Z ~ N(0, 1).
#[pyfunction]
#[pyo3(text_signature = "(x)")]
fn norm_cdf(x: f64) -> f64 {
    special_functions::norm_cdf(x)
}

/// Standard normal probability density function φ(x).
///
/// Returns (1/√(2π)) · exp(-x²/2).
#[pyfunction]
#[pyo3(text_signature = "(x)")]
fn norm_pdf(x: f64) -> f64 {
    special_functions::norm_pdf(x)
}

/// Inverse standard normal CDF (Φ⁻¹).
///
/// Returns x such that Φ(x) = p.
#[pyfunction]
#[pyo3(text_signature = "(p)")]
fn standard_normal_inv_cdf(p: f64) -> f64 {
    special_functions::standard_normal_inv_cdf(p)
}

/// Error function erf(x) = (2/√π) ∫₀ˣ e^(-t²) dt.
#[pyfunction]
#[pyo3(text_signature = "(x)")]
fn erf(x: f64) -> f64 {
    special_functions::erf(x)
}

/// Natural logarithm of the Gamma function ln(Γ(x)).
///
/// Returns `f64::INFINITY` for x ≤ 0.
#[pyfunction]
#[pyo3(text_signature = "(x)")]
fn ln_gamma(x: f64) -> f64 {
    special_functions::ln_gamma(x)
}

/// Student-t cumulative distribution function.
///
/// Returns P(T ≤ x) where T ~ t(df).
#[pyfunction]
#[pyo3(text_signature = "(x, df)")]
fn student_t_cdf(x: f64, df: f64) -> f64 {
    special_functions::student_t_cdf(x, df)
}

/// Inverse Student-t CDF (quantile function).
///
/// Returns x such that P(T ≤ x) = p where T ~ t(df).
#[pyfunction]
#[pyo3(text_signature = "(p, df)")]
fn student_t_inv_cdf(p: f64, df: f64) -> f64 {
    special_functions::student_t_inv_cdf(p, df)
}

// ---------------------------------------------------------------------------
// Register
// ---------------------------------------------------------------------------

/// Build the `finstack_quant.core.math.special_functions` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "special_functions")?;
    m.setattr(
        "__doc__",
        "Special mathematical functions: normal distribution, error function, gamma.",
    )?;

    m.add_function(wrap_pyfunction!(norm_cdf, &m)?)?;
    m.add_function(wrap_pyfunction!(norm_pdf, &m)?)?;
    m.add_function(wrap_pyfunction!(standard_normal_inv_cdf, &m)?)?;
    m.add_function(wrap_pyfunction!(erf, &m)?)?;
    m.add_function(wrap_pyfunction!(ln_gamma, &m)?)?;
    m.add_function(wrap_pyfunction!(student_t_cdf, &m)?)?;
    m.add_function(wrap_pyfunction!(student_t_inv_cdf, &m)?)?;

    let all = PyList::new(
        py,
        [
            "norm_cdf",
            "norm_pdf",
            "standard_normal_inv_cdf",
            "erf",
            "ln_gamma",
            "student_t_cdf",
            "student_t_inv_cdf",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "special_functions",
        "finstack_quant.core.math",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
