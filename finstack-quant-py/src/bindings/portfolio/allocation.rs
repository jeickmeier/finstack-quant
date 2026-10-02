//! Python bindings for lightweight strategy allocation.

use crate::bindings::pandas_utils::{
    serde_rows_to_dataframe_with_schema, serde_to_py, ColumnSchema,
};
use crate::errors::{display_to_py, portfolio_to_py};
use pyo3::prelude::*;
use pyo3::types::PyAny;

/// Column schema for [`PyWeightAllocationResult::to_dataframe`].
const ALLOCATION_COLUMNS: &[ColumnSchema<'static>] = &[
    ("id", "str"),
    ("weight", "float64"),
    ("capital", "float64"),
    ("volatility", "float64"),
    ("risk_contribution", "float64"),
];

/// Strategy weight-allocation result.
///
/// Returned by :func:`allocate_weights`.
#[pyclass(
    name = "WeightAllocationResult",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyWeightAllocationResult {
    pub(crate) inner: finstack_quant_portfolio::WeightAllocationResult,
}

#[pymethods]
impl PyWeightAllocationResult {
    /// Allocation scheme applied (e.g. ``"inverse_volatility"``).
    #[getter]
    fn scheme<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.scheme)
    }

    /// Per-strategy allocation rows as a list of dicts.
    #[getter]
    fn allocations<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.allocations)
    }

    /// Portfolio-level diagnostics (``weights_sum``, ``leverage``) as a dict.
    #[getter]
    fn diagnostics<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.diagnostics)
    }

    /// Per-strategy allocations as a :class:`pandas.DataFrame`.
    ///
    /// Columns: ``id``, ``weight``, ``capital``, ``volatility``,
    /// ``risk_contribution`` (the last two are null when the scheme does not
    /// produce them).
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_rows_to_dataframe_with_schema(py, &self.inner.allocations, ALLOCATION_COLUMNS)
    }

    /// Serialize to a compact JSON string.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Deserialize from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_portfolio::WeightAllocationResult =
            serde_json::from_str(json).map_err(display_to_py)?;
        Ok(Self { inner })
    }

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    fn __repr__(&self) -> String {
        format!(
            "WeightAllocationResult(allocations={}, weights_sum={})",
            self.inner.allocations.len(),
            self.inner.diagnostics.weights_sum,
        )
    }
}

/// Allocate strategy weights from a JSON specification.
///
/// ``total_capital`` is finite and signed; nonnegative strategy weights allocate
/// amounts with the same sign. Capital sums to the total rounded at
/// ``money_decimal_places`` (default 10, maximum 12). Minor units are assigned
/// by largest fractional remainder, with ties awarded in strategy input order.
/// Zero-weight strategies receive zero capital.
///
/// Parameters
/// ----------
/// spec_json : str | dict | list | pandas.DataFrame
///     JSON-serialized ``WeightAllocationSpec`` with a scheme, finite signed
///     total capital, strategy rows, and any required covariance data.
///
/// Returns
/// -------
/// WeightAllocationResult
///     Typed result with ``scheme``, ``allocations`` and ``diagnostics``
///     getters plus ``to_dataframe()``. Use :func:`allocate_weights_json`
///     for the raw wire string.
///
/// Raises
/// ------
/// PortfolioError
///     If the specification is malformed, violates scheme invariants, or its
///     capital is not representable at the requested rounding precision.
#[pyfunction]
#[pyo3(text_signature = "(spec_json)")]
fn allocate_weights(
    py: Python<'_>,
    spec_json: &Bound<'_, PyAny>,
) -> PyResult<PyWeightAllocationResult> {
    let spec_json = crate::bindings::extract::extract_records_json(py, spec_json, "spec")?;
    let spec_json: &str = &spec_json;
    let spec_json = spec_json.to_owned();
    let inner = py
        .detach(move || {
            // Mirrors the crate-private `parse_allocation_spec` so the typed
            // and `_json` twins raise identical parse errors.
            let spec: finstack_quant_portfolio::WeightAllocationSpec =
                serde_json::from_str(&spec_json).map_err(|err| {
                    finstack_quant_portfolio::Error::validation(format!(
                        "invalid allocation JSON: {err}"
                    ))
                })?;
            finstack_quant_portfolio::allocate_weights(&spec)
        })
        .map_err(portfolio_to_py)?;
    Ok(PyWeightAllocationResult { inner })
}

/// Allocate strategy weights from a JSON specification and return wire JSON.
///
/// Wire twin of :func:`allocate_weights`; same signed-capital, largest-remainder
/// rounding and validation, with JSON-string output.
///
/// Parameters
/// ----------
/// spec_json : str | dict | list | pandas.DataFrame
///     JSON-serialized ``WeightAllocationSpec`` selecting the scheme, finite
///     signed total capital, strategy inputs, and any required covariance data.
///
/// Returns
/// -------
/// str
///     JSON-serialized ``WeightAllocationResult``.
///
/// Raises
/// ------
/// PortfolioError
///     If the specification is malformed, violates scheme invariants, or its
///     capital is not representable at the requested rounding precision.
#[pyfunction]
#[pyo3(text_signature = "(spec_json)")]
fn allocate_weights_json(py: Python<'_>, spec_json: &Bound<'_, PyAny>) -> PyResult<String> {
    let spec_json = crate::bindings::extract::extract_records_json(py, spec_json, "spec")?;
    let spec_json: &str = &spec_json;
    let spec_json = spec_json.to_owned();
    py.detach(move || finstack_quant_portfolio::allocate_weights_json(&spec_json))
        .map_err(portfolio_to_py)
}

/// Validate a strategy allocation JSON specification.
///
/// Executes the same allocation and capital rounding as :func:`allocate_weights`
/// to validate the request, then discards the result.
///
/// Parameters
/// ----------
/// spec_json : str | dict | list | pandas.DataFrame
///     JSON-serialized ``WeightAllocationSpec``.
///
/// Returns
/// -------
/// str
///     Canonical compact JSON.
///
/// Raises
/// ------
/// PortfolioError
///     If the specification is malformed, violates scheme invariants, or its
///     capital is not representable at the requested rounding precision.
#[pyfunction]
#[pyo3(text_signature = "(spec_json)")]
fn validate_allocation_json(py: Python<'_>, spec_json: &Bound<'_, PyAny>) -> PyResult<String> {
    let spec_json = crate::bindings::extract::extract_records_json(py, spec_json, "spec")?;
    let spec_json: &str = &spec_json;
    let spec_json = spec_json.to_owned();
    py.detach(move || finstack_quant_portfolio::validate_allocation_json(&spec_json))
        .map_err(portfolio_to_py)
}

/// Register strategy allocation functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyWeightAllocationResult>()?;
    m.add_function(pyo3::wrap_pyfunction!(allocate_weights, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(allocate_weights_json, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(validate_allocation_json, m)?)?;
    Ok(())
}
