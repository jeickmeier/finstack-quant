use pyo3::prelude::*;

use finstack_quant_factor::risk::{
    self as model_risk, PositionRiskBudget, PositionRiskReport, PositionRiskRequest,
    PositionRiskSource,
};

use super::budget::PyRiskBudgetResult;
use super::contributions::PyPositionRiskDecomposition;
use super::stress::PyStressAttribution;
use crate::bindings::macros::wire_methods;
use crate::bindings::module_utils::spec_from_py;
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::core_to_py;

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.models.factor.risk._report", "PositionRiskSource")]
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.models.factor.risk._report", "PositionRiskSource.parametric")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.models.factor.risk._report", "PositionRiskSource.historical")]
    #[staticmethod]
    fn historical(pnls: Vec<Vec<f64>>) -> Self {
        Self {
            inner: PositionRiskSource::Historical { pnls },
        }
    }
}

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.models.factor.risk._report", "PositionRiskBudget")]
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.models.factor.risk._report", "PositionRiskBudget.__init__")]
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

#[doc = crate::bindings::pydoc::pydoc!("finstack_quant.models.factor.risk._report", "PositionRiskRequest")]
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
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.models.factor.risk._report", "PositionRiskRequest.__init__")]
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

/// Position VaR/ES decomposition with its optional risk budget and tail stress
/// attribution, as returned by :func:`position_risk_report`.
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
    "PositionRiskReport"
);

#[pymethods]
impl PyPositionRiskReport {
    /// Portfolio and per-position VaR/ES (losses negative).
    ///
    /// Returns
    /// -------
    /// PositionRiskDecomposition
    ///     The decomposition of the request's source.
    #[getter]
    fn decomposition(&self) -> PyPositionRiskDecomposition {
        PyPositionRiskDecomposition::from_inner(self.inner.decomposition.clone())
    }

    /// Budget evaluation of the decomposition.
    ///
    /// Returns
    /// -------
    /// RiskBudgetResult | None
    ///     The per-position budget comparison, or ``None`` when the request had
    ///     no budget.
    #[getter]
    fn budget(&self) -> Option<PyRiskBudgetResult> {
        self.inner
            .budget
            .clone()
            .map(PyRiskBudgetResult::from_inner)
    }

    /// Tail stress attribution of a historical source's scenario P&L.
    ///
    /// Returns
    /// -------
    /// StressAttribution | None
    ///     The tail attribution, or ``None`` for a parametric source.
    #[getter]
    fn stress(&self) -> Option<PyStressAttribution> {
        self.inner
            .stress
            .clone()
            .map(PyStressAttribution::from_inner)
    }

    fn __repr__(&self) -> String {
        repr_from_serde("PositionRiskReport", &self.inner)
    }
}

/// Decompose position VaR/ES from a parametric or historical source, with an
/// optional risk budget and, for scenario P&L, the tail stress attribution.
///
/// Args:
///     request: ``PositionRiskRequest`` as a dict or JSON text with
///         ``position_ids``, an optional ``confidence`` (decimal strictly
///         inside ``(0.5, 1)``, default ``0.95``), a ``source`` tagged by
///         ``kind`` — ``{"kind": "parametric", "weights", "covariance",
///         "compute_incremental"}`` (exposures in portfolio currency, an
///         ``n x n`` covariance in position order, incremental VaR off by
///         default) or ``{"kind": "historical", "pnls"}`` (one row of scenario
///         P&L per position, losses negative) — and an optional ``budget``
///         ``{"targets", "utilization_threshold"}`` (target VaR shares in
///         position order summing to ~1).
///
/// Returns:
///     ``PositionRiskReport`` with the decomposition, the budget evaluation
///     (``None`` without a budget) and the stress attribution (``None`` for a
///     parametric source).
///
/// Raises:
///     ValueError: If the request is malformed or has an unknown field or kind,
///         the source's dimensions disagree with the position ids, the
///         covariance is invalid, too few scenarios resolve the tail, the
///         confidence is outside ``(0.5, 1)``, or the budget is malformed.
#[pyfunction]
#[pyo3(signature = (request))]
pub(super) fn position_risk_report(
    py: Python<'_>,
    request: &Bound<'_, PyAny>,
) -> PyResult<PyPositionRiskReport> {
    let request = match request.cast::<PyPositionRiskRequest>() {
        Ok(typed) => typed.borrow().inner.clone(),
        Err(_) => spec_from_py(py, request, "PositionRiskRequest")?,
    };
    let inner = py
        .detach(move || model_risk::position_risk_report(&request))
        .map_err(core_to_py)?;
    Ok(PyPositionRiskReport { inner })
}
