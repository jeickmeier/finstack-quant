use pyo3::prelude::*;
use pyo3::types::PyType;

use finstack_quant_portfolio::optimization::{OptimizationStatus, TradeSpec};

use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::tabular_methods;
use crate::bindings::table_builders::serde_object_to_single_row_arrow_table;

use super::enums::{PyTradeDirection, PyTradeType};
use crate::bindings::json_bridge::{deserialize_json, serialize_json};
use crate::errors::core_to_py;

/// Status of an optimization run (mirrors `OptimizationStatus`).
#[pyclass(
    name = "OptimizationStatus",
    module = "finstack_quant.portfolio",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) struct PyOptimizationStatus {
    pub(crate) inner: OptimizationStatus,
}

impl PyOptimizationStatus {
    pub(crate) fn from_inner(inner: OptimizationStatus) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyOptimizationStatus {
    #[classmethod]
    fn optimal(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(OptimizationStatus::Optimal)
    }

    #[classmethod]
    fn unbounded(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(OptimizationStatus::Unbounded)
    }

    #[classmethod]
    fn infeasible(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(OptimizationStatus::Infeasible)
    }

    #[classmethod]
    #[pyo3(text_signature = "(cls, message)")]
    fn error(_cls: &Bound<'_, PyType>, message: String) -> Self {
        Self::from_inner(OptimizationStatus::Error { message })
    }

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
        let inner: OptimizationStatus = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

    /// Variant tag (``"optimal"``, ``"infeasible"``, ``"unbounded"`` or ``"error"``).
    #[getter]
    fn kind(&self) -> PyResult<String> {
        finstack_quant_core::wire::serde_tag(&self.inner).map_err(core_to_py)
    }

    /// Whether this status represents a usable (feasible) solution.
    #[getter]
    fn is_feasible(&self) -> bool {
        self.inner.is_feasible()
    }

    /// Error message when ``kind == "error"``, otherwise ``None``.
    #[getter]
    fn message(&self) -> Option<String> {
        match &self.inner {
            OptimizationStatus::Error { message } => Some(message.clone()),
            _ => None,
        }
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("OptimizationStatus.{}(...)", self.kind()?))
    }
}

/// Trade specification for a single position.
#[pyclass(
    name = "TradeSpec",
    module = "finstack_quant.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyTradeSpec {
    pub(crate) inner: TradeSpec,
}

impl PyTradeSpec {
    pub(crate) fn from_inner(inner: TradeSpec) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyTradeSpec {
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
        let inner: TradeSpec = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

    /// Position identifier in the optimized portfolio.
    #[getter]
    fn position_id(&self) -> String {
        self.inner.position_id.as_str().to_owned()
    }

    /// Underlying instrument identifier, or the candidate id for a new position.
    #[getter]
    fn instrument_id(&self) -> String {
        self.inner.instrument_id.clone()
    }

    /// Whether the trade adjusts an existing position, opens a new one, or
    /// closes one out.
    #[getter]
    fn trade_type(&self) -> PyTradeType {
        PyTradeType {
            inner: self.inner.trade_type,
        }
    }

    /// Buy / sell / hold classification, derived from the sign of
    /// :attr:`delta_quantity`.
    #[getter]
    fn direction(&self) -> PyTradeDirection {
        PyTradeDirection {
            inner: self.inner.direction,
        }
    }

    /// Pre-trade quantity in instrument units (units / face / notional).
    #[getter]
    fn current_quantity(&self) -> f64 {
        self.inner.current_quantity
    }

    /// Post-trade quantity in the same units as :attr:`current_quantity`.
    #[getter]
    fn target_quantity(&self) -> f64 {
        self.inner.target_quantity
    }

    /// Quantity change ``target - current``; negative means a sell.
    #[getter]
    fn delta_quantity(&self) -> f64 {
        self.inner.delta_quantity
    }

    /// Pre-trade portfolio weight as a **fraction**, not a percentage.
    #[getter]
    fn current_weight(&self) -> f64 {
        self.inner.current_weight
    }

    /// Post-trade portfolio weight as a **fraction**, not a percentage.
    #[getter]
    fn target_weight(&self) -> f64 {
        self.inner.target_weight
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "TradeSpec(position_id={:?}, instrument_id={:?}, direction={:?}, delta_quantity={})",
            self.inner.position_id.as_str(),
            self.inner.instrument_id,
            finstack_quant_core::wire::serde_label(&self.inner.direction).map_err(core_to_py)?,
            self.inner.delta_quantity,
        ))
    }
}

impl PyTradeSpec {
    pub(crate) fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        serde_object_to_single_row_arrow_table(
            &self.inner,
            &[
                ("position_id", "str"),
                ("instrument_id", "str"),
                ("trade_type", "str"),
                ("current_quantity", "float64"),
                ("target_quantity", "float64"),
                ("delta_quantity", "float64"),
                ("direction", "str"),
                ("current_weight", "float64"),
                ("target_weight", "float64"),
            ],
        )
    }
}

tabular_methods!(PyTradeSpec);
