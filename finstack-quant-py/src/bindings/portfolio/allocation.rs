//! Python bindings for lightweight strategy allocation.

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::json_bridge::serde_to_py;
use crate::bindings::macros::tabular_methods;
use crate::bindings::table_builders::serde_rows_to_arrow_table;
use crate::errors::portfolio_to_py;
use pyo3::prelude::*;
use pyo3::types::PyAny;

use finstack_quant_portfolio::allocation::{
    StrategyAllocationInput, WeightAllocationSpec, default_money_decimal_places,
};

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._allocation", "StrategyAllocationInput")]
#[pyclass(
    name = "StrategyAllocationInput",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
struct PyStrategyAllocationInput {
    inner: StrategyAllocationInput,
}

#[pymethods]
impl PyStrategyAllocationInput {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._allocation", "StrategyAllocationInput.__init__")]
    #[new]
    #[pyo3(signature = (id, *, fixed_weight=None, returns=None, risk_budget=None))]
    fn new(
        id: String,
        fixed_weight: Option<f64>,
        returns: Option<Vec<f64>>,
        risk_budget: Option<f64>,
    ) -> Self {
        Self {
            inner: StrategyAllocationInput {
                id,
                fixed_weight,
                returns: returns.unwrap_or_default(),
                risk_budget,
            },
        }
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._allocation", "WeightAllocationSpec")]
#[pyclass(
    name = "WeightAllocationSpec",
    module = "finstack_quant.portfolio",
    frozen,
    skip_from_py_object
)]
struct PyWeightAllocationSpec {
    inner: WeightAllocationSpec,
}

#[pymethods]
impl PyWeightAllocationSpec {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.portfolio._allocation", "WeightAllocationSpec.__init__")]
    #[new]
    #[pyo3(signature = (scheme, total_capital, strategies, *, covariance=None, money_decimal_places=None))]
    fn new(
        py: Python<'_>,
        scheme: &str,
        total_capital: f64,
        strategies: Vec<PyRef<'_, PyStrategyAllocationInput>>,
        covariance: Option<&Bound<'_, PyAny>>,
        money_decimal_places: Option<u32>,
    ) -> PyResult<Self> {
        let covariance = match covariance.filter(|cov| !cov.is_none()) {
            Some(cov) => {
                if let Some((flat, nrows, ncols)) =
                    crate::bindings::dataframe_utils::extract_2d_f64(cov, "covariance")?
                {
                    if nrows != ncols {
                        return Err(crate::errors::value_error(format!(
                            "covariance must be square, got {nrows} x {ncols}"
                        )));
                    }
                    Some(
                        flat.chunks_exact(ncols)
                            .map(|row| row.to_vec())
                            .collect::<Vec<_>>(),
                    )
                } else {
                    Some(cov.extract::<Vec<Vec<f64>>>()?)
                }
            }
            None => None,
        };
        let _ = py;
        Ok(Self {
            inner: WeightAllocationSpec {
                scheme: finstack_quant_core::wire::serde_parse(scheme)
                    .map_err(crate::errors::core_to_py)?,
                total_capital,
                strategies: strategies
                    .into_iter()
                    .map(|strategy| strategy.inner.clone())
                    .collect(),
                covariance,
                money_decimal_places: money_decimal_places
                    .unwrap_or_else(default_money_decimal_places),
            },
        })
    }
}

/// Column schema for [`PyWeightAllocationResult::to_arrow_table`].
const ALLOCATION_COLUMNS: &[(&str, &str)] = &[
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
    pub(crate) inner: finstack_quant_portfolio::allocation::WeightAllocationResult,
}

crate::bindings::macros::wire_methods!(
    PyWeightAllocationResult,
    finstack_quant_portfolio::allocation::WeightAllocationResult,
    "WeightAllocationResult"
);

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

    fn __repr__(&self) -> String {
        format!(
            "WeightAllocationResult(allocations={}, weights_sum={})",
            self.inner.allocations.len(),
            self.inner.diagnostics.weights_sum,
        )
    }
}

impl PyWeightAllocationResult {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        serde_rows_to_arrow_table(&self.inner.allocations, ALLOCATION_COLUMNS)
    }
}

tabular_methods!(PyWeightAllocationResult);

/// Allocate strategy weights from a typed specification.
///
/// ``total_capital`` is finite and signed; nonnegative strategy weights allocate
/// amounts with the same sign. Capital sums to the total rounded at
/// ``money_decimal_places`` (default 10, maximum 12). Minor units are assigned
/// by largest fractional remainder, with ties awarded in strategy input order.
/// Zero-weight strategies receive zero capital.
///
/// Parameters
/// ----------
/// spec : WeightAllocationSpec | str | dict
///     Typed request or its wire representation with a scheme, finite signed
///     total capital, strategy rows, and any required covariance data.
///
/// Returns
/// -------
/// WeightAllocationResult
///     Typed result with ``scheme``, ``allocations`` and ``diagnostics``
///     getters plus ``to_pandas()`` / ``to_polars()`` / ``to_arrow()``; ``to_json()`` gives the wire form.
///     A spec that allocates without error is valid.
///
/// Raises
/// ------
/// PortfolioError
///     If the specification is malformed, violates scheme invariants, or its
///     capital is not representable at the requested rounding precision.
#[pyfunction]
#[pyo3(text_signature = "(spec)")]
fn allocate_weights(py: Python<'_>, spec: &Bound<'_, PyAny>) -> PyResult<PyWeightAllocationResult> {
    if let Ok(typed) = spec.cast::<PyWeightAllocationSpec>() {
        let spec = typed.borrow().inner.clone();
        let inner = py
            .detach(move || finstack_quant_portfolio::allocation::allocate_weights(&spec))
            .map_err(portfolio_to_py)?;
        return Ok(PyWeightAllocationResult { inner });
    }
    let spec = crate::bindings::extract::extract_records_json(py, spec, "spec")?;
    let inner = py
        .detach(move || {
            let spec =
                finstack_quant_portfolio::allocation::WeightAllocationSpec::from_json(&spec)?;
            finstack_quant_portfolio::allocation::allocate_weights(&spec)
        })
        .map_err(portfolio_to_py)?;
    Ok(PyWeightAllocationResult { inner })
}

/// Register strategy allocation functions on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyStrategyAllocationInput>()?;
    m.add_class::<PyWeightAllocationSpec>()?;
    m.add_class::<PyWeightAllocationResult>()?;
    m.add_function(pyo3::wrap_pyfunction!(allocate_weights, m)?)?;
    Ok(())
}
