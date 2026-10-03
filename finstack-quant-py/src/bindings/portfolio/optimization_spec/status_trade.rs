use pyo3::prelude::*;
use pyo3::types::PyType;

use finstack_quant_portfolio::optimization::{OptimizationStatus, TradeDirection, TradeSpec};

use crate::bindings::pandas_utils::serde_object_to_single_row_dataframe;

use super::enums::{PyTradeDirection, PyTradeType};
use crate::bindings::json_bridge::{deserialize_json, serialize_json};

/// Status of an optimization run (mirrors `OptimizationStatus`).
#[pyclass(
    name = "OptimizationStatus",
    module = "finstack_quant.portfolio",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Clone, PartialEq, Eq)]
pub(super) struct PyOptimizationStatus {
    pub(crate) inner: OptimizationStatus,
}

impl std::hash::Hash for PyOptimizationStatus {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // `OptimizationStatus` does not derive `Hash` in the Rust crate, so
        // hash a variant tag plus the payload. Equal values (derived
        // `PartialEq`) hash equally by construction.
        match &self.inner {
            OptimizationStatus::Optimal => 0u8.hash(state),
            OptimizationStatus::FeasibleButSuboptimal => 1u8.hash(state),
            OptimizationStatus::Unbounded => 2u8.hash(state),
            OptimizationStatus::Infeasible {
                conflicting_constraints,
            } => {
                3u8.hash(state);
                conflicting_constraints.hash(state);
            }
            OptimizationStatus::Error { message } => {
                4u8.hash(state);
                message.hash(state);
            }
        }
    }
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
    fn feasible_but_suboptimal(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(OptimizationStatus::FeasibleButSuboptimal)
    }

    #[classmethod]
    fn unbounded(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(OptimizationStatus::Unbounded)
    }

    #[classmethod]
    #[pyo3(text_signature = "(cls, conflicting_constraints)")]
    fn infeasible(_cls: &Bound<'_, PyType>, conflicting_constraints: Vec<String>) -> Self {
        Self::from_inner(OptimizationStatus::Infeasible {
            conflicting_constraints,
        })
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

    /// Variant tag (``"optimal"``, ``"feasible_but_suboptimal"``, ...).
    #[getter]
    fn kind(&self) -> &'static str {
        match &self.inner {
            OptimizationStatus::Optimal => "optimal",
            OptimizationStatus::FeasibleButSuboptimal => "feasible_but_suboptimal",
            OptimizationStatus::Infeasible { .. } => "infeasible",
            OptimizationStatus::Unbounded => "unbounded",
            OptimizationStatus::Error { .. } => "error",
        }
    }

    /// Whether this status represents a usable (feasible) solution.
    #[getter]
    fn is_feasible(&self) -> bool {
        self.inner.is_feasible()
    }

    /// Conflicting constraint names when ``kind == "infeasible"``, otherwise
    /// an empty list.
    #[getter]
    fn conflicting_constraints(&self) -> Vec<String> {
        match &self.inner {
            OptimizationStatus::Infeasible {
                conflicting_constraints,
            } => conflicting_constraints.clone(),
            _ => Vec::new(),
        }
    }

    /// Error message when ``kind == "error"``, otherwise ``None``.
    #[getter]
    fn message(&self) -> Option<String> {
        match &self.inner {
            OptimizationStatus::Error { message } => Some(message.clone()),
            _ => None,
        }
    }

    fn __repr__(&self) -> String {
        format!("OptimizationStatus.{}(...)", self.kind())
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

    /// Export this trade as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``position_id``, ``instrument_id``, ``trade_type``,
    /// ``current_quantity``, ``target_quantity``, ``delta_quantity``,
    /// ``direction``, ``current_weight``, ``target_weight`` — the same schema
    /// :meth:`PortfolioOptimizationResult.to_trade_dataframe` emits, so
    /// single-trade frames concatenate cleanly onto a full trade list.
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_object_to_single_row_dataframe(py, &self.inner)
    }

    fn __repr__(&self) -> String {
        format!(
            "TradeSpec(position_id={:?}, instrument_id={:?}, direction={:?}, delta_quantity={})",
            self.inner.position_id.as_str(),
            self.inner.instrument_id,
            match self.inner.direction {
                TradeDirection::Buy => "buy",
                TradeDirection::Sell => "sell",
                TradeDirection::Hold => "hold",
            },
            self.inner.delta_quantity,
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
