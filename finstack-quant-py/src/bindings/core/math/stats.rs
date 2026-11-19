//! Python bindings for `finstack_quant_core::math::stats`.

use crate::bindings::extract::parse_label;
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
/// Validates all observations upfront. Returns ``None`` for an empty series.
///
/// # Arguments
///
/// * `data` - One-dimensional numeric sequence or float64 NumPy array, including strided views.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
/// Raises `ValueError` when any observation is non-finite or `len(data)` exceeds 2^53.
/// Raises `RuntimeError` when reconstruction overflows `float`.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn mean(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<Option<f64>> {
    let data = extract_series(data)?;
    py.detach(|| stats::mean(&data))
        .map_err(crate::errors::core_to_py)
}

/// Sample variance (unbiased, n-1 denominator).
///
/// Validates all observations upfront. Returns ``None`` for fewer than 2 observations.
///
/// # Arguments
///
/// * `data` - One-dimensional numeric sequence or float64 NumPy array, including strided views.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
/// Raises `ValueError` when any observation is non-finite or `len(data)` exceeds 2^53.
/// Raises `RuntimeError` when a non-constant sample variance overflows or underflows `float`.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn variance(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<Option<f64>> {
    let data = extract_series(data)?;
    py.detach(|| stats::variance(&data))
        .map_err(crate::errors::core_to_py)
}

/// Population variance (n denominator).
///
/// Validates all observations upfront. Returns ``None`` for an empty series.
///
/// # Arguments
///
/// * `data` - One-dimensional numeric sequence or float64 NumPy array, including strided views.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
/// Raises `ValueError` when any observation is non-finite or `len(data)` exceeds 2^53.
/// Raises `RuntimeError` when a non-constant population variance overflows or underflows `float`.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn population_variance(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<Option<f64>> {
    let data = extract_series(data)?;
    py.detach(|| stats::population_variance(&data))
        .map_err(crate::errors::core_to_py)
}

/// ``(mean, sample_variance)`` in a single pass; missing slots are ``None``.
///
/// # Arguments
///
/// * `data` - One-dimensional numeric sequence or float64 NumPy array, including strided views.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
/// Raises `ValueError` when any observation is non-finite or `len(data)` exceeds 2^53.
/// Raises `RuntimeError` when the mean or a non-constant variance overflows or underflows `float`.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn mean_var(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<(Option<f64>, Option<f64>)> {
    let data = extract_series(data)?;
    py.detach(|| stats::mean_var(&data))
        .map_err(crate::errors::core_to_py)
}

/// Unbiased sample standard deviation (n-1 denominator).
///
/// Validates all observations upfront. Returns ``None`` when ``data`` has
/// fewer than two observations, and ``0.0`` when ``len(data) >= 2`` and all
/// observations are identical.
///
/// # Arguments
///
/// * `data` - One-dimensional numeric sequence or float64 NumPy array, including strided views.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
/// Raises `ValueError` when any observation is non-finite.
/// Raises `RuntimeError` when a non-constant sample standard deviation overflows or underflows.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn std_dev(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<Option<f64>> {
    let data = extract_series(data)?;
    py.detach(|| stats::std_dev(&data))
        .map_err(crate::errors::core_to_py)
}

/// Sample median (R-7 / NumPy quantile at q=0.5).
///
/// Validates all observations upfront without mutating ``data``. Returns
/// ``None`` for an empty series.
///
/// # Arguments
///
/// * `data` - One-dimensional numeric sequence or float64 NumPy array, including strided views.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
/// Raises `ValueError` when any observation is non-finite.
#[pyfunction]
#[pyo3(text_signature = "(data)")]
fn median(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<Option<f64>> {
    let data = extract_series(data)?;
    py.detach(|| stats::median(&data))
        .map_err(crate::errors::core_to_py)
}

/// Minimum over finite values, or ``None`` when there are none.
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
fn finite_min(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<Option<f64>> {
    let data = extract_series(data)?;
    Ok(py.detach(|| stats::finite_min(&data)))
}

/// Maximum over finite values, or ``None`` when there are none.
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
fn finite_max(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<Option<f64>> {
    let data = extract_series(data)?;
    Ok(py.detach(|| stats::finite_max(&data)))
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
/// Returns ``None`` when fewer than 2 observations exist or either series is constant.
///
/// # Arguments
///
/// * `x` - First one-dimensional numeric series, including strided NumPy arrays.
/// * `y` - Second numeric series with observations aligned to `x`.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
/// Raises `ValueError` when lengths differ, any observation is non-finite, or `len(x)` exceeds 2^53.
/// Raises `RuntimeError` when numerical evaluation leaves ``[-1.0, 1.0]`` or produces a non-finite value.
#[pyfunction]
#[pyo3(text_signature = "(x, y)")]
fn correlation(
    py: Python<'_>,
    x: &Bound<'_, PyAny>,
    y: &Bound<'_, PyAny>,
) -> PyResult<Option<f64>> {
    let x = extract_series(x)?;
    let y = extract_series(y)?;
    py.detach(|| stats::correlation(&x, &y))
        .map_err(crate::errors::core_to_py)
}

/// Sample covariance (unbiased, n-1 denominator).
///
/// Returns ``None`` when fewer than 2 observations exist.
///
/// # Arguments
///
/// * `x` - First one-dimensional numeric series, including strided NumPy arrays.
/// * `y` - Second numeric series with observations aligned to `x`.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
/// Raises `ValueError` when lengths differ, any observation is non-finite, or `len(x)` exceeds 2^53.
/// Raises `RuntimeError` when a nonzero co-moment overflows or underflows `float`.
#[pyfunction]
#[pyo3(text_signature = "(x, y)")]
fn covariance(py: Python<'_>, x: &Bound<'_, PyAny>, y: &Bound<'_, PyAny>) -> PyResult<Option<f64>> {
    let x = extract_series(x)?;
    let y = extract_series(y)?;
    py.detach(|| stats::covariance(&x, &y))
        .map_err(crate::errors::core_to_py)
}

/// Empirical quantile (R-7 / NumPy default) with linear interpolation.
///
/// Returns ``None`` for empty data; caller storage is never reordered.
///
/// # Arguments
///
/// * `data` - One-dimensional numeric observations; caller storage is never reordered.
/// * `q` - Quantile probability in ``[0, 1]``.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
/// Raises `ValueError` when `q` is outside ``[0, 1]`` or non-finite, any observation is non-finite, or `len(data)` exceeds 2^53.
/// Raises `RuntimeError` when a nonzero quantile underflows `float`.
#[pyfunction]
#[pyo3(text_signature = "(data, q)")]
fn quantile(py: Python<'_>, data: &Bound<'_, PyAny>, q: f64) -> PyResult<Option<f64>> {
    let mut data = extract_series(data)?;
    py.detach(|| stats::quantile(&mut data, q))
        .map_err(crate::errors::core_to_py)
}

/// Log returns ``ln(p_t / p_{t-1})`` of a chronological price series.
///
/// Fewer than two valid prices yield an empty list.
///
/// # Arguments
///
/// * `prices` - Finite positive one-dimensional chronological prices. Invalid levels raise ValueError.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
#[pyfunction]
#[pyo3(text_signature = "(prices)")]
fn log_returns(py: Python<'_>, prices: &Bound<'_, PyAny>) -> PyResult<Vec<f64>> {
    let prices = extract_series(prices)?;
    py.detach(|| stats::log_returns(&prices))
        .map_err(core_to_py)
}

/// Annualized realized variance of a close price series (mean of squared log
/// returns, no mean subtraction, times ``annualization_factor``).
///
/// The OHLC estimators live in ``realized_variance_ohlc``. ``None`` selects
/// the Rust default of 252 daily observations per year. Raises ``ValueError``
/// for non-positive or non-finite prices or annualization factor. Fewer than two
/// valid prices yield ``None``; an unrepresentable variance raises ``RuntimeError``.
///
/// # Arguments
///
/// * `prices` - One-dimensional positive close prices in chronological order.
/// * `annualization_factor` - Positive observations-per-year multiplier; `None`
///   selects the Rust default of 252 for daily bars.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
#[pyfunction]
#[pyo3(signature = (prices, annualization_factor=None))]
#[pyo3(text_signature = "(prices, annualization_factor=None)")]
fn realized_variance(
    py: Python<'_>,
    prices: &Bound<'_, PyAny>,
    annualization_factor: Option<f64>,
) -> PyResult<Option<f64>> {
    let prices = extract_series(prices)?;
    py.detach(|| stats::realized_variance(&prices, annualization_factor))
        .map_err(core_to_py)
}

/// Annualized realized variance from OHLC bars.
///
/// ``method`` is one of ``"close_to_close"``, ``"parkinson"``,
/// ``"garman_klass"``, ``"rogers_satchell"``, ``"yang_zhang"``. ``None``
/// arguments select the Rust defaults (Yang-Zhang, 252 daily bars per year).
/// Raises ``ValueError`` when the four series differ in length or contain
/// invalid prices.
///
/// # Arguments
///
/// * `open` - One-dimensional opening prices, positive and finite in chronological order.
/// * `high` - Bar highs in the same units and observation order as `open`.
/// * `low` - Bar lows in the same units and observation order as `open`.
/// * `close` - Bar closes in the same units and observation order as `open`.
/// * `method` - Canonical OHLC estimator label; `None` selects the Rust default `yang_zhang`.
/// * `annualization_factor` - Positive observations-per-year multiplier; `None`
///   selects the Rust default of 252 for daily bars.
///
/// # Errors
///
/// Raises `TypeError` when a series is not one-dimensional or contains non-numeric values.
#[pyfunction]
#[pyo3(signature = (open, high, low, close, method=None, annualization_factor=None, previous_close=None))]
#[pyo3(
    text_signature = "(open, high, low, close, method=None, annualization_factor=None, previous_close=None)"
)]
fn realized_variance_ohlc(
    open: &Bound<'_, PyAny>,
    high: &Bound<'_, PyAny>,
    low: &Bound<'_, PyAny>,
    close: &Bound<'_, PyAny>,
    method: Option<&str>,
    annualization_factor: Option<f64>,
    previous_close: Option<f64>,
) -> PyResult<Option<f64>> {
    let py = open.py();
    let open = extract_series(open)?;
    let high = extract_series(high)?;
    let low = extract_series(low)?;
    let close = extract_series(close)?;
    let method = method.map(parse_label::<RealizedVarMethod>).transpose()?;
    py.detach(|| {
        stats::realized_variance_ohlc(
            &open,
            &high,
            &low,
            &close,
            method,
            annualization_factor,
            previous_close,
        )
    })
    .map_err(core_to_py)
}

/// Build the `finstack_quant.core.math.stats` submodule.
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = crate::bindings::module_utils::new_submodule(parent, "stats")?;
    m.setattr(
        "__doc__",
        "Statistical functions: mean, variance, standard deviation, median, correlation, covariance, quantiles, log returns and realized variance.",
    )?;

    m.add_function(wrap_pyfunction!(correlation, &m)?)?;
    m.add_function(wrap_pyfunction!(covariance, &m)?)?;
    m.add_function(wrap_pyfunction!(finite_count, &m)?)?;
    m.add_function(wrap_pyfunction!(finite_max, &m)?)?;
    m.add_function(wrap_pyfunction!(finite_min, &m)?)?;
    m.add_function(wrap_pyfunction!(log_returns, &m)?)?;
    m.add_function(wrap_pyfunction!(mean, &m)?)?;
    m.add_function(wrap_pyfunction!(mean_var, &m)?)?;
    m.add_function(wrap_pyfunction!(median, &m)?)?;
    m.add_function(wrap_pyfunction!(population_variance, &m)?)?;
    m.add_function(wrap_pyfunction!(quantile, &m)?)?;
    m.add_function(wrap_pyfunction!(realized_variance, &m)?)?;
    m.add_function(wrap_pyfunction!(realized_variance_ohlc, &m)?)?;
    m.add_function(wrap_pyfunction!(std_dev, &m)?)?;
    m.add_function(wrap_pyfunction!(variance, &m)?)?;

    let all = PyList::new(
        py,
        [
            "correlation",
            "covariance",
            "finite_count",
            "finite_max",
            "finite_min",
            "log_returns",
            "mean",
            "mean_var",
            "median",
            "population_variance",
            "quantile",
            "realized_variance",
            "realized_variance_ohlc",
            "std_dev",
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
