//! Python wrappers for EBITDA normalization and adjustments.

use super::evaluator::PyStatementResult;
use crate::bindings::macros::wire_methods;
use crate::bindings::pydoc::pydoc;
use crate::errors::statements_to_py;
use finstack_quant_statements::adjustments::{
    Adjustment, AdjustmentCap, AdjustmentValue, AppliedAdjustment, NormalizationConfig,
    NormalizationResult,
};
use pyo3::prelude::*;

/// One add-back or deduction applied while normalizing a target metric.
///
/// Build with :meth:`fixed` (explicit per-period amounts) or
/// :meth:`percentage` (a fraction of another node), then optionally cap it
/// with :meth:`with_cap`.
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

wire_methods!(PyAdjustment, Adjustment, "Adjustment");

#[pymethods]
impl PyAdjustment {
    #[doc = pydoc!("finstack_quant.statements._adjustments", "Adjustment.fixed")]
    #[staticmethod]
    #[pyo3(text_signature = "(id, name, amounts)")]
    fn fixed(id: &str, name: &str, amounts: &Bound<'_, PyAny>) -> PyResult<Self> {
        let amounts = super::extract_value_series(amounts)?.into_iter().collect();
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

    #[doc = pydoc!("finstack_quant.statements._adjustments", "Adjustment.with_cap")]
    #[pyo3(text_signature = "($self, cap)")]
    fn with_cap(&self, cap: &Bound<'_, PyAny>) -> PyResult<Self> {
        let cap: AdjustmentCap =
            crate::bindings::module_utils::spec_from_py(cap.py(), cap, "adjustment cap")?;
        Ok(Self {
            inner: self.inner.clone().with_cap(cap),
        })
    }

    /// Return a copy with a grouping category (``"one_time"``, ``"run_rate"``).
    #[pyo3(text_signature = "($self, category)")]
    fn with_category(&self, category: &str) -> Self {
        Self {
            inner: self.inner.clone().with_category(category),
        }
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

    #[doc = pydoc!("finstack_quant.statements._adjustments", "Adjustment.cap")]
    #[getter]
    fn cap<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .cap
            .as_ref()
            .map(|cap| crate::bindings::pandas_utils::serde_to_py(py, cap))
            .transpose()
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

wire_methods!(PyAppliedAdjustment, AppliedAdjustment, "AppliedAdjustment");

#[pymethods]
impl PyAppliedAdjustment {
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

    /// Signed amount before applying a cap, as a float.
    #[getter]
    fn raw_amount(&self) -> f64 {
        self.inner.raw_amount.value()
    }

    #[doc = pydoc!("finstack_quant.statements._adjustments", "AppliedAdjustment.cap_base")]
    #[getter]
    fn cap_base(&self) -> Option<f64> {
        self.inner.cap_base.map(|amount| amount.value())
    }

    #[doc = pydoc!("finstack_quant.statements._adjustments", "AppliedAdjustment.cap_limit")]
    #[getter]
    fn cap_limit(&self) -> Option<f64> {
        self.inner.cap_limit.map(|amount| amount.value())
    }

    /// Signed amount applied after any cap, as a float.
    #[getter]
    fn capped_amount(&self) -> f64 {
        self.inner.capped_amount.value()
    }

    /// Whether a cap changed the calculated amount.
    #[getter]
    fn is_capped(&self) -> bool {
        self.inner.is_capped
    }

    #[doc = pydoc!("finstack_quant.statements._adjustments", "AppliedAdjustment.adjusted_value")]
    #[getter]
    fn adjusted_value(&self) -> f64 {
        self.inner.adjusted_value.value()
    }

    /// Return a concise diagnostic representation.
    fn __repr__(&self) -> String {
        format!(
            "AppliedAdjustment(id={:?}, raw_amount={}, capped_amount={}, is_capped={})",
            self.inner.adjustment_id,
            self.inner.raw_amount.value(),
            self.inner.capped_amount.value(),
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

wire_methods!(
    PyNormalizationResult,
    NormalizationResult,
    "NormalizationResult"
);

#[pymethods]
impl PyNormalizationResult {
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

    #[doc = pydoc!("finstack_quant.statements._adjustments", "NormalizationResult.base_value")]
    #[getter]
    fn base_value(&self) -> Option<f64> {
        self.inner.base_value.map(|amount| amount.value())
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

    #[doc = pydoc!("finstack_quant.statements._adjustments", "NormalizationResult.final_value")]
    #[getter]
    fn final_value(&self) -> Option<f64> {
        self.inner.final_value.map(|amount| amount.value())
    }

    #[doc = pydoc!("finstack_quant.statements._adjustments", "NormalizationResult.unavailable")]
    #[getter]
    fn unavailable(&self) -> Option<&'static str> {
        self.inner.unavailable.map(|reason| reason.as_str())
    }

    /// Return a concise diagnostic representation.
    fn __repr__(&self) -> String {
        format!(
            "NormalizationResult(period={:?}, base_value={:?}, final_value={:?}, adjustments={})",
            self.inner.period.to_string(),
            self.base_value(),
            self.final_value(),
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

wire_methods!(
    PyNormalizationConfig,
    NormalizationConfig,
    "NormalizationConfig"
);

#[pymethods]
impl PyNormalizationConfig {
    #[doc = pydoc!("finstack_quant.statements._adjustments", "NormalizationConfig.__init__")]
    #[new]
    #[pyo3(text_signature = "(target_node, output_node)")]
    fn new(target_node: &str, output_node: &str) -> Self {
        Self {
            inner: NormalizationConfig::new(target_node, output_node),
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

    /// Node identifier of the metric being normalized (e.g. ``"ebitda"``).
    #[getter]
    fn target_node(&self) -> &str {
        &self.inner.target_node
    }

    /// New node the adjusted metric is published under (e.g. ``"adjusted_ebitda"``).
    #[getter]
    fn output_node(&self) -> &str {
        &self.inner.output_node
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
            "NormalizationConfig(target={:?}, output={:?}, adjustments={})",
            self.inner.target_node,
            self.inner.output_node,
            self.inner.adjustments.len()
        )
    }
}

#[doc = pydoc!("finstack_quant.statements._adjustments", "normalize")]
#[pyfunction]
#[pyo3(text_signature = "(results, config)")]
fn normalize(
    results: &PyStatementResult,
    config: &PyNormalizationConfig,
) -> PyResult<Vec<PyNormalizationResult>> {
    let norm_results =
        finstack_quant_statements::adjustments::normalize(&results.inner, &config.inner)
            .map_err(statements_to_py)?;

    Ok(norm_results
        .into_iter()
        .map(|inner| PyNormalizationResult { inner })
        .collect())
}

#[doc = pydoc!("finstack_quant.statements._adjustments", "merge_into_results")]
#[pyfunction]
#[pyo3(text_signature = "(results, config)")]
fn merge_into_results(
    results: &PyStatementResult,
    config: &PyNormalizationConfig,
) -> PyResult<PyStatementResult> {
    let mut inner = results.inner.clone();
    finstack_quant_statements::adjustments::merge_into_results(&mut inner, &config.inner)
        .map_err(statements_to_py)?;
    Ok(PyStatementResult { inner })
}

/// Register adjustment classes and functions.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyAdjustment>()?;
    m.add_class::<PyAppliedAdjustment>()?;
    m.add_class::<PyNormalizationConfig>()?;
    m.add_class::<PyNormalizationResult>()?;
    m.add_function(pyo3::wrap_pyfunction!(normalize, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(merge_into_results, m)?)?;
    Ok(())
}
