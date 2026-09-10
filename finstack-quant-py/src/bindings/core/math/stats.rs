//! Python bindings for `finstack_quant_core::math::stats`.

use finstack_quant_core::math::stats::{self, RealizedVarMethod};
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

use crate::errors::core_to_py;

/// Arithmetic mean of a data series.
///
/// Returns ``0.0`` for an empty list.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn mean(data: Vec<f64>) -> f64 {
    stats::mean(&data)
}

/// Sample variance (unbiased, n-1 denominator).
///
/// Returns ``0.0`` for fewer than 2 observations.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn variance(data: Vec<f64>) -> f64 {
    stats::variance(&data)
}

/// Population variance (n denominator).
///
/// Returns ``0.0`` for an empty list.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn population_variance(data: Vec<f64>) -> f64 {
    stats::population_variance(&data)
}

/// ``(mean, sample_variance)`` in a single Welford pass; ``(0.0, 0.0)`` for empty input.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn mean_var(data: Vec<f64>) -> (f64, f64) {
    stats::mean_var(&data)
}

/// Arithmetic mean, or ``nan`` for empty input.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn mean_or_nan(data: Vec<f64>) -> f64 {
    stats::mean_or_nan(&data)
}

/// Sample variance (n-1 denominator), or ``nan`` for fewer than 2 observations.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn sample_variance_or_nan(data: Vec<f64>) -> f64 {
    stats::sample_variance_or_nan(&data)
}

/// Sample standard deviation (n-1 denominator), or ``nan`` for fewer than 2 observations.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn sample_std_or_nan(data: Vec<f64>) -> f64 {
    stats::sample_std_or_nan(&data)
}

/// Median (mean of the two middle values for even counts), or ``nan`` for empty input.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn median_or_nan(data: Vec<f64>) -> f64 {
    stats::median_or_nan(&data)
}

/// Linearly interpolated quantile (R-7); clamps ``q`` to ``[0, 1]`` and returns ``nan`` for NaN ``q`` or empty/non-finite data.
#[pyfunction]
#[pyo3(text_signature = "(data, q)")]
fn quantile_linear_or_nan(data: Vec<f64>, q: f64) -> f64 {
    stats::quantile_linear_or_nan(&data, q)
}

/// Minimum over finite values, or ``nan`` when there are none.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn finite_min_or_nan(data: Vec<f64>) -> f64 {
    stats::finite_min_or_nan(&data)
}

/// Maximum over finite values, or ``nan`` when there are none.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn finite_max_or_nan(data: Vec<f64>) -> f64 {
    stats::finite_max_or_nan(&data)
}

/// Number of finite (non-NaN, non-infinite) values.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn finite_count(data: Vec<f64>) -> usize {
    stats::finite_count(&data)
}

/// Pearson correlation coefficient between two equal-length series.
///
/// Returns ``NaN`` if the input lengths differ.
#[pyfunction]
#[pyo3(text_signature = "(x, y)")]
fn correlation(x: Vec<f64>, y: Vec<f64>) -> f64 {
    stats::correlation(&x, &y)
}

/// Sample covariance (unbiased, n-1 denominator).
///
/// Returns ``NaN`` if the input lengths differ.
#[pyfunction]
#[pyo3(text_signature = "(x, y)")]
fn covariance(x: Vec<f64>, y: Vec<f64>) -> f64 {
    stats::covariance(&x, &y)
}

/// Empirical quantile (R-7 / NumPy default) with linear interpolation.
///
/// Returns ``NaN`` for empty data, `q` outside ``[0, 1]``, or non-finite inputs.
#[pyfunction]
#[pyo3(text_signature = "(data, q)")]
fn quantile(mut data: Vec<f64>, q: f64) -> f64 {
    stats::quantile(&mut data, q)
}

/// Log returns ``ln(p_t / p_{t-1})`` of a chronological price series.
///
/// Windows with a non-positive or non-finite price yield ``nan``; fewer than
/// two prices yield an empty list.
#[pyfunction]
#[pyo3(text_signature = "(prices)")]
fn log_returns(prices: Vec<f64>) -> Vec<f64> {
    stats::log_returns(&prices)
}

fn parse_method(method: &str) -> PyResult<RealizedVarMethod> {
    finstack_quant_core::wire::serde_parse(method).map_err(core_to_py)
}

/// Annualized realized variance of a close price series (sum of squared log
/// returns, no mean subtraction, times ``annualization_factor``).
///
/// ``method`` must be ``"close_to_close"``; the OHLC estimators require
/// ``realized_variance_ohlc``. Raises ``ValueError`` for non-positive or
/// non-finite prices or annualization factor.
#[pyfunction]
#[pyo3(signature = (prices, method="close_to_close", annualization_factor=252.0))]
#[pyo3(text_signature = "(prices, method='close_to_close', annualization_factor=252.0)")]
fn realized_variance(prices: Vec<f64>, method: &str, annualization_factor: f64) -> PyResult<f64> {
    let method = parse_method(method)?;
    stats::realized_variance(&prices, method, annualization_factor).map_err(core_to_py)
}

/// Annualized realized variance from OHLC bars.
///
/// ``method`` is one of ``"close_to_close"``, ``"parkinson"``,
/// ``"garman_klass"``, ``"rogers_satchell"``, ``"yang_zhang"``. Raises
/// ``ValueError`` when the four series differ in length or contain invalid
/// prices.
#[pyfunction]
#[pyo3(signature = (open, high, low, close, method="yang_zhang", annualization_factor=252.0))]
#[pyo3(
    text_signature = "(open, high, low, close, method='yang_zhang', annualization_factor=252.0)"
)]
fn realized_variance_ohlc(
    open: Vec<f64>,
    high: Vec<f64>,
    low: Vec<f64>,
    close: Vec<f64>,
    method: &str,
    annualization_factor: f64,
) -> PyResult<f64> {
    let method = parse_method(method)?;
    stats::realized_variance_ohlc(&open, &high, &low, &close, method, annualization_factor)
        .map_err(core_to_py)
}

/// Build the `finstack_quant.core.math.stats` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "stats")?;
    m.setattr(
        "__doc__",
        "Statistical functions: mean, variance, correlation, covariance, quantiles, NaN-sentinel summaries, log returns and realized variance.",
    )?;

    m.add_function(wrap_pyfunction!(correlation, &m)?)?;
    m.add_function(wrap_pyfunction!(covariance, &m)?)?;
    m.add_function(wrap_pyfunction!(finite_count, &m)?)?;
    m.add_function(wrap_pyfunction!(finite_max_or_nan, &m)?)?;
    m.add_function(wrap_pyfunction!(finite_min_or_nan, &m)?)?;
    m.add_function(wrap_pyfunction!(log_returns, &m)?)?;
    m.add_function(wrap_pyfunction!(mean, &m)?)?;
    m.add_function(wrap_pyfunction!(mean_or_nan, &m)?)?;
    m.add_function(wrap_pyfunction!(mean_var, &m)?)?;
    m.add_function(wrap_pyfunction!(median_or_nan, &m)?)?;
    m.add_function(wrap_pyfunction!(population_variance, &m)?)?;
    m.add_function(wrap_pyfunction!(quantile, &m)?)?;
    m.add_function(wrap_pyfunction!(quantile_linear_or_nan, &m)?)?;
    m.add_function(wrap_pyfunction!(realized_variance, &m)?)?;
    m.add_function(wrap_pyfunction!(realized_variance_ohlc, &m)?)?;
    m.add_function(wrap_pyfunction!(sample_std_or_nan, &m)?)?;
    m.add_function(wrap_pyfunction!(sample_variance_or_nan, &m)?)?;
    m.add_function(wrap_pyfunction!(variance, &m)?)?;

    let all = PyList::new(
        py,
        [
            "correlation",
            "covariance",
            "finite_count",
            "finite_max_or_nan",
            "finite_min_or_nan",
            "log_returns",
            "mean",
            "mean_or_nan",
            "mean_var",
            "median_or_nan",
            "population_variance",
            "quantile",
            "quantile_linear_or_nan",
            "realized_variance",
            "realized_variance_ohlc",
            "sample_std_or_nan",
            "sample_variance_or_nan",
            "variance",
        ],
    )?;
    m.setattr("__all__", all)?;
    crate::bindings::module_utils::register_submodule(
        py,
        parent,
        &m,
        "stats",
        "finstack_quant.core.math",
        crate::bindings::module_utils::ParentNameSource::Package,
    )?;

    Ok(())
}
