use pyo3::prelude::*;

use finstack_quant_factor::risk::{
    self as model_risk, PositionRiskBudget, PositionRiskReport, PositionRiskRequest,
    PositionRiskSource,
};

use super::budget::PyRiskBudgetResult;
use super::contributions::PyPositionRiskDecomposition;
use super::stress::PyStressAttribution;
use crate::bindings::macros::{py_getters, wire_methods};
use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pydoc::pydoc;
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::core_to_py;

#[doc = pydoc!("finstack_quant.models.factor.risk._report", "PositionRiskSource")]
#[pyclass(
    name = "PositionRiskSource",
    module = "finstack_quant.models.factor.risk",
    frozen,
    skip_from_py_object
)]
pub(super) struct PyPositionRiskSource {
    inner: PositionRiskSource,
}

#[pymethods]
impl PyPositionRiskSource {
    #[doc = pydoc!("finstack_quant.models.factor.risk._report", "PositionRiskSource.parametric")]
    #[staticmethod]
    #[pyo3(signature = (weights, covariance, *, compute_incremental=false))]
    fn parametric(weights: Vec<f64>, covariance: Vec<Vec<f64>>, compute_incremental: bool) -> Self {
        Self {
            inner: PositionRiskSource::Parametric {
                weights,
                covariance,
                compute_incremental,
            },
        }
    }

    #[doc = pydoc!("finstack_quant.models.factor.risk._report", "PositionRiskSource.historical")]
    #[staticmethod]
    fn historical(pnls: Vec<Vec<f64>>) -> Self {
        Self {
            inner: PositionRiskSource::Historical { pnls },
        }
    }
}

#[doc = pydoc!("finstack_quant.models.factor.risk._report", "PositionRiskBudget")]
#[pyclass(
    name = "PositionRiskBudget",
    module = "finstack_quant.models.factor.risk",
    frozen,
    skip_from_py_object
)]
pub(super) struct PyPositionRiskBudget {
    inner: PositionRiskBudget,
}

#[pymethods]
impl PyPositionRiskBudget {
    #[doc = pydoc!("finstack_quant.models.factor.risk._report", "PositionRiskBudget.__init__")]
    #[new]
    #[pyo3(signature = (targets, *, utilization_threshold=None))]
    fn new(targets: Vec<f64>, utilization_threshold: Option<f64>) -> Self {
        Self {
            inner: PositionRiskBudget {
                targets,
                utilization_threshold,
            },
        }
    }
}

#[doc = pydoc!("finstack_quant.models.factor.risk._report", "PositionRiskRequest")]
#[pyclass(
    name = "PositionRiskRequest",
    module = "finstack_quant.models.factor.risk",
    frozen,
    skip_from_py_object
)]
pub(super) struct PyPositionRiskRequest {
    inner: PositionRiskRequest,
}

#[pymethods]
impl PyPositionRiskRequest {
    #[doc = pydoc!("finstack_quant.models.factor.risk._report", "PositionRiskRequest.__init__")]
    #[new]
    #[pyo3(signature = (position_ids, source, *, confidence=None, budget=None))]
    fn new(
        position_ids: Vec<String>,
        source: &PyPositionRiskSource,
        confidence: Option<f64>,
        budget: Option<&PyPositionRiskBudget>,
    ) -> Self {
        Self {
            inner: PositionRiskRequest {
                position_ids,
                source: source.inner.clone(),
                confidence,
                budget: budget.map(|budget| budget.inner.clone()),
            },
        }
    }
}

#[doc = pydoc!("finstack_quant.models.factor.risk._report", "PositionRiskReport")]
#[pyclass(
    name = "PositionRiskReport",
    module = "finstack_quant.models.factor.risk",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyPositionRiskReport {
    pub(crate) inner: PositionRiskReport,
}

wire_methods!(
    PyPositionRiskReport,
    PositionRiskReport,
    "PositionRiskReport",
    module = "finstack_quant.models.factor.risk._report"
);

py_getters!(PyPositionRiskReport |s| {
    #[doc = pydoc!("finstack_quant.models.factor.risk._report", "PositionRiskReport.decomposition")]
    decomposition -> PyPositionRiskDecomposition =
        PyPositionRiskDecomposition::from_inner(s.inner.decomposition.clone());
    #[doc = pydoc!("finstack_quant.models.factor.risk._report", "PositionRiskReport.budget")]
    budget -> Option<PyRiskBudgetResult> =
        s.inner.budget.clone().map(PyRiskBudgetResult::from_inner);
    #[doc = pydoc!("finstack_quant.models.factor.risk._report", "PositionRiskReport.stress")]
    stress -> Option<PyStressAttribution> =
        s.inner.stress.clone().map(PyStressAttribution::from_inner);
});

#[pymethods]
impl PyPositionRiskReport {
    fn __repr__(&self) -> String {
        repr_from_serde("PositionRiskReport", &self.inner)
    }
}

#[doc = pydoc!("finstack_quant.models.factor.risk._report", "position_risk_report")]
#[pyfunction]
#[pyo3(signature = (request))]
pub(super) fn position_risk_report(
    py: Python<'_>,
    request: &Bound<'_, PyAny>,
) -> PyResult<PyPositionRiskReport> {
    let request = match request.cast::<PyPositionRiskRequest>() {
        Ok(typed) => typed.borrow().inner.clone(),
        Err(_) => py_to_serde(py, request, "PositionRiskRequest")?,
    };
    let inner = py
        .detach(move || model_risk::position_risk_report(&request))
        .map_err(core_to_py)?;
    Ok(PyPositionRiskReport { inner })
}
