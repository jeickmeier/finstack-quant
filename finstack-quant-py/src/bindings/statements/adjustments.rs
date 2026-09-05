//! Python wrappers for EBITDA normalization and adjustments.

use super::evaluator::PyStatementResult;
use crate::errors::{serde_json_to_py, statements_to_py, value_error};
use finstack_quant_statements::adjustments::types::{
    Adjustment, AdjustmentValue, AppliedAdjustment, CapBaseMode, NormalizationConfig,
    NormalizationResult,
};
use pyo3::prelude::*;

/// One add-back or deduction applied while normalizing a target metric.
///
/// Build with :meth:`fixed` (explicit per-period amounts) or
/// :meth:`percentage` (a fraction of another node), then optionally cap it
/// with :meth:`with_cap` / :meth:`with_cap_mode`.
#[pyclass(
    name = "Adjustment",
    module = "finstack_quant.statements",
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyAdjustment {
    pub(super) inner: Adjustment,
}

/// Parse a cap base mode discriminant (``"reported"`` / ``"progressive"``).
fn parse_cap_base_mode(mode: &str) -> PyResult<CapBaseMode> {
    finstack_quant_core::wire::serde_parse(mode).map_err(|e| {
        value_error(format!(
            "invalid cap base mode {mode:?}: {e}; expected reported or progressive"
        ))
    })
}

#[pymethods]
impl PyAdjustment {
    /// Fixed per-period adjustment.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique adjustment identifier within a configuration.
    /// name : str
    ///     Human-readable name (``"Synergies"``, ``"Management fees"``).
    /// amounts : Mapping[str, float] | Sequence[tuple[str, float]] | pd.Series
    ///     Period id to signed amount in the target metric's units
    ///     (positive add-back, negative deduction). Periods without an
    ///     entry receive no adjustment.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a period id does not parse or an amount is not numeric.
    #[staticmethod]
    #[pyo3(text_signature = "(id, name, amounts)")]
    fn fixed(id: &str, name: &str, amounts: &Bound<'_, PyAny>) -> PyResult<Self> {
        let amounts = super::extract_scalar_series(amounts)?.into_iter().collect();
        Ok(Self {
            inner: Adjustment::fixed(id, name, amounts),
        })
    }

    /// Adjustment sized as a fraction of another node's value each period.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique adjustment identifier within a configuration.
    /// name : str
    ///     Human-readable name.
    /// node_id : str
    ///     Reference node (``"revenue"``) whose per-period value is scaled.
    /// percentage : float
    ///     Fraction as a **decimal** (``0.05`` = 5%), signed: negative for a
    ///     deduction.
    #[staticmethod]
    #[pyo3(text_signature = "(id, name, node_id, percentage)")]
    fn percentage(id: &str, name: &str, node_id: &str, percentage: f64) -> Self {
        Self {
            inner: Adjustment::percentage(id, name, node_id, percentage),
        }
    }

    /// Return a copy of this adjustment with a cap (default ``"reported"``
    /// base mode).
    ///
    /// Parameters
    /// ----------
    /// base_node : str | None
    ///     Node the cap is measured against (``"ebitda"``); ``None`` makes
    ///     ``value`` an absolute amount.
    /// value : float
    ///     Cap as a decimal fraction of ``base_node`` (``0.20`` = 20%), or
    ///     an absolute amount in the metric's units when ``base_node`` is
    ///     ``None``.
    ///
    /// Returns
    /// -------
    /// Adjustment
    ///     New adjustment carrying the cap; the original is unchanged.
    #[pyo3(text_signature = "($self, base_node, value)")]
    fn with_cap(&self, base_node: Option<String>, value: f64) -> Self {
        Self {
            inner: self.inner.clone().with_cap(base_node, value),
        }
    }

    /// Return a copy with a cap and an explicit self-referential base mode.
    ///
    /// Parameters
    /// ----------
    /// base_node : str | None
    ///     Node the cap is measured against; ``None`` makes ``value``
    ///     absolute.
    /// value : float
    ///     Cap as a decimal fraction of ``base_node`` or an absolute amount.
    /// base_mode : str
    ///     ``"reported"`` (cap against the pre-adjustment base — the
    ///     credit-agreement convention) or ``"progressive"`` (cap against the
    ///     base plus earlier adjustments). Only matters when ``base_node``
    ///     is the target node itself.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``base_mode`` is not ``"reported"`` or ``"progressive"``.
    #[pyo3(text_signature = "($self, base_node, value, base_mode)")]
    fn with_cap_mode(
        &self,
        base_node: Option<String>,
        value: f64,
        base_mode: &str,
    ) -> PyResult<Self> {
        let mode = parse_cap_base_mode(base_mode)?;
        Ok(Self {
            inner: self.inner.clone().with_cap_mode(base_node, value, mode),
        })
    }

    /// Return a copy with a grouping category (``"one_time"``, ``"run_rate"``).
    #[pyo3(text_signature = "($self, category)")]
    fn with_category(&self, category: &str) -> Self {
        let mut inner = self.inner.clone();
        inner.category = Some(category.to_string());
        Self { inner }
    }

    /// Support `pickle` via the canonical JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize an adjustment from canonical JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid Adjustment JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize this adjustment to canonical JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize Adjustment"))
    }

    /// Adjustment identifier.
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    /// Human-readable name.
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    /// Grouping category, or ``None``.
    #[getter]
    fn category(&self) -> Option<&str> {
        self.inner.category.as_deref()
    }

    /// How the amount is derived: ``"fixed"`` or ``"percentage_of_node"``.
    #[getter]
    fn value_type(&self) -> &'static str {
        match self.inner.value {
            AdjustmentValue::Fixed { .. } => "fixed",
            AdjustmentValue::PercentageOfNode { .. } => "percentage_of_node",
        }
    }

    /// Whether a cap is configured.
    #[getter]
    fn has_cap(&self) -> bool {
        self.inner.cap.is_some()
    }

    /// Cap base node, or ``None`` (absolute cap or no cap).
    #[getter]
    fn cap_base_node(&self) -> Option<String> {
        self.inner
            .cap
            .as_ref()
            .and_then(|cap| cap.base_node.clone())
    }

    /// Cap value (decimal fraction of the base node, or absolute amount),
    /// or ``None`` when no cap is configured.
    #[getter]
    fn cap_value(&self) -> Option<f64> {
        self.inner.cap.as_ref().map(|cap| cap.value)
    }

    /// Cap base mode (``"reported"`` / ``"progressive"``), or ``None``.
    #[getter]
    fn cap_base_mode(&self) -> Option<String> {
        self.inner
            .cap
            .as_ref()
            .map(|cap| crate::bindings::statements_analytics::serde_variant_str(&cap.base_mode))
    }

    /// Return ``Adjustment(id=..., name=..., value_type=..., has_cap=...)``.
    fn __repr__(&self) -> String {
        format!(
            "Adjustment(id={:?}, name={:?}, value_type={:?}, has_cap={})",
            self.inner.id,
            self.inner.name,
            self.value_type(),
            if self.inner.cap.is_some() {
                "True"
            } else {
                "False"
            }
        )
    }
}

/// One adjustment applied while normalizing a statement metric.
#[pyclass(
    name = "AppliedAdjustment",
    module = "finstack_quant.statements",
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyAppliedAdjustment {
    inner: AppliedAdjustment,
}

#[pymethods]
impl PyAppliedAdjustment {
    /// Support `pickle` via the canonical JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize an applied adjustment from canonical JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid AppliedAdjustment JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize this applied adjustment to canonical JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize AppliedAdjustment"))
    }

    /// Stable adjustment identifier.
    #[getter]
    fn adjustment_id(&self) -> &str {
        &self.inner.adjustment_id
    }

    /// Human-readable adjustment name.
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    /// Calculated amount before applying a cap.
    #[getter]
    fn raw_amount(&self) -> f64 {
        self.inner.raw_amount
    }

    /// Amount included after applying any cap.
    #[getter]
    fn capped_amount(&self) -> f64 {
        self.inner.capped_amount
    }

    /// Whether a cap changed the calculated amount.
    #[getter]
    fn is_capped(&self) -> bool {
        self.inner.is_capped
    }

    /// Return a concise diagnostic representation.
    fn __repr__(&self) -> String {
        format!(
            "AppliedAdjustment(id={:?}, raw_amount={}, capped_amount={}, is_capped={})",
            self.inner.adjustment_id,
            self.inner.raw_amount,
            self.inner.capped_amount,
            if self.inner.is_capped {
                "True"
            } else {
                "False"
            }
        )
    }
}

/// Normalized metric result for one reporting period.
#[pyclass(
    name = "NormalizationResult",
    module = "finstack_quant.statements",
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyNormalizationResult {
    inner: NormalizationResult,
}

#[pymethods]
impl PyNormalizationResult {
    /// Deserialize a normalization result from compact JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid NormalizationResult JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize this result to compact JSON.
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize NormalizationResult"))
    }

    /// Support pickle through the canonical JSON representation.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Reporting period identifier such as ``"2025Q1"``.
    #[getter]
    fn period(&self) -> String {
        self.inner.period.to_string()
    }

    /// Unit classification of the base, adjustments, and final value.
    #[getter]
    fn value_type(&self) -> &'static str {
        match self.inner.value_type {
            finstack_quant_statements::types::NodeValueType::Monetary { .. } => "monetary",
            finstack_quant_statements::types::NodeValueType::Scalar => "scalar",
        }
    }

    /// ISO currency of every amount in this normalization result, or None for scalars.
    #[getter]
    fn currency(&self) -> Option<String> {
        match self.inner.value_type {
            finstack_quant_statements::types::NodeValueType::Monetary { currency } => {
                Some(currency.to_string())
            }
            finstack_quant_statements::types::NodeValueType::Scalar => None,
        }
    }

    /// Reported value before normalization adjustments.
    #[getter]
    fn base_value(&self) -> f64 {
        self.inner.base_value
    }

    /// Applied adjustments in declaration order.
    #[getter]
    fn adjustments(&self) -> Vec<PyAppliedAdjustment> {
        self.inner
            .adjustments
            .iter()
            .cloned()
            .map(|inner| PyAppliedAdjustment { inner })
            .collect()
    }

    /// Value after all capped adjustments.
    #[getter]
    fn final_value(&self) -> f64 {
        self.inner.final_value
    }

    /// Return a concise diagnostic representation.
    fn __repr__(&self) -> String {
        format!(
            "NormalizationResult(period={:?}, base_value={}, final_value={}, adjustments={})",
            self.inner.period.to_string(),
            self.inner.base_value,
            self.inner.final_value,
            self.inner.adjustments.len()
        )
    }
}

/// Configuration for normalizing a financial metric.
#[pyclass(
    name = "NormalizationConfig",
    module = "finstack_quant.statements",
    eq,
    skip_from_py_object
)]
#[derive(Clone, PartialEq)]
pub struct PyNormalizationConfig {
    pub(super) inner: NormalizationConfig,
}

#[pymethods]
impl PyNormalizationConfig {
    /// Create a normalization configuration for a target node.
    ///
    /// Starts with no adjustments; add them with :meth:`add_adjustment`.
    ///
    /// Parameters
    /// ----------
    /// target_node : str
    ///     Node identifier of the metric to normalize (e.g. ``"ebitda"``).
    #[new]
    #[pyo3(text_signature = "(target_node)")]
    fn new(target_node: &str) -> Self {
        Self {
            inner: NormalizationConfig::new(target_node),
        }
    }

    /// Append an adjustment, returning this configuration for chaining.
    ///
    /// Order matters for progressive self-referential caps, so adjustments
    /// are applied in the order added.
    ///
    /// Parameters
    /// ----------
    /// adjustment : Adjustment
    ///     Add-back or deduction; its ``id`` must be unique within the
    ///     configuration.
    ///
    /// Returns
    /// -------
    /// NormalizationConfig
    ///     This configuration, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If another adjustment already uses the same ``id`` (which would
    ///     double-count it).
    #[pyo3(text_signature = "($self, adjustment)")]
    fn add_adjustment<'py>(
        mut slf: PyRefMut<'py, Self>,
        adjustment: PyRef<'_, PyAdjustment>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let next = slf
            .inner
            .clone()
            .add_adjustment(adjustment.inner.clone())
            .map_err(statements_to_py)?;
        slf.inner = next;
        Ok(slf)
    }

    /// Check that the configuration holds no duplicate adjustment ids.
    ///
    /// ``normalize`` runs this itself; call it directly to reject a config
    /// loaded from JSON before it reaches a pipeline.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     Naming the first adjustment id that appears more than once.
    #[pyo3(text_signature = "($self)")]
    fn validate(&self) -> PyResult<()> {
        self.inner.validate().map_err(statements_to_py)
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

    /// Deserialize a normalization configuration from JSON.
    ///
    /// Each adjustment carries an id, name, optional category, a value rule
    /// (``{"type": "fixed", "amounts": {...}}`` or ``{"type":
    /// "percentage_of_node", "node_id": ..., "percentage": 0.05}``), and an
    /// optional cap. Unknown fields are rejected.
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: NormalizationConfig = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid NormalizationConfig JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize this configuration to compact JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize NormalizationConfig"))
    }

    /// Node identifier of the metric being normalized (e.g. ``"ebitda"``).
    #[getter]
    fn target_node(&self) -> &str {
        &self.inner.target_node
    }

    /// Configured adjustments in application order.
    #[getter]
    fn adjustments(&self) -> Vec<PyAdjustment> {
        self.inner
            .adjustments
            .iter()
            .cloned()
            .map(|inner| PyAdjustment { inner })
            .collect()
    }

    /// Number of add-back / deduction adjustments configured.
    #[getter]
    fn adjustment_count(&self) -> usize {
        self.inner.adjustments.len()
    }

    /// Return the representation with the target node and adjustment count.
    fn __repr__(&self) -> String {
        format!(
            "NormalizationConfig(target={:?}, adjustments={})",
            self.inner.target_node,
            self.inner.adjustments.len()
        )
    }
}

/// Run normalization on statement results.
///
/// Parameters
/// ----------
/// results : StatementResult
///     Evaluated statement results.
/// config : NormalizationConfig
///     Normalization configuration (target node + adjustments).
///
/// Returns
/// -------
/// list[NormalizationResult]
///     Typed period results in chronological order.
///
/// Raises
/// ------
/// KeyError
///     If the target node or a referenced node is not in ``results``.
/// ValueError
///     If the configuration holds duplicate adjustment ids or a cap is
///     misconfigured.
#[pyfunction]
#[pyo3(text_signature = "(results, config)")]
fn normalize(
    results: &PyStatementResult,
    config: &PyNormalizationConfig,
) -> PyResult<Vec<PyNormalizationResult>> {
    let norm_results =
        finstack_quant_statements::adjustments::engine::NormalizationEngine::normalize(
            &results.inner,
            &config.inner,
        )
        .map_err(statements_to_py)?;

    Ok(norm_results
        .into_iter()
        .map(|inner| PyNormalizationResult { inner })
        .collect())
}

/// Run normalization and return compact wire JSON.
///
/// The JSON twin of :func:`normalize`: a JSON array of normalization
/// results, one per period.
#[pyfunction]
#[pyo3(text_signature = "(results, config)")]
fn normalize_json(results: &PyStatementResult, config: &PyNormalizationConfig) -> PyResult<String> {
    let norm_results =
        finstack_quant_statements::adjustments::engine::NormalizationEngine::normalize(
            &results.inner,
            &config.inner,
        )
        .map_err(statements_to_py)?;
    serde_json::to_string(&norm_results)
        .map_err(|e| serde_json_to_py(e, "failed to serialize normalization results"))
}

/// Register adjustment classes and functions.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyAdjustment>()?;
    m.add_class::<PyAppliedAdjustment>()?;
    m.add_class::<PyNormalizationConfig>()?;
    m.add_class::<PyNormalizationResult>()?;
    m.add_function(pyo3::wrap_pyfunction!(normalize, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(normalize_json, m)?)?;
    Ok(())
}
