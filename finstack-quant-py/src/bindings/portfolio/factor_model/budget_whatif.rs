use crate::bindings::module_utils::py_to_serde;
use crate::bindings::pandas_utils::dict_to_dataframe;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use finstack_quant_models::factor::risk::{
    self as model_risk, PositionBudgetEntry, RiskBudgetResult,
};
use finstack_quant_portfolio::factor_model::{self as fm, FactorContributionDelta, WhatIfResult};

use crate::bindings::extract::{extract_market_ref, extract_portfolio_ref};
use crate::errors::portfolio_to_py;

use super::super::json_bridge::{deserialize_json, serialize_json};
use super::contributions::PyRiskDecomposition;

/// Deserialize what-if position changes straight into the canonical Rust
/// `PositionChange` wire shape (`{"kind": "remove" | "resize", ...}`).
fn parse_position_changes(
    py: Python<'_>,
    changes: &Bound<'_, PyAny>,
) -> PyResult<Vec<fm::PositionChange>> {
    py_to_serde(py, changes, "position changes")
}

/// Per-position budget comparison entry.
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

#[pymethods]
impl PyPositionBudgetEntry {
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
        let inner: PositionBudgetEntry = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

    /// Portfolio position identifier.
    #[getter]
    fn position_id(&self) -> String {
        self.inner.position_id.as_str().to_owned()
    }

    /// Realized component VaR for this position, taken from the decomposition
    /// (portfolio currency, loss convention).
    #[getter]
    fn actual_component_var(&self) -> f64 {
        self.inner.actual_component_var
    }

    /// Budgeted component VaR for this position, in the same units as
    /// :attr:`actual_component_var`.
    #[getter]
    fn target_component_var(&self) -> f64 {
        self.inner.target_component_var
    }

    /// Utilization ratio ``actual / target``.
    ///
    /// Returns
    /// -------
    /// float
    ///     A **ratio**, not a percentage: ``1.0`` is exactly on budget, above
    ///     ``1.0`` uses more risk than budgeted, below ``1.0`` leaves budget
    ///     unused.
    #[getter]
    fn utilization(&self) -> f64 {
        self.inner.utilization
    }

    /// Over/under-budget amount ``actual - target``; positive means over budget.
    #[getter]
    fn excess(&self) -> f64 {
        self.inner.excess
    }

    /// Export this budget entry as a single-row pandas ``DataFrame``.
    ///
    /// Columns: ``position_id``, ``actual_component_var``,
    /// ``target_component_var``, ``utilization``, ``excess``.
    ///
    /// Built from the typed fields rather than the wire form so a non-finite
    /// ``utilization`` (zero-target breach) stays a float ``inf`` column
    /// value instead of the JSON ``"inf"`` string sentinel.
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let data = PyDict::new(py);
        data.set_item("position_id", vec![self.inner.position_id.as_str()])?;
        data.set_item(
            "actual_component_var",
            vec![self.inner.actual_component_var],
        )?;
        data.set_item(
            "target_component_var",
            vec![self.inner.target_component_var],
        )?;
        data.set_item("utilization", vec![self.inner.utilization])?;
        data.set_item("excess", vec![self.inner.excess])?;
        dict_to_dataframe(py, &data, None)
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

/// Budget evaluation result across positions.
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
    fn from_inner(inner: RiskBudgetResult) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyRiskBudgetResult {
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
        let inner: RiskBudgetResult = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

    /// Total absolute over-budget amount summed across exceeding positions.
    #[getter]
    fn total_overbudget(&self) -> f64 {
        self.inner.total_overbudget
    }

    /// Whether any position exceeds the configured utilization threshold.
    #[getter]
    fn has_breach(&self) -> bool {
        self.inner.has_breach
    }

    /// Per-position budget comparison entries.
    #[getter]
    fn positions(&self) -> Vec<PyPositionBudgetEntry> {
        self.inner
            .positions
            .iter()
            .cloned()
            .map(PyPositionBudgetEntry::from_inner)
            .collect()
    }

    /// Export the per-position budget comparison as a pandas ``DataFrame``.
    ///
    /// One row per entry of :attr:`positions`. The scalars
    /// :attr:`total_overbudget` and :attr:`has_breach` are header metadata and
    /// are not repeated per row.
    ///
    /// Columns: ``position_id``, ``actual_component_var``,
    /// ``target_component_var``, ``utilization`` (ratio, not percentage),
    /// ``excess``.
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = &self.inner.positions;
        let position_ids: Vec<&str> = rows.iter().map(|e| e.position_id.as_str()).collect();
        let actual: Vec<f64> = rows.iter().map(|e| e.actual_component_var).collect();
        let target: Vec<f64> = rows.iter().map(|e| e.target_component_var).collect();
        let utilization: Vec<f64> = rows.iter().map(|e| e.utilization).collect();
        let excess: Vec<f64> = rows.iter().map(|e| e.excess).collect();
        let data = PyDict::new(py);
        data.set_item("position_id", position_ids)?;
        data.set_item("actual_component_var", actual)?;
        data.set_item("target_component_var", target)?;
        data.set_item("utilization", utilization)?;
        data.set_item("excess", excess)?;
        dict_to_dataframe(py, &data, None)
    }

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

/// Per-factor contribution change between a baseline and a scenario.
#[pyclass(
    name = "FactorContributionDelta",
    module = "finstack_quant.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyFactorContributionDelta {
    pub(crate) inner: FactorContributionDelta,
}

impl PyFactorContributionDelta {
    fn from_inner(inner: FactorContributionDelta) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFactorContributionDelta {
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
        let inner: FactorContributionDelta = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

    /// Identifier of the factor whose contribution changed.
    #[getter]
    fn factor_id(&self) -> String {
        self.inner.factor_id.as_str().to_owned()
    }

    /// ``after - before`` change in the factor's absolute risk contribution, in
    /// the units of the decomposition's risk measure.
    #[getter]
    fn absolute_change(&self) -> f64 {
        self.inner.absolute_change
    }

    /// ``after - before`` change in the factor's relative (fractional) risk
    /// contribution; dimensionless, not a percentage.
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

/// Result of a position what-if scenario.
#[pyclass(
    name = "WhatIfResult",
    module = "finstack_quant.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyWhatIfResult {
    pub(crate) inner: WhatIfResult,
}

impl PyWhatIfResult {
    fn from_inner(inner: WhatIfResult) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyWhatIfResult {
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
        let inner: WhatIfResult = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

    /// Baseline risk decomposition used as the comparison point.
    ///
    /// Use its own frame accessors (:meth:`RiskDecomposition.to_factor_dataframe`
    /// and friends) for a tabular view of the baseline.
    #[getter]
    fn before(&self) -> PyRiskDecomposition {
        PyRiskDecomposition::from_inner(self.inner.before.clone())
    }

    /// Risk decomposition after applying the requested position changes.
    #[getter]
    fn after(&self) -> PyRiskDecomposition {
        PyRiskDecomposition::from_inner(self.inner.after.clone())
    }

    /// Per-factor ``after - before`` contribution changes.
    #[getter]
    fn delta(&self) -> Vec<PyFactorContributionDelta> {
        self.inner
            .delta
            .iter()
            .cloned()
            .map(PyFactorContributionDelta::from_inner)
            .collect()
    }

    /// Export the per-factor contribution changes as a pandas ``DataFrame``.
    ///
    /// One row per entry of :attr:`delta`. The baseline and scenario
    /// decompositions are not flattened here — reach them through
    /// :attr:`before` / :attr:`after` and their own frame accessors.
    ///
    /// Columns: ``factor_id``, ``absolute_change`` (risk-measure units),
    /// ``relative_change`` (dimensionless fraction, not a percentage).
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rows = &self.inner.delta;
        let factor_ids: Vec<&str> = rows.iter().map(|d| d.factor_id.as_str()).collect();
        let absolute_change: Vec<f64> = rows.iter().map(|d| d.absolute_change).collect();
        let relative_change: Vec<f64> = rows.iter().map(|d| d.relative_change).collect();
        let data = PyDict::new(py);
        data.set_item("factor_id", factor_ids)?;
        data.set_item("absolute_change", absolute_change)?;
        data.set_item("relative_change", relative_change)?;
        dict_to_dataframe(py, &data, None)
    }

    fn __repr__(&self) -> String {
        format!(
            "WhatIfResult(before_total={}, after_total={}, delta_entries={})",
            self.inner.before.total_risk,
            self.inner.after.total_risk,
            self.inner.delta.len(),
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

// The literal default below must stay equal to the Rust canonical constant so
// the generated text signature shows the real number.
const _: () = assert!(model_risk::DEFAULT_UTILIZATION_THRESHOLD == 1.2);

/// Evaluate a per-position risk budget against actual component VaRs,
/// returning a typed ``RiskBudgetResult``.
///
/// Args:
///     position_ids: Position identifiers aligned with ``actual_var`` and
///         ``target_var_pct``; duplicates are rejected.
///     actual_var: Realized component VaR per position (portfolio currency,
///         loss convention).
///     target_var_pct: Target share of portfolio VaR per position, as
///         fractions that sum to one.
///     portfolio_var: Portfolio VaR whose magnitude scales each target share
///         into a target component VaR.
///     utilization_threshold: Ratio ``actual / target`` above which a position
///         is flagged as breaching; defaults to ``DEFAULT_UTILIZATION_THRESHOLD``
///         (``1.2``).
///
/// Returns:
///     ``RiskBudgetResult`` with per-position utilization, excess and breach
///     flags.
///
/// Raises:
///     ValueError: If array lengths differ, a position id is duplicated,
///         non-empty target shares do not sum to one, or a non-zero component
///         is paired with a zero ``portfolio_var``.
#[pyfunction]
#[pyo3(signature = (position_ids, actual_var, target_var_pct, portfolio_var, utilization_threshold = 1.2))]
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
            model_risk::evaluate_risk_budget_arrays(
                position_ids,
                &actual_var,
                &target_var_pct,
                portfolio_var,
                utilization_threshold,
            )
        })
        .map_err(crate::errors::core_to_py)?;

    Ok(PyRiskBudgetResult::from_inner(result))
}

/// Run position remove/resize what-if analysis from a factor-model config.
///
/// ``as_of`` accepts either a date-like object (``datetime.date``,
/// ``pandas.Timestamp``) or an ISO 8601 string.
#[pyfunction]
#[pyo3(signature = (portfolio, market, factor_model_config_json, as_of, changes))]
pub(super) fn position_what_if(
    py: Python<'_>,
    portfolio: &Bound<'_, PyAny>,
    market: &Bound<'_, PyAny>,
    factor_model_config_json: &str,
    as_of: &Bound<'_, PyAny>,
    changes: &Bound<'_, PyAny>,
) -> PyResult<PyWhatIfResult> {
    let portfolio = extract_portfolio_ref(py, portfolio)?;
    let market = extract_market_ref(py, market)?;
    let as_of = crate::bindings::date_utils::extract_date(as_of)?;
    let config_json = factor_model_config_json.to_owned();
    let config: finstack_quant_models::factor::FactorModelConfig = py
        .detach(move || serde_json::from_str(&config_json))
        .map_err(crate::errors::display_to_py)?;
    let changes = parse_position_changes(py, changes)?;

    let portfolio_ref: &finstack_quant_portfolio::Portfolio = &portfolio;
    let market_ref: &finstack_quant_core::market_data::context::MarketContext = &market;
    let result = py
        .detach(move || {
            let model = fm::FactorModelBuilder::new().config(config).build()?;
            let (base, sensitivities) =
                model.analyze_with_sensitivities(portfolio_ref, market_ref, as_of)?;
            model
                .what_if(&base, &sensitivities, portfolio_ref, market_ref, as_of)
                .position_what_if(&changes)
        })
        .map_err(portfolio_to_py)?;

    Ok(PyWhatIfResult::from_inner(result))
}
