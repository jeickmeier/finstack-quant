use pyo3::prelude::*;
use pyo3::types::PyDict;

use finstack_quant_portfolio::factor_model::{StressPnl, StressResult};

use crate::bindings::pandas_utils::dict_to_dataframe;

use crate::bindings::json_bridge::{deserialize_json, serialize_json};
use crate::bindings::models::factor::risk::PyRiskDecomposition;

/// Result of a factor-stress scenario.
#[pyclass(
    name = "StressResult",
    module = "finstack_quant.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyStressResult {
    pub(crate) inner: StressResult,
}

impl PyStressResult {
    pub(super) fn from_inner(inner: StressResult) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyStressResult {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Parse from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: StressResult = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

    /// Total portfolio P&L under the stressed market.
    ///
    /// Returns
    /// -------
    /// float
    ///     Portfolio-currency amount; a loss is **negative**.
    #[getter]
    fn total_pnl(&self) -> f64 {
        self.inner.total_pnl
    }

    /// Per-position ``(position_id, pnl)`` entries.
    #[getter]
    fn position_pnl(&self) -> Vec<(String, f64)> {
        self.inner
            .position_pnl
            .iter()
            .map(|(id, pnl)| (id.as_str().to_owned(), *pnl))
            .collect()
    }

    /// Risk decomposition recomputed under the stressed market.
    #[getter]
    fn stressed_decomposition(&self) -> PyRiskDecomposition {
        PyRiskDecomposition::from_inner(self.inner.stressed_decomposition.clone())
    }

    /// Export the per-position stressed P&L as a pandas ``DataFrame``.
    ///
    /// One row per entry of :attr:`position_pnl`. The scenario totals stay on
    /// :attr:`total_pnl` and :attr:`stressed_decomposition`.
    ///
    /// Columns: ``position_id``, ``pnl`` (portfolio-currency amount; a loss is
    /// negative).
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = &self.inner.position_pnl;
        let position_ids: Vec<&str> = rows.iter().map(|(id, _)| id.as_str()).collect();
        let pnl: Vec<f64> = rows.iter().map(|(_, pnl)| *pnl).collect();
        let data = PyDict::new(py);
        data.set_item("position_id", position_ids)?;
        data.set_item("pnl", pnl)?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        format!(
            "StressResult(total_pnl={}, positions={}, stressed_total_risk={})",
            self.inner.total_pnl,
            self.inner.position_pnl.len(),
            self.inner.stressed_decomposition.total_risk,
        )
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to the frame from `to_dataframe`, so pandas' own row/column
    /// truncation applies and a large result stays a small repr. Returns
    /// `None` if the frame cannot be built, which makes IPython fall back to
    /// `__repr__` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}

/// P&L-only result of a factor-stress scenario.
///
/// Stressed-minus-base present value without a risk decomposition on the
/// shocked market. Produced by :meth:`FactorModel.factor_stress_pnl`; use
/// :meth:`FactorModel.factor_stress` when the stressed decomposition is also
/// needed.
#[pyclass(
    name = "StressPnl",
    module = "finstack_quant.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyStressPnl {
    pub(crate) inner: StressPnl,
}

impl PyStressPnl {
    pub(super) fn from_inner(inner: StressPnl) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyStressPnl {
    /// Support `pickle` through the same strict serde round-trip as
    /// `to_json` / `from_json`.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Parse from a JSON string.
    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: StressPnl = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

    /// Total portfolio P&L under the stressed market.
    ///
    /// Returns
    /// -------
    /// float
    ///     Portfolio base-currency amount; a loss is **negative**.
    #[getter]
    fn total_pnl(&self) -> f64 {
        self.inner.total_pnl
    }

    /// Per-position ``(position_id, pnl)`` entries in portfolio order.
    #[getter]
    fn position_pnl(&self) -> Vec<(String, f64)> {
        self.inner
            .position_pnl
            .iter()
            .map(|(id, pnl)| (id.as_str().to_owned(), *pnl))
            .collect()
    }

    /// Export the per-position stressed P&L as a pandas ``DataFrame``.
    ///
    /// Columns: ``position_id``, ``pnl`` (portfolio base-currency amount; a
    /// loss is negative).
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = &self.inner.position_pnl;
        let position_ids: Vec<&str> = rows.iter().map(|(id, _)| id.as_str()).collect();
        let pnl: Vec<f64> = rows.iter().map(|(_, pnl)| *pnl).collect();
        let data = PyDict::new(py);
        data.set_item("position_id", position_ids)?;
        data.set_item("pnl", pnl)?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        format!(
            "StressPnl(total_pnl={}, positions={})",
            self.inner.total_pnl,
            self.inner.position_pnl.len(),
        )
    }

    /// Render as an HTML table in Jupyter notebooks; `None` falls back to
    /// `__repr__`.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py).ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }
}
