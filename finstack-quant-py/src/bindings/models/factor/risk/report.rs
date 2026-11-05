use pyo3::prelude::*;

use finstack_quant_factor::risk::{self as model_risk, PositionRiskReport, PositionRiskRequest};

use super::budget::PyRiskBudgetResult;
use super::contributions::PyPositionRiskDecomposition;
use super::stress::PyStressAttribution;
use crate::bindings::macros::wire_methods;
use crate::bindings::module_utils::spec_from_py;
use crate::bindings::repr_support::repr_from_serde;
use crate::errors::core_to_py;

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
    let request: PositionRiskRequest = spec_from_py(py, request, "PositionRiskRequest")?;
    let inner = py
        .detach(move || model_risk::position_risk_report(&request))
        .map_err(core_to_py)?;
    Ok(PyPositionRiskReport { inner })
}
