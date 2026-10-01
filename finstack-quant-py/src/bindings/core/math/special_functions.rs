//! Python bindings for `finstack_quant_core::math::special_functions`.

use finstack_quant_core::math::special_functions;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

use crate::errors::core_to_py;

/// Standard normal cumulative distribution function Φ(x).
///
/// Returns the probability P(Z ≤ x) where Z ~ N(0, 1).
#[pyfunction]
#[pyo3(text_signature = "(x)")]
fn norm_cdf(x: f64) -> f64 {
    special_functions::norm_cdf(x)
}

/// Natural logarithm of the standard normal cumulative distribution function.
///
/// # Arguments
/// - `x`: Standard-normal threshold in standard-deviation units; accepts infinities and propagates NaN.
#[pyfunction]
#[pyo3(text_signature = "(x)")]
fn log_norm_cdf(x: f64) -> f64 {
    special_functions::log_norm_cdf(x)
}

/// Standard normal probability density function φ(x).
///
/// Returns (1/√(2π)) · exp(-x²/2).
#[pyfunction]
#[pyo3(text_signature = "(x)")]
fn norm_pdf(x: f64) -> f64 {
    special_functions::norm_pdf(x)
}

/// Normal CDF with explicit mean and standard deviation: P(X ≤ x), X ~ N(mean, std_dev²).
///
/// Raises ``ValueError`` if ``std_dev`` is not strictly positive or any input is non-finite.
#[pyfunction]
#[pyo3(text_signature = "(x, mean, std_dev)")]
fn norm_cdf_with_params(x: f64, mean: f64, std_dev: f64) -> PyResult<f64> {
    special_functions::norm_cdf_with_params(x, mean, std_dev).map_err(core_to_py)
}

/// Normal PDF with explicit mean and standard deviation.
///
/// Raises ``ValueError`` if ``std_dev`` is not strictly positive or any input is non-finite.
#[pyfunction]
#[pyo3(text_signature = "(x, mean, std_dev)")]
fn norm_pdf_with_params(x: f64, mean: f64, std_dev: f64) -> PyResult<f64> {
    special_functions::norm_pdf_with_params(x, mean, std_dev).map_err(core_to_py)
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
fn student_t_cdf(x: f64, df: f64) -> PyResult<f64> {
    special_functions::student_t_cdf(x, df).map_err(core_to_py)
}

/// Inverse Student-t CDF (quantile function).
///
/// Returns x such that P(T ≤ x) = p where T ~ t(df).
#[pyfunction]
#[pyo3(text_signature = "(p, df)")]
fn student_t_inv_cdf(p: f64, df: f64) -> PyResult<f64> {
    special_functions::student_t_inv_cdf(p, df).map_err(core_to_py)
}

/// Build the `finstack_quant.core.math.special_functions` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "special_functions")?;
    m.setattr(
        "__doc__",
        "Special mathematical functions: normal distribution, error function, gamma.",
    )?;

    m.add_function(wrap_pyfunction!(norm_cdf, &m)?)?;
    m.add_function(wrap_pyfunction!(log_norm_cdf, &m)?)?;
    m.add_function(wrap_pyfunction!(norm_pdf, &m)?)?;
    m.add_function(wrap_pyfunction!(norm_cdf_with_params, &m)?)?;
    m.add_function(wrap_pyfunction!(norm_pdf_with_params, &m)?)?;
    m.add_function(wrap_pyfunction!(standard_normal_inv_cdf, &m)?)?;
    m.add_function(wrap_pyfunction!(erf, &m)?)?;
    m.add_function(wrap_pyfunction!(ln_gamma, &m)?)?;
    m.add_function(wrap_pyfunction!(student_t_cdf, &m)?)?;
    m.add_function(wrap_pyfunction!(student_t_inv_cdf, &m)?)?;

    let all = PyList::new(
        py,
        [
            "erf",
            "ln_gamma",
            "log_norm_cdf",
            "norm_cdf",
            "norm_cdf_with_params",
            "norm_pdf",
            "norm_pdf_with_params",
            "standard_normal_inv_cdf",
            "student_t_cdf",
            "student_t_inv_cdf",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;

    Ok(())
}
