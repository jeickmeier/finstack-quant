//! Typed `#[pyclass]` wrappers for `finstack_portfolio::factor_model` result types.
//!
//! The pre-existing dict-returning helpers in [`super::position_risk`]
//! (``parametric_var_decomposition``, ``historical_var_decomposition``,
//! ``evaluate_risk_budget``) are kept untouched for backwards compatibility.
//! This module adds *typed* sibling functions (suffix ``_typed``) that return
//! structured ``#[pyclass]`` wrappers around the same Rust result types, plus
//! the full set of result classes for callers that want to inspect a
//! ``RiskDecomposition``, ``WhatIfResult``, ``StressResult``, ``CreditVolReport``,
//! or ``FactorAssignmentReport`` without serializing through JSON.
//!
//! Engine and builder types (``FactorModel``, ``FactorModelBuilder``,
//! ``ParametricPositionDecomposer``, ``HistoricalPositionDecomposer``,
//! ``WhatIfEngine``, ``FactorCovarianceForecast``) are intentionally left for
//! a future slice — they hold borrowed handles or trait objects that do not
//! map cleanly to a JSON-first PyO3 surface and are not required by the
//! result-type contract this slice fulfils.

use std::collections::HashMap;

use indexmap::IndexMap;
use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyModule, PyType};

use finstack_portfolio::factor_model::{
    self as fm, CreditVolReport, DecompositionConfig, DecompositionMethod, FactorAssignmentReport,
    FactorContribution, FactorContributionDelta, HistoricalPositionDecomposer,
    LevelVolContribution, ParametricPositionDecomposer, PositionAssignment, PositionBudgetEntry,
    PositionEsContribution, PositionFactorContribution, PositionResidualContribution,
    PositionRiskDecomposition, PositionVarContribution, PositionVolContribution,
    ResidualContributionSource, RiskBudget, RiskBudgetResult, RiskDecomposition, StressAttribution,
    StressPositionEntry, StressResult, TailScenarioBreakdown, UnmatchedEntry, VolHorizon,
    WhatIfResult,
};
use finstack_portfolio::types::PositionId;

use crate::errors::{core_to_py, display_to_py};

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

/// Convert `Vec<String>` of position ids into the Rust newtype.
fn to_position_ids(ids: Vec<String>) -> Vec<PositionId> {
    ids.into_iter().map(PositionId::new).collect()
}

/// Forward to the shared `factor_model::flatten_square_matrix` and remap the
/// `core::Error::Validation` shape into a `PyValueError` so the same matrix
/// validation diagnostics surface from both the Python and WASM bindings.
fn flatten_square_matrix(matrix: Vec<Vec<f64>>, n: usize, label: &str) -> PyResult<Vec<f64>> {
    fm::flatten_square_matrix(matrix, n, label).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// Convert a Rust `DecompositionMethod` to a stable Python string.
fn decomposition_method_label(method: DecompositionMethod) -> &'static str {
    match method {
        DecompositionMethod::Parametric => "parametric",
        DecompositionMethod::Historical => "historical",
    }
}

// ---------------------------------------------------------------------------
// FactorContribution
// ---------------------------------------------------------------------------

/// Aggregate contribution of a single factor to portfolio risk.
#[pyclass(
    name = "FactorContribution",
    module = "finstack.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyFactorContribution {
    pub(crate) inner: FactorContribution,
}

impl PyFactorContribution {
    fn from_inner(inner: FactorContribution) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFactorContribution {
    /// Parse from a JSON string.
    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: FactorContribution = serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn factor_id(&self) -> String {
        self.inner.factor_id.as_str().to_owned()
    }

    #[getter]
    fn absolute_risk(&self) -> f64 {
        self.inner.absolute_risk
    }

    #[getter]
    fn relative_risk(&self) -> f64 {
        self.inner.relative_risk
    }

    #[getter]
    fn marginal_risk(&self) -> f64 {
        self.inner.marginal_risk
    }

    fn __repr__(&self) -> String {
        format!(
            "FactorContribution(factor_id={:?}, absolute_risk={}, relative_risk={}, marginal_risk={})",
            self.inner.factor_id.as_str(),
            self.inner.absolute_risk,
            self.inner.relative_risk,
            self.inner.marginal_risk,
        )
    }
}

// ---------------------------------------------------------------------------
// PositionFactorContribution
// ---------------------------------------------------------------------------

/// Per-position contribution to a specific factor bucket.
#[pyclass(
    name = "PositionFactorContribution",
    module = "finstack.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyPositionFactorContribution {
    pub(crate) inner: PositionFactorContribution,
}

impl PyPositionFactorContribution {
    fn from_inner(inner: PositionFactorContribution) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPositionFactorContribution {
    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: PositionFactorContribution =
            serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn position_id(&self) -> String {
        self.inner.position_id.as_str().to_owned()
    }

    #[getter]
    fn factor_id(&self) -> String {
        self.inner.factor_id.as_str().to_owned()
    }

    #[getter]
    fn risk_contribution(&self) -> f64 {
        self.inner.risk_contribution
    }

    fn __repr__(&self) -> String {
        format!(
            "PositionFactorContribution(position_id={:?}, factor_id={:?}, risk_contribution={})",
            self.inner.position_id.as_str(),
            self.inner.factor_id.as_str(),
            self.inner.risk_contribution,
        )
    }
}

// ---------------------------------------------------------------------------
// PositionResidualContribution
// ---------------------------------------------------------------------------

/// Annualized residual variance contributed by a single position.
#[pyclass(
    name = "PositionResidualContribution",
    module = "finstack.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyPositionResidualContribution {
    pub(crate) inner: PositionResidualContribution,
}

impl PyPositionResidualContribution {
    fn from_inner(inner: PositionResidualContribution) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPositionResidualContribution {
    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: PositionResidualContribution =
            serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn position_id(&self) -> String {
        self.inner.position_id.as_str().to_owned()
    }

    #[getter]
    fn residual_variance(&self) -> f64 {
        self.inner.residual_variance
    }

    /// Source kind: ``"from_credit_model"`` or ``"other"``.
    #[getter]
    fn source_kind(&self) -> &'static str {
        match &self.inner.source {
            ResidualContributionSource::FromCreditModel { .. } => "from_credit_model",
            ResidualContributionSource::Other => "other",
        }
    }

    /// Issuer ID when ``source_kind == "from_credit_model"``, ``None`` otherwise.
    #[getter]
    fn source_issuer_id(&self) -> Option<String> {
        match &self.inner.source {
            ResidualContributionSource::FromCreditModel { issuer_id } => {
                Some(issuer_id.as_str().to_owned())
            }
            ResidualContributionSource::Other => None,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "PositionResidualContribution(position_id={:?}, residual_variance={}, source_kind={:?})",
            self.inner.position_id.as_str(),
            self.inner.residual_variance,
            self.source_kind(),
        )
    }
}

// ---------------------------------------------------------------------------
// RiskDecomposition
// ---------------------------------------------------------------------------

/// Portfolio-level decomposition of total risk across common factors and residuals.
#[pyclass(
    name = "RiskDecomposition",
    module = "finstack.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyRiskDecomposition {
    pub(crate) inner: RiskDecomposition,
}

impl PyRiskDecomposition {
    pub(crate) fn from_inner(inner: RiskDecomposition) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyRiskDecomposition {
    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: RiskDecomposition = serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn total_risk(&self) -> f64 {
        self.inner.total_risk
    }

    /// Risk measure used for aggregation (serialized as a JSON-compatible string).
    #[getter]
    fn measure_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner.measure).map_err(display_to_py)
    }

    #[getter]
    fn residual_risk(&self) -> f64 {
        self.inner.residual_risk
    }

    #[getter]
    fn factor_contributions(&self) -> Vec<PyFactorContribution> {
        self.inner
            .factor_contributions
            .iter()
            .cloned()
            .map(PyFactorContribution::from_inner)
            .collect()
    }

    #[getter]
    fn position_factor_contributions(&self) -> Vec<PyPositionFactorContribution> {
        self.inner
            .position_factor_contributions
            .iter()
            .cloned()
            .map(PyPositionFactorContribution::from_inner)
            .collect()
    }

    #[getter]
    fn position_residual_contributions(&self) -> Vec<PyPositionResidualContribution> {
        self.inner
            .position_residual_contributions
            .iter()
            .cloned()
            .map(PyPositionResidualContribution::from_inner)
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "RiskDecomposition(total_risk={}, residual_risk={}, factors={}, position_factors={}, position_residuals={})",
            self.inner.total_risk,
            self.inner.residual_risk,
            self.inner.factor_contributions.len(),
            self.inner.position_factor_contributions.len(),
            self.inner.position_residual_contributions.len(),
        )
    }
}

// ---------------------------------------------------------------------------
// PositionVarContribution / PositionEsContribution
// ---------------------------------------------------------------------------

/// Per-position component VaR and marginal VaR.
#[pyclass(
    name = "PositionVarContribution",
    module = "finstack.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyPositionVarContribution {
    pub(crate) inner: PositionVarContribution,
}

impl PyPositionVarContribution {
    fn from_inner(inner: PositionVarContribution) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPositionVarContribution {
    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: PositionVarContribution =
            serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn position_id(&self) -> String {
        self.inner.position_id.as_str().to_owned()
    }

    #[getter]
    fn component_var(&self) -> f64 {
        self.inner.component_var
    }

    #[getter]
    fn relative_var(&self) -> f64 {
        self.inner.relative_var
    }

    #[getter]
    fn marginal_var(&self) -> Option<f64> {
        self.inner.marginal_var
    }

    #[getter]
    fn incremental_var(&self) -> Option<f64> {
        self.inner.incremental_var
    }

    fn __repr__(&self) -> String {
        format!(
            "PositionVarContribution(position_id={:?}, component_var={}, relative_var={}, marginal_var={:?}, incremental_var={:?})",
            self.inner.position_id.as_str(),
            self.inner.component_var,
            self.inner.relative_var,
            self.inner.marginal_var,
            self.inner.incremental_var,
        )
    }
}

/// Per-position component ES and marginal ES.
#[pyclass(
    name = "PositionEsContribution",
    module = "finstack.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyPositionEsContribution {
    pub(crate) inner: PositionEsContribution,
}

impl PyPositionEsContribution {
    fn from_inner(inner: PositionEsContribution) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPositionEsContribution {
    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: PositionEsContribution =
            serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn position_id(&self) -> String {
        self.inner.position_id.as_str().to_owned()
    }

    #[getter]
    fn component_es(&self) -> f64 {
        self.inner.component_es
    }

    #[getter]
    fn relative_es(&self) -> f64 {
        self.inner.relative_es
    }

    #[getter]
    fn marginal_es(&self) -> Option<f64> {
        self.inner.marginal_es
    }

    fn __repr__(&self) -> String {
        format!(
            "PositionEsContribution(position_id={:?}, component_es={}, relative_es={}, marginal_es={:?})",
            self.inner.position_id.as_str(),
            self.inner.component_es,
            self.inner.relative_es,
            self.inner.marginal_es,
        )
    }
}

// ---------------------------------------------------------------------------
// PositionRiskDecomposition
// ---------------------------------------------------------------------------

/// Complete position-level risk decomposition.
#[pyclass(
    name = "PositionRiskDecomposition",
    module = "finstack.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyPositionRiskDecomposition {
    pub(crate) inner: PositionRiskDecomposition,
}

impl PyPositionRiskDecomposition {
    pub(crate) fn from_inner(inner: PositionRiskDecomposition) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPositionRiskDecomposition {
    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: PositionRiskDecomposition =
            serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn portfolio_var(&self) -> f64 {
        self.inner.portfolio_var
    }

    #[getter]
    fn portfolio_es(&self) -> f64 {
        self.inner.portfolio_es
    }

    #[getter]
    fn confidence(&self) -> f64 {
        self.inner.confidence
    }

    #[getter]
    fn n_positions(&self) -> usize {
        self.inner.n_positions
    }

    /// Decomposition method: ``"parametric"`` or ``"historical"``.
    #[getter]
    fn method(&self) -> &'static str {
        decomposition_method_label(self.inner.method)
    }

    /// Parametric-mode numerical residual; ``None`` in historical mode.
    #[getter]
    fn euler_residual(&self) -> Option<f64> {
        self.inner.euler_residual
    }

    #[getter]
    fn var_contributions(&self) -> Vec<PyPositionVarContribution> {
        self.inner
            .var_contributions
            .iter()
            .cloned()
            .map(PyPositionVarContribution::from_inner)
            .collect()
    }

    #[getter]
    fn es_contributions(&self) -> Vec<PyPositionEsContribution> {
        self.inner
            .es_contributions
            .iter()
            .cloned()
            .map(PyPositionEsContribution::from_inner)
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "PositionRiskDecomposition(portfolio_var={}, portfolio_es={}, confidence={}, n_positions={}, method={:?})",
            self.inner.portfolio_var,
            self.inner.portfolio_es,
            self.inner.confidence,
            self.inner.n_positions,
            decomposition_method_label(self.inner.method),
        )
    }
}

// ---------------------------------------------------------------------------
// PositionBudgetEntry / RiskBudgetResult
// ---------------------------------------------------------------------------

/// Per-position budget comparison entry.
#[pyclass(
    name = "PositionBudgetEntry",
    module = "finstack.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyPositionBudgetEntry {
    pub(crate) inner: PositionBudgetEntry,
}

impl PyPositionBudgetEntry {
    fn from_inner(inner: PositionBudgetEntry) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPositionBudgetEntry {
    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: PositionBudgetEntry = serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn position_id(&self) -> String {
        self.inner.position_id.as_str().to_owned()
    }

    #[getter]
    fn actual_component_var(&self) -> f64 {
        self.inner.actual_component_var
    }

    #[getter]
    fn target_component_var(&self) -> f64 {
        self.inner.target_component_var
    }

    #[getter]
    fn utilization(&self) -> f64 {
        self.inner.utilization
    }

    #[getter]
    fn excess(&self) -> f64 {
        self.inner.excess
    }

    fn __repr__(&self) -> String {
        format!(
            "PositionBudgetEntry(position_id={:?}, actual={}, target={}, utilization={}, excess={})",
            self.inner.position_id.as_str(),
            self.inner.actual_component_var,
            self.inner.target_component_var,
            self.inner.utilization,
            self.inner.excess,
        )
    }
}

/// Budget evaluation result across positions.
#[pyclass(
    name = "RiskBudgetResult",
    module = "finstack.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyRiskBudgetResult {
    pub(crate) inner: RiskBudgetResult,
}

impl PyRiskBudgetResult {
    pub(crate) fn from_inner(inner: RiskBudgetResult) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyRiskBudgetResult {
    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: RiskBudgetResult = serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn total_overbudget(&self) -> f64 {
        self.inner.total_overbudget
    }

    #[getter]
    fn has_breach(&self) -> bool {
        self.inner.has_breach
    }

    #[getter]
    fn positions(&self) -> Vec<PyPositionBudgetEntry> {
        self.inner
            .positions
            .iter()
            .cloned()
            .map(PyPositionBudgetEntry::from_inner)
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "RiskBudgetResult(positions={}, total_overbudget={}, has_breach={})",
            self.inner.positions.len(),
            self.inner.total_overbudget,
            self.inner.has_breach,
        )
    }
}

// ---------------------------------------------------------------------------
// FactorContributionDelta
// ---------------------------------------------------------------------------

/// Per-factor contribution change between a baseline and a scenario.
#[pyclass(
    name = "FactorContributionDelta",
    module = "finstack.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyFactorContributionDelta {
    pub(crate) inner: FactorContributionDelta,
}

impl PyFactorContributionDelta {
    fn from_inner(inner: FactorContributionDelta) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFactorContributionDelta {
    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: FactorContributionDelta =
            serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn factor_id(&self) -> String {
        self.inner.factor_id.as_str().to_owned()
    }

    #[getter]
    fn absolute_change(&self) -> f64 {
        self.inner.absolute_change
    }

    #[getter]
    fn relative_change(&self) -> f64 {
        self.inner.relative_change
    }

    fn __repr__(&self) -> String {
        format!(
            "FactorContributionDelta(factor_id={:?}, absolute_change={}, relative_change={})",
            self.inner.factor_id.as_str(),
            self.inner.absolute_change,
            self.inner.relative_change,
        )
    }
}

// ---------------------------------------------------------------------------
// WhatIfResult
// ---------------------------------------------------------------------------

/// Result of a position what-if scenario.
#[pyclass(
    name = "WhatIfResult",
    module = "finstack.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyWhatIfResult {
    pub(crate) inner: WhatIfResult,
}

impl PyWhatIfResult {
    pub(crate) fn from_inner(inner: WhatIfResult) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyWhatIfResult {
    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: WhatIfResult = serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn before(&self) -> PyRiskDecomposition {
        PyRiskDecomposition::from_inner(self.inner.before.clone())
    }

    #[getter]
    fn after(&self) -> PyRiskDecomposition {
        PyRiskDecomposition::from_inner(self.inner.after.clone())
    }

    #[getter]
    fn delta(&self) -> Vec<PyFactorContributionDelta> {
        self.inner
            .delta
            .iter()
            .cloned()
            .map(PyFactorContributionDelta::from_inner)
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "WhatIfResult(before_total={}, after_total={}, delta_entries={})",
            self.inner.before.total_risk,
            self.inner.after.total_risk,
            self.inner.delta.len(),
        )
    }
}

// ---------------------------------------------------------------------------
// StressResult
// ---------------------------------------------------------------------------

/// Result of a factor-stress scenario.
#[pyclass(
    name = "StressResult",
    module = "finstack.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyStressResult {
    pub(crate) inner: StressResult,
}

impl PyStressResult {
    pub(crate) fn from_inner(inner: StressResult) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyStressResult {
    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: StressResult = serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

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

    #[getter]
    fn stressed_decomposition(&self) -> PyRiskDecomposition {
        PyRiskDecomposition::from_inner(self.inner.stressed_decomposition.clone())
    }

    fn __repr__(&self) -> String {
        format!(
            "StressResult(total_pnl={}, positions={}, stressed_total_risk={})",
            self.inner.total_pnl,
            self.inner.position_pnl.len(),
            self.inner.stressed_decomposition.total_risk,
        )
    }
}

// ---------------------------------------------------------------------------
// StressPositionEntry / TailScenarioBreakdown / StressAttribution
// ---------------------------------------------------------------------------

/// Single position's contribution to tail stress.
#[pyclass(
    name = "StressPositionEntry",
    module = "finstack.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyStressPositionEntry {
    pub(crate) inner: StressPositionEntry,
}

impl PyStressPositionEntry {
    fn from_inner(inner: StressPositionEntry) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyStressPositionEntry {
    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: StressPositionEntry = serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn position_id(&self) -> String {
        self.inner.position_id.as_str().to_owned()
    }

    #[getter]
    fn avg_tail_pnl(&self) -> f64 {
        self.inner.avg_tail_pnl
    }

    #[getter]
    fn pct_of_tail_loss(&self) -> f64 {
        self.inner.pct_of_tail_loss
    }

    #[getter]
    fn worst_scenario_pnl(&self) -> f64 {
        self.inner.worst_scenario_pnl
    }

    fn __repr__(&self) -> String {
        format!(
            "StressPositionEntry(position_id={:?}, avg_tail_pnl={}, pct_of_tail_loss={}, worst_scenario_pnl={})",
            self.inner.position_id.as_str(),
            self.inner.avg_tail_pnl,
            self.inner.pct_of_tail_loss,
            self.inner.worst_scenario_pnl,
        )
    }
}

/// Breakdown of a single tail scenario.
#[pyclass(
    name = "TailScenarioBreakdown",
    module = "finstack.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyTailScenarioBreakdown {
    pub(crate) inner: TailScenarioBreakdown,
}

impl PyTailScenarioBreakdown {
    fn from_inner(inner: TailScenarioBreakdown) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyTailScenarioBreakdown {
    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: TailScenarioBreakdown = serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn scenario_index(&self) -> usize {
        self.inner.scenario_index
    }

    #[getter]
    fn portfolio_pnl(&self) -> f64 {
        self.inner.portfolio_pnl
    }

    /// Per-position ``(position_id, pnl)`` entries for this scenario.
    #[getter]
    fn position_pnls(&self) -> Vec<(String, f64)> {
        self.inner
            .position_pnls
            .iter()
            .map(|(id, pnl)| (id.as_str().to_owned(), *pnl))
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "TailScenarioBreakdown(scenario_index={}, portfolio_pnl={}, positions={})",
            self.inner.scenario_index,
            self.inner.portfolio_pnl,
            self.inner.position_pnls.len(),
        )
    }
}

/// Per-position attribution of portfolio losses in tail scenarios.
#[pyclass(
    name = "StressAttribution",
    module = "finstack.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyStressAttribution {
    pub(crate) inner: StressAttribution,
}

impl PyStressAttribution {
    pub(crate) fn from_inner(inner: StressAttribution) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyStressAttribution {
    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: StressAttribution = serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn var_threshold(&self) -> f64 {
        self.inner.var_threshold
    }

    #[getter]
    fn n_tail_scenarios(&self) -> usize {
        self.inner.n_tail_scenarios
    }

    #[getter]
    fn position_contributions(&self) -> Vec<PyStressPositionEntry> {
        self.inner
            .position_contributions
            .iter()
            .cloned()
            .map(PyStressPositionEntry::from_inner)
            .collect()
    }

    #[getter]
    fn tail_scenarios(&self) -> Vec<PyTailScenarioBreakdown> {
        self.inner
            .tail_scenarios
            .iter()
            .cloned()
            .map(PyTailScenarioBreakdown::from_inner)
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "StressAttribution(var_threshold={}, n_tail_scenarios={}, position_contributions={}, tail_scenarios={})",
            self.inner.var_threshold,
            self.inner.n_tail_scenarios,
            self.inner.position_contributions.len(),
            self.inner.tail_scenarios.len(),
        )
    }
}

// ---------------------------------------------------------------------------
// PositionAssignment / UnmatchedEntry / FactorAssignmentReport
// ---------------------------------------------------------------------------

/// Matched factor assignments for a single portfolio position.
///
/// The `mappings` field carries ``(MarketDependency, FactorId)`` pairs whose
/// dependency variant tree is wide enough that the binding exposes it as a
/// JSON-serialized vector via :meth:`mappings_json` rather than a fully
/// structured Python type.
#[pyclass(
    name = "PositionAssignment",
    module = "finstack.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyPositionAssignment {
    pub(crate) inner: PositionAssignment,
}

impl PyPositionAssignment {
    fn from_inner(inner: PositionAssignment) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPositionAssignment {
    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: PositionAssignment = serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn position_id(&self) -> String {
        self.inner.position_id.as_str().to_owned()
    }

    /// Number of matched ``(dependency, factor_id)`` pairs.
    #[getter]
    fn n_mappings(&self) -> usize {
        self.inner.mappings.len()
    }

    /// Matched ``(dependency, factor_id)`` pairs as a JSON string.
    #[pyo3(text_signature = "(self)")]
    fn mappings_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner.mappings).map_err(display_to_py)
    }

    /// Matched factor identifiers (in mapping order).
    #[getter]
    fn factor_ids(&self) -> Vec<String> {
        self.inner
            .mappings
            .iter()
            .map(|(_, fid)| fid.as_str().to_owned())
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "PositionAssignment(position_id={:?}, n_mappings={})",
            self.inner.position_id.as_str(),
            self.inner.mappings.len(),
        )
    }
}

/// Single unmatched dependency surfaced during assignment.
#[pyclass(
    name = "UnmatchedEntry",
    module = "finstack.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyUnmatchedEntry {
    pub(crate) inner: UnmatchedEntry,
}

impl PyUnmatchedEntry {
    fn from_inner(inner: UnmatchedEntry) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyUnmatchedEntry {
    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: UnmatchedEntry = serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn position_id(&self) -> String {
        self.inner.position_id.as_str().to_owned()
    }

    /// Unmatched dependency as a JSON string.
    #[pyo3(text_signature = "(self)")]
    fn dependency_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner.dependency).map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "UnmatchedEntry(position_id={:?})",
            self.inner.position_id.as_str(),
        )
    }
}

/// Assignment results for a portfolio-level factor mapping pass.
#[pyclass(
    name = "FactorAssignmentReport",
    module = "finstack.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyFactorAssignmentReport {
    pub(crate) inner: FactorAssignmentReport,
}

impl PyFactorAssignmentReport {
    pub(crate) fn from_inner(inner: FactorAssignmentReport) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFactorAssignmentReport {
    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: FactorAssignmentReport =
            serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn assignments(&self) -> Vec<PyPositionAssignment> {
        self.inner
            .assignments
            .iter()
            .cloned()
            .map(PyPositionAssignment::from_inner)
            .collect()
    }

    #[getter]
    fn unmatched(&self) -> Vec<PyUnmatchedEntry> {
        self.inner
            .unmatched
            .iter()
            .cloned()
            .map(PyUnmatchedEntry::from_inner)
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "FactorAssignmentReport(assignments={}, unmatched={})",
            self.inner.assignments.len(),
            self.inner.unmatched.len(),
        )
    }
}

// ---------------------------------------------------------------------------
// CreditVolReport / LevelVolContribution / PositionVolContribution
// (These types do not derive Serialize/Deserialize, so no JSON round-trip.)
// ---------------------------------------------------------------------------

/// Aggregated risk contribution for a single hierarchy level.
#[pyclass(
    name = "LevelVolContribution",
    module = "finstack.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyLevelVolContribution {
    pub(crate) inner: LevelVolContribution,
}

impl PyLevelVolContribution {
    fn from_inner(inner: LevelVolContribution) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyLevelVolContribution {
    #[getter]
    fn level_name(&self) -> String {
        self.inner.level_name.clone()
    }

    #[getter]
    fn total(&self) -> f64 {
        self.inner.total
    }

    /// Per-bucket contributions keyed by the bucket path.
    #[getter]
    fn by_bucket(&self) -> HashMap<String, f64> {
        self.inner
            .by_bucket
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "LevelVolContribution(level_name={:?}, total={}, buckets={})",
            self.inner.level_name,
            self.inner.total,
            self.inner.by_bucket.len(),
        )
    }
}

/// Per-position vol breakdown under :class:`CreditVolReport`.
#[pyclass(
    name = "PositionVolContribution",
    module = "finstack.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyPositionVolContribution {
    pub(crate) inner: PositionVolContribution,
}

impl PyPositionVolContribution {
    fn from_inner(inner: PositionVolContribution) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPositionVolContribution {
    #[getter]
    fn position_id(&self) -> String {
        self.inner.position_id.as_str().to_owned()
    }

    #[getter]
    fn factor_total(&self) -> f64 {
        self.inner.factor_total
    }

    #[getter]
    fn idiosyncratic(&self) -> f64 {
        self.inner.idiosyncratic
    }

    #[getter]
    fn total(&self) -> f64 {
        self.inner.total
    }

    fn __repr__(&self) -> String {
        format!(
            "PositionVolContribution(position_id={:?}, factor_total={}, idiosyncratic={}, total={})",
            self.inner.position_id.as_str(),
            self.inner.factor_total,
            self.inner.idiosyncratic,
            self.inner.total,
        )
    }
}

/// Aggregated vol report grouped by hierarchy level.
#[pyclass(
    name = "CreditVolReport",
    module = "finstack.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub struct PyCreditVolReport {
    pub(crate) inner: CreditVolReport,
}

#[pymethods]
impl PyCreditVolReport {
    #[getter]
    fn total(&self) -> f64 {
        self.inner.total
    }

    /// Risk measure serialized as a JSON-compatible string (e.g. ``"variance"``).
    #[getter]
    fn measure_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner.measure).map_err(display_to_py)
    }

    #[getter]
    fn generic(&self) -> f64 {
        self.inner.generic
    }

    #[getter]
    fn idiosyncratic_total(&self) -> f64 {
        self.inner.idiosyncratic_total
    }

    #[getter]
    fn by_level(&self) -> Vec<PyLevelVolContribution> {
        self.inner
            .by_level
            .iter()
            .cloned()
            .map(PyLevelVolContribution::from_inner)
            .collect()
    }

    /// Optional per-position breakdown; ``None`` when not requested.
    #[getter]
    fn by_position(&self) -> Option<Vec<PyPositionVolContribution>> {
        self.inner.by_position_optional.as_ref().map(|rows| {
            rows.iter()
                .cloned()
                .map(PyPositionVolContribution::from_inner)
                .collect()
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "CreditVolReport(total={}, generic={}, idiosyncratic_total={}, by_level={})",
            self.inner.total,
            self.inner.generic,
            self.inner.idiosyncratic_total,
            self.inner.by_level.len(),
        )
    }
}

// ---------------------------------------------------------------------------
// VolHorizon
// ---------------------------------------------------------------------------

/// Forecast horizon used to scale a calibrated `Sample` vol estimate.
///
/// Accepted Python constructors:
///   - ``VolHorizon.one_step()``
///   - ``VolHorizon.unconditional()``
///   - ``VolHorizon.n_steps(n)``
///   - ``VolHorizon.parse("one_step" | "unconditional" | '{"n_steps": N}')``
#[pyclass(
    name = "VolHorizon",
    module = "finstack.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone, Copy)]
pub struct PyVolHorizon {
    pub(crate) inner: VolHorizon,
}

impl PyVolHorizon {
    pub(crate) fn from_inner(inner: VolHorizon) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyVolHorizon {
    #[classmethod]
    #[pyo3(text_signature = "(cls)")]
    fn one_step(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(VolHorizon::OneStep)
    }

    #[classmethod]
    #[pyo3(text_signature = "(cls)")]
    fn unconditional(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(VolHorizon::Unconditional)
    }

    #[classmethod]
    #[pyo3(text_signature = "(cls, n)")]
    fn n_steps(_cls: &Bound<'_, PyType>, n: usize) -> Self {
        Self::from_inner(VolHorizon::NSteps(n))
    }

    /// Parse a horizon descriptor string (matches the Rust ``VolHorizon::parse``).
    #[classmethod]
    #[pyo3(text_signature = "(cls, s)")]
    fn parse(_cls: &Bound<'_, PyType>, s: &str) -> PyResult<Self> {
        VolHorizon::parse(s)
            .map(Self::from_inner)
            .map_err(PyValueError::new_err)
    }

    /// Variant label: ``"one_step"`` / ``"unconditional"`` / ``"n_steps"``.
    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner {
            VolHorizon::OneStep => "one_step",
            VolHorizon::Unconditional => "unconditional",
            VolHorizon::NSteps(_) => "n_steps",
        }
    }

    /// Step count when ``kind == "n_steps"``, ``None`` otherwise.
    #[getter]
    fn n(&self) -> Option<usize> {
        match self.inner {
            VolHorizon::NSteps(n) => Some(n),
            _ => None,
        }
    }

    fn __repr__(&self) -> String {
        match self.inner {
            VolHorizon::OneStep => "VolHorizon.one_step()".to_owned(),
            VolHorizon::Unconditional => "VolHorizon.unconditional()".to_owned(),
            VolHorizon::NSteps(n) => format!("VolHorizon.n_steps({n})"),
        }
    }
}

// ---------------------------------------------------------------------------
// DecompositionConfig
// ---------------------------------------------------------------------------

/// Configuration for position-level VaR decomposition.
#[pyclass(
    name = "DecompositionConfig",
    module = "finstack.portfolio",
    from_py_object
)]
#[derive(Clone)]
pub struct PyDecompositionConfig {
    pub(crate) inner: DecompositionConfig,
}

impl PyDecompositionConfig {
    pub(crate) fn from_inner(inner: DecompositionConfig) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyDecompositionConfig {
    /// Standard 95% parametric configuration.
    #[classmethod]
    #[pyo3(text_signature = "(cls)")]
    fn parametric_95(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(DecompositionConfig::parametric_95())
    }

    /// Standard 99% parametric configuration.
    #[classmethod]
    #[pyo3(text_signature = "(cls)")]
    fn parametric_99(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(DecompositionConfig::parametric_99())
    }

    /// Historical-mode configuration at the given confidence.
    #[classmethod]
    #[pyo3(text_signature = "(cls, confidence)")]
    fn historical(_cls: &Bound<'_, PyType>, confidence: f64) -> Self {
        Self::from_inner(DecompositionConfig::historical(confidence))
    }

    /// Enable incremental VaR computation (expensive).
    #[pyo3(text_signature = "(self)")]
    fn with_incremental(&self) -> Self {
        Self::from_inner(self.inner.clone().with_incremental())
    }

    /// Pin the RNG seed for simulation-path decompositions.
    #[pyo3(text_signature = "(self, seed)")]
    fn with_seed(&self, seed: u64) -> Self {
        Self::from_inner(self.inner.clone().with_seed(seed))
    }

    #[getter]
    fn confidence(&self) -> f64 {
        self.inner.confidence
    }

    #[getter]
    fn method(&self) -> &'static str {
        decomposition_method_label(self.inner.method)
    }

    #[getter]
    fn compute_incremental(&self) -> bool {
        self.inner.compute_incremental
    }

    #[getter]
    fn seed(&self) -> Option<u64> {
        self.inner.seed
    }

    fn __repr__(&self) -> String {
        format!(
            "DecompositionConfig(confidence={}, method={:?}, compute_incremental={}, seed={:?})",
            self.inner.confidence,
            decomposition_method_label(self.inner.method),
            self.inner.compute_incremental,
            self.inner.seed,
        )
    }
}

// ---------------------------------------------------------------------------
// Typed sibling functions
//
// These mirror the dict-returning helpers in `position_risk.rs` but return
// the typed result classes above. The original dict-returning entry points
// in `super::position_risk` remain unchanged for backwards compatibility.
// ---------------------------------------------------------------------------

/// Decompose portfolio VaR/ES into position contributions via parametric
/// Euler allocation, returning a typed :class:`PositionRiskDecomposition`.
#[pyfunction]
#[pyo3(signature = (position_ids, weights, covariance, confidence = 0.95, compute_incremental = false))]
fn parametric_var_decomposition_typed(
    position_ids: Vec<String>,
    weights: Vec<f64>,
    covariance: Vec<Vec<f64>>,
    confidence: f64,
    compute_incremental: bool,
) -> PyResult<PyPositionRiskDecomposition> {
    let n = weights.len();
    let cov_flat = flatten_square_matrix(covariance, n, "covariance")?;
    let ids = to_position_ids(position_ids);

    let mut config = DecompositionConfig::parametric_95();
    config.confidence = confidence;
    if compute_incremental {
        config = config.with_incremental();
    }

    let result = ParametricPositionDecomposer
        .decompose_positions(&weights, &cov_flat, &ids, &config)
        .map_err(core_to_py)?;

    Ok(PyPositionRiskDecomposition::from_inner(result))
}

/// Decompose portfolio VaR and ES from per-position scenario P&Ls via
/// historical simulation, returning a typed :class:`PositionRiskDecomposition`.
#[pyfunction]
#[pyo3(signature = (position_ids, position_pnls, confidence = 0.95))]
fn historical_var_decomposition_typed(
    position_ids: Vec<String>,
    position_pnls: Vec<Vec<f64>>,
    confidence: f64,
) -> PyResult<PyPositionRiskDecomposition> {
    let n = position_ids.len();
    if position_pnls.len() != n {
        return Err(PyValueError::new_err(format!(
            "position_pnls must have {n} rows (one per position), got {}",
            position_pnls.len()
        )));
    }
    if n == 0 {
        let ids = to_position_ids(position_ids);
        let config = DecompositionConfig::historical(confidence);
        let result = HistoricalPositionDecomposer
            .decompose_from_pnls(&[], &ids, 0, &config)
            .map_err(core_to_py)?;
        return Ok(PyPositionRiskDecomposition::from_inner(result));
    }

    let n_scenarios = position_pnls[0].len();
    for (i, row) in position_pnls.iter().enumerate() {
        if row.len() != n_scenarios {
            return Err(PyValueError::new_err(format!(
                "position_pnls row {i} has {} scenarios, expected {n_scenarios}",
                row.len()
            )));
        }
    }

    // Transpose to row-major scenarios x positions layout expected by the engine.
    let mut flat = Vec::with_capacity(n_scenarios * n);
    for s in 0..n_scenarios {
        for row in &position_pnls {
            flat.push(row[s]);
        }
    }

    let ids = to_position_ids(position_ids);
    let config = DecompositionConfig::historical(confidence);

    let result = HistoricalPositionDecomposer
        .decompose_from_pnls(&flat, &ids, n_scenarios, &config)
        .map_err(core_to_py)?;

    Ok(PyPositionRiskDecomposition::from_inner(result))
}

/// Evaluate a per-position risk budget against actual component VaRs,
/// returning a typed :class:`RiskBudgetResult`.
#[pyfunction]
#[pyo3(signature = (position_ids, actual_var, target_var_pct, portfolio_var, utilization_threshold = 1.20))]
fn evaluate_risk_budget_typed(
    position_ids: Vec<String>,
    actual_var: Vec<f64>,
    target_var_pct: Vec<f64>,
    portfolio_var: f64,
    utilization_threshold: f64,
) -> PyResult<PyRiskBudgetResult> {
    let n = position_ids.len();
    if actual_var.len() != n {
        return Err(PyValueError::new_err(format!(
            "actual_var length ({}) must match position_ids length ({n})",
            actual_var.len()
        )));
    }
    if target_var_pct.len() != n {
        return Err(PyValueError::new_err(format!(
            "target_var_pct length ({}) must match position_ids length ({n})",
            target_var_pct.len()
        )));
    }

    let shared_ids: Vec<PositionId> = position_ids.into_iter().map(PositionId::new).collect();

    let mut targets: IndexMap<PositionId, f64> = IndexMap::with_capacity(n);
    for (id, &pct) in shared_ids.iter().zip(target_var_pct.iter()) {
        targets.insert(id.clone(), pct);
    }
    let budget = RiskBudget::new(targets).with_threshold(utilization_threshold);
    let result = budget
        .evaluate_components(
            shared_ids.iter().zip(actual_var.iter().copied()),
            portfolio_var,
        )
        .map_err(core_to_py)?;

    Ok(PyRiskBudgetResult::from_inner(result))
}

/// Look up a position by id inside a :class:`PositionRiskDecomposition` and
/// return its component VaR. Raises ``KeyError`` if absent.
#[pyfunction]
#[pyo3(signature = (decomp, position_id))]
fn position_component_var(
    decomp: &PyPositionRiskDecomposition,
    position_id: &str,
) -> PyResult<f64> {
    decomp
        .inner
        .var_contributions
        .iter()
        .find(|c| c.position_id.as_str() == position_id)
        .map(|c| c.component_var)
        .ok_or_else(|| {
            PyKeyError::new_err(format!("position '{position_id}' not in decomposition"))
        })
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register factor_model typed result classes and typed-sibling functions on
/// the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFactorContribution>()?;
    m.add_class::<PyPositionFactorContribution>()?;
    m.add_class::<PyPositionResidualContribution>()?;
    m.add_class::<PyRiskDecomposition>()?;
    m.add_class::<PyPositionVarContribution>()?;
    m.add_class::<PyPositionEsContribution>()?;
    m.add_class::<PyPositionRiskDecomposition>()?;
    m.add_class::<PyPositionBudgetEntry>()?;
    m.add_class::<PyRiskBudgetResult>()?;
    m.add_class::<PyFactorContributionDelta>()?;
    m.add_class::<PyWhatIfResult>()?;
    m.add_class::<PyStressResult>()?;
    m.add_class::<PyStressPositionEntry>()?;
    m.add_class::<PyTailScenarioBreakdown>()?;
    m.add_class::<PyStressAttribution>()?;
    m.add_class::<PyPositionAssignment>()?;
    m.add_class::<PyUnmatchedEntry>()?;
    m.add_class::<PyFactorAssignmentReport>()?;
    m.add_class::<PyLevelVolContribution>()?;
    m.add_class::<PyPositionVolContribution>()?;
    m.add_class::<PyCreditVolReport>()?;
    m.add_class::<PyVolHorizon>()?;
    m.add_class::<PyDecompositionConfig>()?;

    m.add_function(wrap_pyfunction!(parametric_var_decomposition_typed, m)?)?;
    m.add_function(wrap_pyfunction!(historical_var_decomposition_typed, m)?)?;
    m.add_function(wrap_pyfunction!(evaluate_risk_budget_typed, m)?)?;
    m.add_function(wrap_pyfunction!(position_component_var, m)?)?;

    Ok(())
}
