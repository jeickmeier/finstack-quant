//! Python bindings for `finstack_quant_core::math::stats`.

use finstack_quant_core::math::stats::{self, RealizedVarMethod};
use numpy::PyReadonlyArray1;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyModule};

use crate::errors::core_to_py;

/// Copy a one-dimensional series while holding the GIL, before detached Rust work.
fn extract_series(data: &Bound<'_, PyAny>) -> PyResult<Vec<f64>> {
    if let Ok(array) = data.extract::<PyReadonlyArray1<'_, f64>>() {
        return Ok(match array.as_slice() {
            Ok(slice) => slice.to_vec(),
            Err(_) => array.as_array().iter().copied().collect(),
        });
    }
    data.extract()
}

/// Arithmetic mean of a data series.
///
/// Returns ``0.0`` for an empty list.
///
/// # Arguments
///
/// * `data` - One-dimensional numeric sequence or float64 NumPy array, including strided views.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn mean(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<f64> {
    let data = extract_series(data)?;
    Ok(py.detach(|| stats::mean(&data)))
}

/// Sample variance (unbiased, n-1 denominator).
///
/// Returns ``0.0`` for fewer than 2 observations.
///
/// # Arguments
///
/// * `data` - One-dimensional numeric sequence or float64 NumPy array, including strided views.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn variance(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<f64> {
    let data = extract_series(data)?;
    Ok(py.detach(|| stats::variance(&data)))
}

/// Population variance (n denominator).
///
/// Returns ``0.0`` for an empty list.
///
/// # Arguments
///
/// * `data` - One-dimensional numeric sequence or float64 NumPy array, including strided views.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn population_variance(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<f64> {
    let data = extract_series(data)?;
    Ok(py.detach(|| stats::population_variance(&data)))
}

/// ``(mean, sample_variance)`` in a single Welford pass; ``(0.0, 0.0)`` for empty input.
///
/// # Arguments
///
/// * `data` - One-dimensional numeric sequence or float64 NumPy array, including strided views.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn mean_var(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<(f64, f64)> {
    let data = extract_series(data)?;
    Ok(py.detach(|| stats::mean_var(&data)))
}

/// Arithmetic mean, or ``nan`` for empty input.
///
/// # Arguments
///
/// * `data` - One-dimensional numeric sequence or float64 NumPy array, including strided views.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn mean_or_nan(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<f64> {
    let data = extract_series(data)?;
    Ok(py.detach(|| stats::mean_or_nan(&data)))
}

/// Sample variance (n-1 denominator), or ``nan`` for fewer than 2 observations.
///
/// # Arguments
///
/// * `data` - One-dimensional numeric sequence or float64 NumPy array, including strided views.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn sample_variance_or_nan(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<f64> {
    let data = extract_series(data)?;
    Ok(py.detach(|| stats::sample_variance_or_nan(&data)))
}

/// Sample standard deviation (n-1 denominator), or ``nan`` for fewer than 2 observations.
///
/// # Arguments
///
/// * `data` - One-dimensional numeric sequence or float64 NumPy array, including strided views.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn sample_std_or_nan(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<f64> {
    let data = extract_series(data)?;
    Ok(py.detach(|| stats::sample_std_or_nan(&data)))
}

/// Median (mean of the two middle values for even counts), or ``nan`` for empty input.
///
/// # Arguments
///
/// * `data` - One-dimensional numeric sequence or float64 NumPy array, including strided views.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn median_or_nan(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<f64> {
    let data = extract_series(data)?;
    Ok(py.detach(|| stats::median_or_nan(&data)))
}

/// Linearly interpolated quantile (R-7); clamps ``q`` to ``[0, 1]`` and returns ``nan`` for NaN ``q`` or empty/non-finite data.
///
/// # Arguments
///
/// * `data` - One-dimensional numeric observations; caller storage is never reordered.
/// * `q` - Quantile probability clamped to [0, 1]; NaN returns NaN.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
#[pyfunction]
#[pyo3(text_signature = "(data, q)")]
fn quantile_linear_or_nan(py: Python<'_>, data: &Bound<'_, PyAny>, q: f64) -> PyResult<f64> {
    let data = extract_series(data)?;
    Ok(py.detach(|| stats::quantile_linear_or_nan(&data, q)))
}

/// Minimum over finite values, or ``nan`` when there are none.
///
/// # Arguments
///
/// * `data` - One-dimensional numeric sequence or float64 NumPy array, including strided views.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn finite_min_or_nan(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<f64> {
    let data = extract_series(data)?;
    Ok(py.detach(|| stats::finite_min_or_nan(&data)))
}

/// Maximum over finite values, or ``nan`` when there are none.
///
/// # Arguments
///
/// * `data` - One-dimensional numeric sequence or float64 NumPy array, including strided views.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn finite_max_or_nan(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<f64> {
    let data = extract_series(data)?;
    Ok(py.detach(|| stats::finite_max_or_nan(&data)))
}

/// Number of finite (non-NaN, non-infinite) values.
///
/// # Arguments
///
/// * `data` - One-dimensional numeric sequence or float64 NumPy array, including strided views.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn finite_count(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<usize> {
    let data = extract_series(data)?;
    Ok(py.detach(|| stats::finite_count(&data)))
}

/// Pearson correlation coefficient between two equal-length series.
///
/// Returns ``NaN`` if the input lengths differ.
///
/// # Arguments
///
/// * `x` - First one-dimensional numeric series, including strided NumPy arrays.
/// * `y` - Second numeric series with observations aligned to `x`.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
#[pyfunction]
#[pyo3(text_signature = "(x, y)")]
fn correlation(py: Python<'_>, x: &Bound<'_, PyAny>, y: &Bound<'_, PyAny>) -> PyResult<f64> {
    let x = extract_series(x)?;
    let y = extract_series(y)?;
    Ok(py.detach(|| stats::correlation(&x, &y)))
}

/// Sample covariance (unbiased, n-1 denominator).
///
/// Returns ``NaN`` if the input lengths differ.
///
/// # Arguments
///
/// * `x` - First one-dimensional numeric series, including strided NumPy arrays.
/// * `y` - Second numeric series with observations aligned to `x`.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
#[pyfunction]
#[pyo3(text_signature = "(x, y)")]
fn covariance(py: Python<'_>, x: &Bound<'_, PyAny>, y: &Bound<'_, PyAny>) -> PyResult<f64> {
    let x = extract_series(x)?;
    let y = extract_series(y)?;
    Ok(py.detach(|| stats::covariance(&x, &y)))
}

/// Empirical quantile (R-7 / NumPy default) with linear interpolation.
///
/// Returns ``NaN`` for empty data, `q` outside ``[0, 1]``, or non-finite inputs.
///
/// # Arguments
///
/// * `data` - One-dimensional numeric observations; caller storage is never reordered.
/// * `q` - Quantile probability in [0, 1]; invalid values return NaN.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
#[pyfunction]
#[pyo3(text_signature = "(data, q)")]
fn quantile(py: Python<'_>, data: &Bound<'_, PyAny>, q: f64) -> PyResult<f64> {
    let mut data = extract_series(data)?;
    Ok(py.detach(|| stats::quantile(&mut data, q)))
}

/// Log returns ``ln(p_t / p_{t-1})`` of a chronological price series.
///
/// Windows with a non-positive or non-finite price yield ``nan``; fewer than
/// two prices yield an empty list.
///
/// # Arguments
///
/// * `prices` - One-dimensional chronological prices; invalid windows retain Rust NaN semantics.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
#[pyfunction]
#[pyo3(text_signature = "(prices)")]
fn log_returns(py: Python<'_>, prices: &Bound<'_, PyAny>) -> PyResult<Vec<f64>> {
    let prices = extract_series(prices)?;
    Ok(py.detach(|| stats::log_returns(&prices)))
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
///
/// # Arguments
///
/// * `prices` - One-dimensional positive close prices in chronological order.
/// * `method` - Canonical estimator label; this entry point requires `close_to_close`.
/// * `annualization_factor` - Positive observations-per-year multiplier, normally 252 for daily bars.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
#[pyfunction]
#[pyo3(signature = (prices, method="close_to_close", annualization_factor=252.0))]
#[pyo3(text_signature = "(prices, method='close_to_close', annualization_factor=252.0)")]
fn realized_variance(
    py: Python<'_>,
    prices: &Bound<'_, PyAny>,
    method: &str,
    annualization_factor: f64,
) -> PyResult<f64> {
    let prices = extract_series(prices)?;
    let method = parse_method(method)?;
    py.detach(|| stats::realized_variance(&prices, method, annualization_factor))
        .map_err(core_to_py)
}

/// Annualized realized variance from OHLC bars.
///
/// ``method`` is one of ``"close_to_close"``, ``"parkinson"``,
/// ``"garman_klass"``, ``"rogers_satchell"``, ``"yang_zhang"``. Raises
/// ``ValueError`` when the four series differ in length or contain invalid
/// prices.
///
/// # Arguments
///
/// * `open` - One-dimensional opening prices, positive and finite in chronological order.
/// * `high` - Bar highs in the same units and observation order as `open`.
/// * `low` - Bar lows in the same units and observation order as `open`.
/// * `close` - Bar closes in the same units and observation order as `open`.
/// * `method` - Canonical OHLC estimator label, defaulting to `yang_zhang`.
/// * `annualization_factor` - Positive observations-per-year multiplier, normally 252 for daily bars.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
#[pyfunction]
#[pyo3(signature = (open, high, low, close, method="yang_zhang", annualization_factor=252.0))]
#[pyo3(
    text_signature = "(open, high, low, close, method='yang_zhang', annualization_factor=252.0)"
)]
fn realized_variance_ohlc(
    py: Python<'_>,
    open: &Bound<'_, PyAny>,
    high: &Bound<'_, PyAny>,
    low: &Bound<'_, PyAny>,
    close: &Bound<'_, PyAny>,
    method: &str,
    annualization_factor: f64,
) -> PyResult<f64> {
    let open = extract_series(open)?;
    let high = extract_series(high)?;
    let low = extract_series(low)?;
    let close = extract_series(close)?;
    let method = parse_method(method)?;
    py.detach(|| {
        stats::realized_variance_ohlc(&open, &high, &low, &close, method, annualization_factor)
    })
    .map_err(core_to_py)
}

/// Build the `finstack_quant.core.math.stats` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "stats")?;
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
    crate::bindings::module_utils::attach_submodule(
        parent,
        &m,
        crate::bindings::module_utils::Exposure::Compiled,
    )?;

    Ok(())
}
