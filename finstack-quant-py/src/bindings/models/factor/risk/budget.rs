use super::contributions::PyPositionRiskDecomposition;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::{py_getters, tabular_methods, wire_methods};
use crate::bindings::pydoc::pydoc;
use crate::bindings::repr_support::repr_from_serde;
use crate::bindings::table_builders::TableBuilder;
use crate::errors::core_to_py;
use finstack_quant_factor::risk::{self as model_risk, PositionBudgetEntry, RiskBudgetResult};
use pyo3::prelude::*;

fn position_budget_entries_table(rows: &[PositionBudgetEntry]) -> PyResult<PyArrowTable> {
    let position_ids: Vec<String> = rows
        .iter()
        .map(|e| e.position_id.as_str().to_owned())
        .collect();
    let actual: Vec<f64> = rows.iter().map(|e| e.actual_component_var).collect();
    let target: Vec<f64> = rows.iter().map(|e| e.target_component_var).collect();
    let utilization: Vec<f64> = rows.iter().map(|e| e.utilization).collect();
    let excess: Vec<f64> = rows.iter().map(|e| e.excess).collect();
    TableBuilder::new()
        .utf8("position_id", position_ids)?
        .f64("actual_component_var", actual)?
        .f64("target_component_var", target)?
        .f64("utilization", utilization)?
        .f64("excess", excess)?
        .build()
}

#[doc = pydoc!("finstack_quant.models.factor.risk._budget", "PositionBudgetEntry")]
#[pyclass(
    name = "PositionBudgetEntry",
    module = "finstack_quant.models.factor.risk",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyPositionBudgetEntry {
    pub(crate) inner: PositionBudgetEntry,
}

impl PyPositionBudgetEntry {
    fn from_inner(inner: PositionBudgetEntry) -> Self {
        Self { inner }
    }
}

wire_methods!(
    PyPositionBudgetEntry,
    PositionBudgetEntry,
    "PositionBudgetEntry",
    module = "finstack_quant.models.factor.risk._budget"
);

py_getters!(PyPositionBudgetEntry |s| {
    #[doc = pydoc!("finstack_quant.models.factor.risk._budget", "PositionBudgetEntry.position_id")]
    position_id -> String = s.inner.position_id.as_str().to_owned();
    #[doc = pydoc!("finstack_quant.models.factor.risk._budget", "PositionBudgetEntry.actual_component_var")]
    actual_component_var -> f64 = s.inner.actual_component_var;
    #[doc = pydoc!("finstack_quant.models.factor.risk._budget", "PositionBudgetEntry.target_component_var")]
    target_component_var -> f64 = s.inner.target_component_var;
    #[doc = pydoc!("finstack_quant.models.factor.risk._budget", "PositionBudgetEntry.utilization")]
    utilization -> f64 = s.inner.utilization;
    #[doc = pydoc!("finstack_quant.models.factor.risk._budget", "PositionBudgetEntry.excess")]
    excess -> f64 = s.inner.excess;
});

#[pymethods]
impl PyPositionBudgetEntry {
    #[doc = pydoc!("finstack_quant.models.factor.risk._budget", "PositionBudgetEntry.__repr__")]
    fn __repr__(&self) -> String {
        repr_from_serde("PositionBudgetEntry", &self.inner)
    }
}

impl PyPositionBudgetEntry {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        position_budget_entries_table(std::slice::from_ref(&self.inner))
    }
}

tabular_methods!(
    PyPositionBudgetEntry,
    "PositionBudgetEntry",
    module = "finstack_quant.models.factor.risk._budget"
);

#[doc = pydoc!("finstack_quant.models.factor.risk._budget", "RiskBudgetResult")]
#[pyclass(
    name = "RiskBudgetResult",
    module = "finstack_quant.models.factor.risk",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyRiskBudgetResult {
    pub(crate) inner: RiskBudgetResult,
}

impl PyRiskBudgetResult {
    pub(super) fn from_inner(inner: RiskBudgetResult) -> Self {
        Self { inner }
    }
}

wire_methods!(
    PyRiskBudgetResult,
    RiskBudgetResult,
    "RiskBudgetResult",
    module = "finstack_quant.models.factor.risk._budget"
);

py_getters!(PyRiskBudgetResult |s| {
    #[doc = pydoc!("finstack_quant.models.factor.risk._budget", "RiskBudgetResult.total_overbudget")]
    total_overbudget -> f64 = s.inner.total_overbudget;
    #[doc = pydoc!("finstack_quant.models.factor.risk._budget", "RiskBudgetResult.has_breach")]
    has_breach -> bool = s.inner.has_breach;
    #[doc = pydoc!("finstack_quant.models.factor.risk._budget", "RiskBudgetResult.positions")]
    positions -> Vec<PyPositionBudgetEntry> = s
        .inner
        .positions
        .iter()
        .cloned()
        .map(PyPositionBudgetEntry::from_inner)
        .collect();
});

#[pymethods]
impl PyRiskBudgetResult {
    #[doc = pydoc!("finstack_quant.models.factor.risk._budget", "RiskBudgetResult.__repr__")]
    fn __repr__(&self) -> String {
        format!(
            "RiskBudgetResult(positions={}, total_overbudget={}, has_breach={})",
            self.inner.positions.len(),
            self.inner.total_overbudget,
            if self.inner.has_breach {
                "True"
            } else {
                "False"
            },
        )
    }
}

impl PyRiskBudgetResult {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        position_budget_entries_table(&self.inner.positions)
    }
}

tabular_methods!(
    PyRiskBudgetResult,
    "RiskBudgetResult",
    module = "finstack_quant.models.factor.risk._budget"
);

#[doc = pydoc!("finstack_quant.models.factor.risk._budget", "evaluate_risk_budget")]
#[pyfunction]
#[pyo3(signature = (position_ids, actual_var, target_var_pct, portfolio_var, utilization_threshold = finstack_quant_factor::risk::DEFAULT_UTILIZATION_THRESHOLD))]
pub(super) fn evaluate_risk_budget(
    py: Python<'_>,
    position_ids: Vec<String>,
    actual_var: Vec<f64>,
    target_var_pct: Vec<f64>,
    portfolio_var: f64,
    utilization_threshold: f64,
) -> PyResult<PyRiskBudgetResult> {
    let result = py
        .detach(move || {
            model_risk::evaluate_risk_budget(
                position_ids,
                &actual_var,
                &target_var_pct,
                portfolio_var,
                utilization_threshold,
            )
        })
        .map_err(core_to_py)?;

    Ok(PyRiskBudgetResult::from_inner(result))
}

#[doc = pydoc!("finstack_quant.models.factor.risk._budget", "evaluate_risk_budget_from_decomposition")]
#[pyfunction]
#[pyo3(signature = (decomp, target_var_pct, utilization_threshold = None))]
pub(super) fn evaluate_risk_budget_from_decomposition(
    decomp: &PyPositionRiskDecomposition,
    target_var_pct: Vec<f64>,
    utilization_threshold: Option<f64>,
) -> PyResult<PyRiskBudgetResult> {
    model_risk::evaluate_risk_budget_from_decomposition(
        &decomp.inner,
        &target_var_pct,
        utilization_threshold,
    )
    .map(PyRiskBudgetResult::from_inner)
    .map_err(core_to_py)
}

// The literal default below must stay equal to the Rust canonical constant so
// the generated text signature shows the real number.
const _: () = assert!(model_risk::DEFAULT_UTILIZATION_THRESHOLD == 1.2);
