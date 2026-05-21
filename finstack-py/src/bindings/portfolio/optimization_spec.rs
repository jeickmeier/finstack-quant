//! Typed `#[pyclass]` wrappers for `finstack_portfolio::optimization` spec
//! and result types.
//!
//! This module is additive: the existing entry point
//! [`super::optimization::optimize_portfolio`] continues to accept a JSON
//! ``PortfolioOptimizationSpec`` and return a JSON result. Bindings introduced
//! here let callers build a spec programmatically using the same builder
//! pattern as the Rust API, and inspect optimization results via typed
//! getters.
//!
//! Notes on what is bound and what is not:
//!
//! - All declarative variants of [`Constraint`], [`Objective`], [`MetricExpr`],
//!   [`PerPositionMetric`], [`PositionFilter`], and the unit-style enums
//!   (`Inequality`, `WeightingScheme`, `MissingMetricPolicy`,
//!   `OptimizationStatus`, `TradeDirection`, `TradeType`) are exposed.
//! - [`CandidatePosition`] and [`TradeUniverse`] hold an
//!   `Arc<DynInstrument>` and so cannot be constructed from Python without
//!   the wider instrument-binding surface. They are bound here as opaque
//!   wrappers (`from_inner` is internal); a future slice can add Python
//!   constructors once the instrument bridge is wired.
//! - [`PortfolioOptimizationResult`] derives `Serialize` but not
//!   `Deserialize`, so its binding exposes ``to_json`` only (no ``from_json``).

use std::str::FromStr;

use indexmap::IndexMap;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyModule, PyType};

use finstack_portfolio::optimization::{
    self as opt, CandidatePosition, Constraint, Inequality, MetricExpr, MissingMetricPolicy,
    Objective, OptimizationParameters, OptimizationStatus, PerPositionMetric, PortfolioOptimizationResult,
    PortfolioOptimizationSpec, PositionFilter, TradeDirection, TradeSpec, TradeType, TradeUniverse,
    WeightingScheme,
};
use finstack_portfolio::types::{AttributeTest, AttributeValue, ComparisonOp, PositionId};
use finstack_valuations::metrics::MetricId;

use crate::errors::display_to_py;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_metric_id(id: &str) -> MetricId {
    // `FromStr::from_str` never fails for `MetricId` — it falls back to a
    // custom id for unknown names. This matches the JSON-deserialized
    // behaviour the existing entry points expose.
    MetricId::from_str(id).unwrap_or_else(|_| MetricId::custom(id))
}

fn parse_attribute_value(text: Option<String>, number: Option<f64>) -> PyResult<AttributeValue> {
    match (text, number) {
        (Some(t), None) => Ok(AttributeValue::Text(t)),
        (None, Some(n)) => Ok(AttributeValue::Number(n)),
        (Some(_), Some(_)) => Err(PyValueError::new_err(
            "AttributeTest accepts either text= or number=, not both",
        )),
        (None, None) => Err(PyValueError::new_err(
            "AttributeTest requires text= or number=",
        )),
    }
}

fn parse_comparison_op(op: &str) -> PyResult<ComparisonOp> {
    match op {
        "eq" | "==" => Ok(ComparisonOp::Eq),
        "ne" | "!=" => Ok(ComparisonOp::Ne),
        "lt" | "<" => Ok(ComparisonOp::Lt),
        "le" | "<=" => Ok(ComparisonOp::Le),
        "gt" | ">" => Ok(ComparisonOp::Gt),
        "ge" | ">=" => Ok(ComparisonOp::Ge),
        other => Err(PyValueError::new_err(format!(
            "Unknown comparison operator {other:?}; expected one of eq/ne/lt/le/gt/ge"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Unit-style enums: WeightingScheme, MissingMetricPolicy, Inequality,
// OptimizationStatus (Optimal/Feasible variants only — others carry data and
// are constructed via dedicated classmethods), TradeDirection, TradeType.
// ---------------------------------------------------------------------------

/// How optimization weights are defined.
#[pyclass(
    name = "WeightingScheme",
    module = "finstack.portfolio",
    eq,
    hash,
    frozen
)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PyWeightingScheme {
    pub(crate) inner: WeightingScheme,
}

impl std::hash::Hash for PyWeightingScheme {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let tag: u8 = match self.inner {
            WeightingScheme::ValueWeight => 0,
            WeightingScheme::NotionalWeight => 1,
            WeightingScheme::UnitScaling => 2,
        };
        tag.hash(state);
    }
}

#[pymethods]
impl PyWeightingScheme {
    #[classmethod]
    fn value_weight(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: WeightingScheme::ValueWeight,
        }
    }

    #[classmethod]
    fn notional_weight(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: WeightingScheme::NotionalWeight,
        }
    }

    #[classmethod]
    fn unit_scaling(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: WeightingScheme::UnitScaling,
        }
    }

    #[getter]
    fn label(&self) -> &'static str {
        match self.inner {
            WeightingScheme::ValueWeight => "value_weight",
            WeightingScheme::NotionalWeight => "notional_weight",
            WeightingScheme::UnitScaling => "unit_scaling",
        }
    }

    fn __repr__(&self) -> String {
        format!("WeightingScheme.{}()", self.label())
    }
}

/// Policy for handling positions missing required metrics.
#[pyclass(
    name = "MissingMetricPolicy",
    module = "finstack.portfolio",
    eq,
    hash,
    frozen
)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PyMissingMetricPolicy {
    pub(crate) inner: MissingMetricPolicy,
}

impl std::hash::Hash for PyMissingMetricPolicy {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let tag: u8 = match self.inner {
            MissingMetricPolicy::Zero => 0,
            MissingMetricPolicy::Exclude => 1,
            MissingMetricPolicy::Strict => 2,
        };
        tag.hash(state);
    }
}

#[pymethods]
impl PyMissingMetricPolicy {
    #[classmethod]
    fn zero(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: MissingMetricPolicy::Zero,
        }
    }

    #[classmethod]
    fn exclude(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: MissingMetricPolicy::Exclude,
        }
    }

    #[classmethod]
    fn strict(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: MissingMetricPolicy::Strict,
        }
    }

    #[getter]
    fn label(&self) -> &'static str {
        match self.inner {
            MissingMetricPolicy::Zero => "zero",
            MissingMetricPolicy::Exclude => "exclude",
            MissingMetricPolicy::Strict => "strict",
        }
    }

    fn __repr__(&self) -> String {
        format!("MissingMetricPolicy.{}()", self.label())
    }
}

/// Inequality / equality operator (`<=`, `>=`, `==`).
#[pyclass(name = "Inequality", module = "finstack.portfolio", eq, hash, frozen)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PyInequality {
    pub(crate) inner: Inequality,
}

impl std::hash::Hash for PyInequality {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let tag: u8 = match self.inner {
            Inequality::Le => 0,
            Inequality::Ge => 1,
            Inequality::Eq => 2,
        };
        tag.hash(state);
    }
}

#[pymethods]
impl PyInequality {
    #[classmethod]
    fn le(_cls: &Bound<'_, PyType>) -> Self {
        Self { inner: Inequality::Le }
    }

    #[classmethod]
    fn ge(_cls: &Bound<'_, PyType>) -> Self {
        Self { inner: Inequality::Ge }
    }

    #[classmethod]
    fn eq(_cls: &Bound<'_, PyType>) -> Self {
        Self { inner: Inequality::Eq }
    }

    #[getter]
    fn label(&self) -> &'static str {
        match self.inner {
            Inequality::Le => "le",
            Inequality::Ge => "ge",
            Inequality::Eq => "eq",
        }
    }

    fn __repr__(&self) -> String {
        format!("Inequality.{}()", self.label())
    }
}

/// Trade direction (buy / sell / hold).
#[pyclass(
    name = "TradeDirection",
    module = "finstack.portfolio",
    eq,
    hash,
    frozen
)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PyTradeDirection {
    pub(crate) inner: TradeDirection,
}

impl std::hash::Hash for PyTradeDirection {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let tag: u8 = match self.inner {
            TradeDirection::Buy => 0,
            TradeDirection::Sell => 1,
            TradeDirection::Hold => 2,
        };
        tag.hash(state);
    }
}

#[pymethods]
impl PyTradeDirection {
    #[classmethod]
    fn buy(_cls: &Bound<'_, PyType>) -> Self {
        Self { inner: TradeDirection::Buy }
    }

    #[classmethod]
    fn sell(_cls: &Bound<'_, PyType>) -> Self {
        Self { inner: TradeDirection::Sell }
    }

    #[classmethod]
    fn hold(_cls: &Bound<'_, PyType>) -> Self {
        Self { inner: TradeDirection::Hold }
    }

    #[getter]
    fn label(&self) -> &'static str {
        match self.inner {
            TradeDirection::Buy => "buy",
            TradeDirection::Sell => "sell",
            TradeDirection::Hold => "hold",
        }
    }

    fn __repr__(&self) -> String {
        format!("TradeDirection.{}()", self.label())
    }
}

/// Trade type (existing / new position / close-out).
#[pyclass(name = "TradeType", module = "finstack.portfolio", eq, hash, frozen)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PyTradeType {
    pub(crate) inner: TradeType,
}

impl std::hash::Hash for PyTradeType {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let tag: u8 = match self.inner {
            TradeType::Existing => 0,
            TradeType::NewPosition => 1,
            TradeType::CloseOut => 2,
        };
        tag.hash(state);
    }
}

#[pymethods]
impl PyTradeType {
    #[classmethod]
    fn existing(_cls: &Bound<'_, PyType>) -> Self {
        Self { inner: TradeType::Existing }
    }

    #[classmethod]
    fn new_position(_cls: &Bound<'_, PyType>) -> Self {
        Self { inner: TradeType::NewPosition }
    }

    #[classmethod]
    fn close_out(_cls: &Bound<'_, PyType>) -> Self {
        Self { inner: TradeType::CloseOut }
    }

    #[getter]
    fn label(&self) -> &'static str {
        match self.inner {
            TradeType::Existing => "existing",
            TradeType::NewPosition => "new_position",
            TradeType::CloseOut => "close_out",
        }
    }

    fn __repr__(&self) -> String {
        format!("TradeType.{}()", self.label())
    }
}

/// Per-position metric source (clone-only declarative wrapper).
#[pyclass(name = "PerPositionMetric", module = "finstack.portfolio", frozen)]
#[derive(Clone)]
pub struct PyPerPositionMetric {
    pub(crate) inner: PerPositionMetric,
}

impl PyPerPositionMetric {
    pub(crate) fn from_inner(inner: PerPositionMetric) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPerPositionMetric {
    /// From a standard `MetricId` string (e.g. ``"dv01"``, ``"duration_mod"``).
    ///
    /// Unknown identifiers are accepted as custom metrics so the spec
    /// round-trips through JSON identically to the existing entry point.
    #[classmethod]
    #[pyo3(text_signature = "(cls, metric_id)")]
    fn metric(_cls: &Bound<'_, PyType>, metric_id: &str) -> Self {
        Self::from_inner(PerPositionMetric::Metric(parse_metric_id(metric_id)))
    }

    /// From a custom-keyed measure in ``ValuationResult::measures``.
    #[classmethod]
    #[pyo3(text_signature = "(cls, key)")]
    fn custom_key(_cls: &Bound<'_, PyType>, key: &str) -> Self {
        Self::from_inner(PerPositionMetric::CustomKey(key.to_owned()))
    }

    /// Base-currency present value of the position (after scaling).
    #[classmethod]
    fn pv_base(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(PerPositionMetric::PvBase)
    }

    /// Native-currency present value of the position (after scaling).
    #[classmethod]
    fn pv_native(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(PerPositionMetric::PvNative)
    }

    /// Numeric attribute lookup by key.
    #[classmethod]
    #[pyo3(text_signature = "(cls, key)")]
    fn attribute(_cls: &Bound<'_, PyType>, key: &str) -> Self {
        Self::from_inner(PerPositionMetric::Attribute(key.to_owned()))
    }

    /// 1.0 if the supplied attribute test passes, 0.0 otherwise.
    #[classmethod]
    #[pyo3(text_signature = "(cls, key, op, text=None, number=None)")]
    #[pyo3(signature = (key, op, text=None, number=None))]
    fn attribute_indicator(
        _cls: &Bound<'_, PyType>,
        key: &str,
        op: &str,
        text: Option<String>,
        number: Option<f64>,
    ) -> PyResult<Self> {
        let test = AttributeTest::new(
            key.to_owned(),
            parse_comparison_op(op)?,
            parse_attribute_value(text, number)?,
        );
        Ok(Self::from_inner(PerPositionMetric::AttributeIndicator(test)))
    }

    /// Constant scalar applied to every position.
    #[classmethod]
    #[pyo3(text_signature = "(cls, value)")]
    fn constant(_cls: &Bound<'_, PyType>, value: f64) -> Self {
        Self::from_inner(PerPositionMetric::Constant(value))
    }

    /// Parse from a serde-JSON object (e.g. ``{"Metric": "dv01"}``).
    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: PerPositionMetric = serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Variant tag (``"metric"``, ``"pv_base"``, etc.).
    #[getter]
    fn kind(&self) -> &'static str {
        match &self.inner {
            PerPositionMetric::Metric(_) => "metric",
            PerPositionMetric::CustomKey(_) => "custom_key",
            PerPositionMetric::PvBase => "pv_base",
            PerPositionMetric::PvNative => "pv_native",
            PerPositionMetric::Attribute(_) => "attribute",
            PerPositionMetric::AttributeIndicator(_) => "attribute_indicator",
            PerPositionMetric::Constant(_) => "constant",
        }
    }

    fn __repr__(&self) -> String {
        format!("PerPositionMetric.{}(...)", self.kind())
    }
}

// ---------------------------------------------------------------------------
// PositionFilter
// ---------------------------------------------------------------------------

/// Declarative filter selecting which positions a rule applies to.
#[pyclass(name = "PositionFilter", module = "finstack.portfolio", frozen)]
#[derive(Clone)]
pub struct PyPositionFilter {
    pub(crate) inner: PositionFilter,
}

impl PyPositionFilter {
    pub(crate) fn from_inner(inner: PositionFilter) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPositionFilter {
    /// Match every position.
    #[classmethod]
    fn all(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(PositionFilter::All)
    }

    /// Match positions whose entity matches the supplied id.
    #[classmethod]
    #[pyo3(text_signature = "(cls, entity_id)")]
    fn by_entity_id(_cls: &Bound<'_, PyType>, entity_id: &str) -> Self {
        Self::from_inner(PositionFilter::ByEntityId(entity_id.into()))
    }

    /// Match positions whose attribute satisfies the supplied test.
    #[classmethod]
    #[pyo3(text_signature = "(cls, key, op, text=None, number=None)")]
    #[pyo3(signature = (key, op, text=None, number=None))]
    fn by_attribute(
        _cls: &Bound<'_, PyType>,
        key: &str,
        op: &str,
        text: Option<String>,
        number: Option<f64>,
    ) -> PyResult<Self> {
        let test = AttributeTest::new(
            key.to_owned(),
            parse_comparison_op(op)?,
            parse_attribute_value(text, number)?,
        );
        Ok(Self::from_inner(PositionFilter::ByAttribute(test)))
    }

    /// Match positions whose id is in the supplied list.
    #[classmethod]
    #[pyo3(text_signature = "(cls, position_ids)")]
    fn by_position_ids(_cls: &Bound<'_, PyType>, position_ids: Vec<String>) -> Self {
        let ids = position_ids.into_iter().map(PositionId::new).collect();
        Self::from_inner(PositionFilter::ByPositionIds(ids))
    }

    /// Match positions NOT matched by the inner filter.
    #[classmethod]
    #[pyo3(text_signature = "(cls, inner)")]
    fn not(_cls: &Bound<'_, PyType>, inner: PyPositionFilter) -> Self {
        Self::from_inner(PositionFilter::Not(Box::new(inner.inner)))
    }

    /// Match positions matched by ALL of the supplied filters.
    #[classmethod]
    #[pyo3(text_signature = "(cls, filters)")]
    fn and_(_cls: &Bound<'_, PyType>, filters: Vec<PyPositionFilter>) -> Self {
        Self::from_inner(PositionFilter::And(filters.into_iter().map(|f| f.inner).collect()))
    }

    /// Match positions matched by ANY of the supplied filters.
    #[classmethod]
    #[pyo3(text_signature = "(cls, filters)")]
    fn or_(_cls: &Bound<'_, PyType>, filters: Vec<PyPositionFilter>) -> Self {
        Self::from_inner(PositionFilter::Or(filters.into_iter().map(|f| f.inner).collect()))
    }

    /// Parse from JSON (matches the on-wire Rust shape).
    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: PositionFilter = serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Variant tag (``"all"``, ``"by_entity_id"``, ...).
    #[getter]
    fn kind(&self) -> &'static str {
        match &self.inner {
            PositionFilter::All => "all",
            PositionFilter::ByEntityId(_) => "by_entity_id",
            PositionFilter::ByAttribute(_) => "by_attribute",
            PositionFilter::ByPositionIds(_) => "by_position_ids",
            PositionFilter::Not(_) => "not",
            PositionFilter::And(_) => "and",
            PositionFilter::Or(_) => "or",
        }
    }

    fn __repr__(&self) -> String {
        format!("PositionFilter.{}(...)", self.kind())
    }
}

// ---------------------------------------------------------------------------
// MetricExpr
// ---------------------------------------------------------------------------

/// Portfolio-level metric expression (`WeightedSum` / `ValueWeightedAverage`).
#[pyclass(name = "MetricExpr", module = "finstack.portfolio", frozen)]
#[derive(Clone)]
pub struct PyMetricExpr {
    pub(crate) inner: MetricExpr,
}

impl PyMetricExpr {
    pub(crate) fn from_inner(inner: MetricExpr) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyMetricExpr {
    /// `sum_i w_i * m_i`, optionally filtered.
    #[classmethod]
    #[pyo3(text_signature = "(cls, metric, filter=None)")]
    #[pyo3(signature = (metric, filter=None))]
    fn weighted_sum(
        _cls: &Bound<'_, PyType>,
        metric: PyPerPositionMetric,
        filter: Option<PyPositionFilter>,
    ) -> Self {
        Self::from_inner(MetricExpr::WeightedSum {
            metric: metric.inner,
            filter: filter.map(|f| f.inner),
        })
    }

    /// Value-weighted average; assumes weights sum to 1.0.
    #[classmethod]
    #[pyo3(text_signature = "(cls, metric, filter=None)")]
    #[pyo3(signature = (metric, filter=None))]
    fn value_weighted_average(
        _cls: &Bound<'_, PyType>,
        metric: PyPerPositionMetric,
        filter: Option<PyPositionFilter>,
    ) -> Self {
        Self::from_inner(MetricExpr::ValueWeightedAverage {
            metric: metric.inner,
            filter: filter.map(|f| f.inner),
        })
    }

    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: MetricExpr = serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn kind(&self) -> &'static str {
        match &self.inner {
            MetricExpr::WeightedSum { .. } => "weighted_sum",
            MetricExpr::ValueWeightedAverage { .. } => "value_weighted_average",
        }
    }

    fn __repr__(&self) -> String {
        format!("MetricExpr.{}(...)", self.kind())
    }
}

// ---------------------------------------------------------------------------
// Objective
// ---------------------------------------------------------------------------

/// Optimization direction and target.
#[pyclass(name = "Objective", module = "finstack.portfolio", frozen)]
#[derive(Clone)]
pub struct PyObjective {
    pub(crate) inner: Objective,
}

impl PyObjective {
    pub(crate) fn from_inner(inner: Objective) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyObjective {
    #[classmethod]
    #[pyo3(text_signature = "(cls, expr)")]
    fn maximize(_cls: &Bound<'_, PyType>, expr: PyMetricExpr) -> Self {
        Self::from_inner(Objective::Maximize(expr.inner))
    }

    #[classmethod]
    #[pyo3(text_signature = "(cls, expr)")]
    fn minimize(_cls: &Bound<'_, PyType>, expr: PyMetricExpr) -> Self {
        Self::from_inner(Objective::Minimize(expr.inner))
    }

    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: Objective = serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Variant tag: ``"maximize"`` or ``"minimize"``.
    #[getter]
    fn direction(&self) -> &'static str {
        match self.inner {
            Objective::Maximize(_) => "maximize",
            Objective::Minimize(_) => "minimize",
        }
    }

    /// Inner :class:`MetricExpr` being optimized.
    #[getter]
    fn expr(&self) -> PyMetricExpr {
        match &self.inner {
            Objective::Maximize(e) | Objective::Minimize(e) => {
                PyMetricExpr::from_inner(e.clone())
            }
        }
    }

    fn __repr__(&self) -> String {
        format!("Objective.{}(...)", self.direction())
    }
}

// ---------------------------------------------------------------------------
// Constraint
// ---------------------------------------------------------------------------

/// Declarative constraint specification.
#[pyclass(name = "Constraint", module = "finstack.portfolio", frozen)]
#[derive(Clone)]
pub struct PyConstraint {
    pub(crate) inner: Constraint,
}

impl PyConstraint {
    pub(crate) fn from_inner(inner: Constraint) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyConstraint {
    /// Generic `metric op rhs` constraint.
    #[classmethod]
    #[pyo3(text_signature = "(cls, metric, op, rhs, label=None)")]
    #[pyo3(signature = (metric, op, rhs, label=None))]
    fn metric_bound(
        _cls: &Bound<'_, PyType>,
        metric: PyMetricExpr,
        op: PyInequality,
        rhs: f64,
        label: Option<String>,
    ) -> Self {
        Self::from_inner(Constraint::MetricBound {
            label,
            metric: metric.inner,
            op: op.inner,
            rhs,
        })
    }

    /// Weight bounds for positions matching the filter. Returns an error
    /// when ``min > max``.
    #[classmethod]
    #[pyo3(text_signature = "(cls, filter, min, max, label=None)")]
    #[pyo3(signature = (filter, min, max, label=None))]
    fn weight_bounds(
        _cls: &Bound<'_, PyType>,
        filter: PyPositionFilter,
        min: f64,
        max: f64,
        label: Option<String>,
    ) -> PyResult<Self> {
        let mut c = Constraint::weight_bounds(filter.inner, min, max).map_err(display_to_py)?;
        if let Some(lbl) = label {
            c = c.with_label(lbl);
        }
        Ok(Self::from_inner(c))
    }

    /// Maximum turnover: `Σ |w_new - w_current| <= max_turnover`.
    #[classmethod]
    #[pyo3(text_signature = "(cls, max_turnover, label=None)")]
    #[pyo3(signature = (max_turnover, label=None))]
    fn max_turnover(
        _cls: &Bound<'_, PyType>,
        max_turnover: f64,
        label: Option<String>,
    ) -> PyResult<Self> {
        let mut c = Constraint::max_turnover(max_turnover).map_err(display_to_py)?;
        if let Some(lbl) = label {
            c = c.with_label(lbl);
        }
        Ok(Self::from_inner(c))
    }

    /// Budget / normalization constraint (typically ``rhs = 1.0``).
    #[classmethod]
    #[pyo3(text_signature = "(cls, rhs)")]
    fn budget(_cls: &Bound<'_, PyType>, rhs: f64) -> PyResult<Self> {
        let c = Constraint::budget(rhs).map_err(display_to_py)?;
        Ok(Self::from_inner(c))
    }

    /// Shorthand: `sum w_i * I[attr == value] <= max_share`.
    #[classmethod]
    #[pyo3(text_signature = "(cls, key, value, max_share, label=None)")]
    #[pyo3(signature = (key, value, max_share, label=None))]
    fn exposure_limit(
        _cls: &Bound<'_, PyType>,
        key: &str,
        value: &str,
        max_share: f64,
        label: Option<String>,
    ) -> PyResult<Self> {
        let mut c = Constraint::exposure_limit(key, value, max_share).map_err(display_to_py)?;
        if let Some(lbl) = label {
            c = c.with_label(lbl);
        }
        Ok(Self::from_inner(c))
    }

    /// Shorthand: `sum w_i * I[attr == value] >= min_share`.
    #[classmethod]
    #[pyo3(text_signature = "(cls, key, value, min_share, label=None)")]
    #[pyo3(signature = (key, value, min_share, label=None))]
    fn exposure_minimum(
        _cls: &Bound<'_, PyType>,
        key: &str,
        value: &str,
        min_share: f64,
        label: Option<String>,
    ) -> PyResult<Self> {
        let mut c = Constraint::exposure_minimum(key, value, min_share).map_err(display_to_py)?;
        if let Some(lbl) = label {
            c = c.with_label(lbl);
        }
        Ok(Self::from_inner(c))
    }

    /// Attach a label to this constraint (no-op for ``Budget``).
    #[pyo3(text_signature = "(self, label)")]
    fn with_label(&self, label: String) -> Self {
        Self::from_inner(self.inner.clone().with_label(label))
    }

    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: Constraint = serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    /// Variant tag (``"metric_bound"`` / ``"weight_bounds"`` / ``"max_turnover"`` / ``"budget"``).
    #[getter]
    fn kind(&self) -> &'static str {
        match &self.inner {
            Constraint::MetricBound { .. } => "metric_bound",
            Constraint::WeightBounds { .. } => "weight_bounds",
            Constraint::MaxTurnover { .. } => "max_turnover",
            Constraint::Budget { .. } => "budget",
        }
    }

    /// Constraint label, when present.
    #[getter]
    fn label(&self) -> Option<String> {
        self.inner.label().map(str::to_owned)
    }

    fn __repr__(&self) -> String {
        match self.inner.label() {
            Some(lbl) => format!("Constraint.{}(label={:?})", self.kind(), lbl),
            None => format!("Constraint.{}(...)", self.kind()),
        }
    }
}

// ---------------------------------------------------------------------------
// CandidatePosition / TradeUniverse
//
// Both hold `Arc<DynInstrument>` and cannot be safely constructed from
// Python without the wider instrument-binding surface. They remain opaque
// wrappers here so callers can pass them through pipelines once a future
// slice wires the instrument bridge.
// ---------------------------------------------------------------------------

/// Candidate instrument that could be added to the portfolio.
///
/// Construction from Python is not yet supported (requires the instrument
/// binding bridge). The wrapper is exposed so result types and getters can
/// return it.
#[pyclass(name = "CandidatePosition", module = "finstack.portfolio")]
#[derive(Clone)]
pub struct PyCandidatePosition {
    pub(crate) inner: CandidatePosition,
}

impl PyCandidatePosition {
    pub(crate) fn from_inner(inner: CandidatePosition) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCandidatePosition {
    #[getter]
    fn id(&self) -> String {
        self.inner.id.as_str().to_owned()
    }

    #[getter]
    fn entity_id(&self) -> String {
        self.inner.entity_id.as_str().to_owned()
    }

    #[getter]
    fn max_weight(&self) -> f64 {
        self.inner.max_weight
    }

    #[getter]
    fn min_weight(&self) -> f64 {
        self.inner.min_weight
    }

    /// Instrument id, taken from the underlying ``Instrument::id()``.
    #[getter]
    fn instrument_id(&self) -> String {
        self.inner.instrument.id().to_owned()
    }

    fn __repr__(&self) -> String {
        format!(
            "CandidatePosition(id={:?}, entity_id={:?}, instrument_id={:?})",
            self.inner.id.as_str(),
            self.inner.entity_id.as_str(),
            self.inner.instrument.id(),
        )
    }
}

/// Universe of tradeable existing positions and candidate additions.
///
/// Construction from Python is not yet supported (candidate instruments
/// require the instrument binding bridge). The wrapper exists so callers
/// can hold an existing universe and inspect it.
#[pyclass(name = "TradeUniverse", module = "finstack.portfolio")]
#[derive(Clone)]
pub struct PyTradeUniverse {
    pub(crate) inner: TradeUniverse,
}

impl PyTradeUniverse {
    pub(crate) fn from_inner(inner: TradeUniverse) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyTradeUniverse {
    /// Universe where all existing positions are tradeable and no candidates exist.
    #[classmethod]
    fn all_positions(_cls: &Bound<'_, PyType>) -> Self {
        Self::from_inner(TradeUniverse::all_positions())
    }

    #[getter]
    fn tradeable_filter(&self) -> PyPositionFilter {
        PyPositionFilter::from_inner(self.inner.tradeable_filter.clone())
    }

    #[getter]
    fn held_filter(&self) -> Option<PyPositionFilter> {
        self.inner
            .held_filter
            .clone()
            .map(PyPositionFilter::from_inner)
    }

    #[getter]
    fn candidates(&self) -> Vec<PyCandidatePosition> {
        self.inner
            .candidates
            .iter()
            .cloned()
            .map(PyCandidatePosition::from_inner)
            .collect()
    }

    #[getter]
    fn allow_short_candidates(&self) -> bool {
        self.inner.allow_short_candidates
    }

    fn __repr__(&self) -> String {
        format!(
            "TradeUniverse(candidates={}, allow_short_candidates={})",
            self.inner.candidates.len(),
            self.inner.allow_short_candidates,
        )
    }
}

// ---------------------------------------------------------------------------
// OptimizationStatus (enum with structured variants)
// ---------------------------------------------------------------------------

/// Status of an optimization run (mirrors `OptimizationStatus`).
#[pyclass(name = "OptimizationStatus", module = "finstack.portfolio", frozen)]
#[derive(Clone)]
pub struct PyOptimizationStatus {
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

    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: OptimizationStatus = serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
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

// ---------------------------------------------------------------------------
// TradeSpec
// ---------------------------------------------------------------------------

/// Trade specification for a single position.
#[pyclass(name = "TradeSpec", module = "finstack.portfolio", frozen)]
#[derive(Clone)]
pub struct PyTradeSpec {
    pub(crate) inner: TradeSpec,
}

impl PyTradeSpec {
    pub(crate) fn from_inner(inner: TradeSpec) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyTradeSpec {
    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: TradeSpec = serde_json::from_str(json_str).map_err(display_to_py)?;
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
    fn instrument_id(&self) -> String {
        self.inner.instrument_id.clone()
    }

    #[getter]
    fn trade_type(&self) -> PyTradeType {
        PyTradeType { inner: self.inner.trade_type }
    }

    #[getter]
    fn direction(&self) -> PyTradeDirection {
        PyTradeDirection { inner: self.inner.direction }
    }

    #[getter]
    fn current_quantity(&self) -> f64 {
        self.inner.current_quantity
    }

    #[getter]
    fn target_quantity(&self) -> f64 {
        self.inner.target_quantity
    }

    #[getter]
    fn delta_quantity(&self) -> f64 {
        self.inner.delta_quantity
    }

    #[getter]
    fn current_weight(&self) -> f64 {
        self.inner.current_weight
    }

    #[getter]
    fn target_weight(&self) -> f64 {
        self.inner.target_weight
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
}

// ---------------------------------------------------------------------------
// OptimizationParameters
// ---------------------------------------------------------------------------

/// Optimization parameters without an embedded portfolio.
#[pyclass(name = "OptimizationParameters", module = "finstack.portfolio")]
#[derive(Clone)]
pub struct PyOptimizationParameters {
    pub(crate) inner: OptimizationParameters,
}

impl PyOptimizationParameters {
    pub(crate) fn from_inner(inner: OptimizationParameters) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyOptimizationParameters {
    /// Construct with an objective; defaults for other fields mirror the
    /// JSON helpers (`ValueWeight`, `Zero` policy, empty constraints).
    #[new]
    #[pyo3(text_signature = "(objective)")]
    fn new(objective: PyObjective) -> Self {
        Self::from_inner(OptimizationParameters {
            objective: objective.inner,
            constraints: Vec::new(),
            weighting: WeightingScheme::ValueWeight,
            missing_metric_policy: MissingMetricPolicy::Zero,
            label: None,
        })
    }

    /// Append a constraint (returns a new wrapper).
    #[pyo3(text_signature = "(self, constraint)")]
    fn with_constraint(&self, constraint: PyConstraint) -> Self {
        let mut next = self.inner.clone();
        next.constraints.push(constraint.inner);
        Self::from_inner(next)
    }

    /// Replace the weighting scheme.
    #[pyo3(text_signature = "(self, weighting)")]
    fn with_weighting(&self, weighting: PyWeightingScheme) -> Self {
        let mut next = self.inner.clone();
        next.weighting = weighting.inner;
        Self::from_inner(next)
    }

    /// Replace the missing-metric policy.
    #[pyo3(text_signature = "(self, policy)")]
    fn with_missing_metric_policy(&self, policy: PyMissingMetricPolicy) -> Self {
        let mut next = self.inner.clone();
        next.missing_metric_policy = policy.inner;
        Self::from_inner(next)
    }

    /// Replace the auditability label.
    #[pyo3(text_signature = "(self, label)")]
    fn with_label(&self, label: String) -> Self {
        let mut next = self.inner.clone();
        next.label = Some(label);
        Self::from_inner(next)
    }

    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: OptimizationParameters =
            serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn objective(&self) -> PyObjective {
        PyObjective::from_inner(self.inner.objective.clone())
    }

    #[getter]
    fn constraints(&self) -> Vec<PyConstraint> {
        self.inner
            .constraints
            .iter()
            .cloned()
            .map(PyConstraint::from_inner)
            .collect()
    }

    #[getter]
    fn weighting(&self) -> PyWeightingScheme {
        PyWeightingScheme {
            inner: self.inner.weighting,
        }
    }

    #[getter]
    fn missing_metric_policy(&self) -> PyMissingMetricPolicy {
        PyMissingMetricPolicy {
            inner: self.inner.missing_metric_policy,
        }
    }

    #[getter]
    fn label(&self) -> Option<String> {
        self.inner.label.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "OptimizationParameters(direction={:?}, constraints={}, weighting={:?})",
            match self.inner.objective {
                Objective::Maximize(_) => "maximize",
                Objective::Minimize(_) => "minimize",
            },
            self.inner.constraints.len(),
            match self.inner.weighting {
                WeightingScheme::ValueWeight => "value_weight",
                WeightingScheme::NotionalWeight => "notional_weight",
                WeightingScheme::UnitScaling => "unit_scaling",
            },
        )
    }
}

// ---------------------------------------------------------------------------
// PortfolioOptimizationSpec
// ---------------------------------------------------------------------------

/// JSON-serializable portfolio optimization specification, mirroring the
/// Rust builder pattern.
///
/// The portfolio body is held as a ``PortfolioSpec`` JSON payload so this
/// wrapper does not depend on the larger ``PortfolioSpec`` binding (which
/// remains JSON-first elsewhere in the portfolio bindings).
#[pyclass(name = "PortfolioOptimizationSpec", module = "finstack.portfolio")]
#[derive(Clone)]
pub struct PyPortfolioOptimizationSpec {
    pub(crate) inner: PortfolioOptimizationSpec,
}

impl PyPortfolioOptimizationSpec {
    pub(crate) fn from_inner(inner: PortfolioOptimizationSpec) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPortfolioOptimizationSpec {
    /// Build a spec from a portfolio JSON spec + objective. Constraints,
    /// weighting, and policy default to the Rust defaults.
    #[classmethod]
    #[pyo3(text_signature = "(cls, portfolio_spec_json, objective)")]
    fn new(
        _cls: &Bound<'_, PyType>,
        portfolio_spec_json: &str,
        objective: PyObjective,
    ) -> PyResult<Self> {
        let portfolio: finstack_portfolio::portfolio::PortfolioSpec =
            serde_json::from_str(portfolio_spec_json).map_err(display_to_py)?;
        Ok(Self::from_inner(PortfolioOptimizationSpec {
            portfolio,
            objective: objective.inner,
            constraints: Vec::new(),
            weighting: WeightingScheme::ValueWeight,
            missing_metric_policy: MissingMetricPolicy::Zero,
            label: None,
        }))
    }

    /// Append a constraint (returns a new spec).
    #[pyo3(text_signature = "(self, constraint)")]
    fn with_constraint(&self, constraint: PyConstraint) -> Self {
        let mut next = self.inner.clone();
        next.constraints.push(constraint.inner);
        Self::from_inner(next)
    }

    /// Replace the objective.
    #[pyo3(text_signature = "(self, objective)")]
    fn with_objective(&self, objective: PyObjective) -> Self {
        let mut next = self.inner.clone();
        next.objective = objective.inner;
        Self::from_inner(next)
    }

    /// Replace the weighting scheme.
    #[pyo3(text_signature = "(self, weighting)")]
    fn with_weighting(&self, weighting: PyWeightingScheme) -> Self {
        let mut next = self.inner.clone();
        next.weighting = weighting.inner;
        Self::from_inner(next)
    }

    /// Replace the missing-metric policy.
    #[pyo3(text_signature = "(self, policy)")]
    fn with_missing_metric_policy(&self, policy: PyMissingMetricPolicy) -> Self {
        let mut next = self.inner.clone();
        next.missing_metric_policy = policy.inner;
        Self::from_inner(next)
    }

    /// Replace the auditability label.
    #[pyo3(text_signature = "(self, label)")]
    fn with_label(&self, label: String) -> Self {
        let mut next = self.inner.clone();
        next.label = Some(label);
        Self::from_inner(next)
    }

    #[classmethod]
    #[pyo3(text_signature = "(cls, json_str)")]
    fn from_json(_cls: &Bound<'_, PyType>, json_str: &str) -> PyResult<Self> {
        let inner: PortfolioOptimizationSpec =
            serde_json::from_str(json_str).map_err(display_to_py)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn objective(&self) -> PyObjective {
        PyObjective::from_inner(self.inner.objective.clone())
    }

    #[getter]
    fn constraints(&self) -> Vec<PyConstraint> {
        self.inner
            .constraints
            .iter()
            .cloned()
            .map(PyConstraint::from_inner)
            .collect()
    }

    #[getter]
    fn weighting(&self) -> PyWeightingScheme {
        PyWeightingScheme {
            inner: self.inner.weighting,
        }
    }

    #[getter]
    fn missing_metric_policy(&self) -> PyMissingMetricPolicy {
        PyMissingMetricPolicy {
            inner: self.inner.missing_metric_policy,
        }
    }

    #[getter]
    fn label(&self) -> Option<String> {
        self.inner.label.clone()
    }

    /// Portfolio specification body (raw JSON).
    #[pyo3(text_signature = "(self)")]
    fn portfolio_spec_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner.portfolio).map_err(display_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "PortfolioOptimizationSpec(constraints={}, label={:?})",
            self.inner.constraints.len(),
            self.inner.label,
        )
    }
}

// ---------------------------------------------------------------------------
// PortfolioOptimizationResult
// ---------------------------------------------------------------------------

/// Result of an optimization run.
///
/// `PortfolioOptimizationResult` implements `Serialize` but not
/// `Deserialize` in the Rust source, so this wrapper exposes ``to_json``
/// only — there is no ``from_json``.
#[pyclass(
    name = "PortfolioOptimizationResult",
    module = "finstack.portfolio"
)]
pub struct PyPortfolioOptimizationResult {
    pub(crate) inner: PortfolioOptimizationResult,
}

impl PyPortfolioOptimizationResult {
    pub(crate) fn from_inner(inner: PortfolioOptimizationResult) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPortfolioOptimizationResult {
    /// Serialize to the canonical JSON wire format.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner).map_err(display_to_py)
    }

    #[getter]
    fn status(&self) -> PyOptimizationStatus {
        PyOptimizationStatus::from_inner(self.inner.status.clone())
    }

    #[getter]
    fn is_feasible(&self) -> bool {
        self.inner.status.is_feasible()
    }

    #[getter]
    fn objective_value(&self) -> f64 {
        self.inner.objective_value
    }

    #[getter]
    fn current_weights(&self) -> std::collections::HashMap<String, f64> {
        self.inner
            .current_weights
            .iter()
            .map(|(k, v)| (k.as_str().to_owned(), *v))
            .collect()
    }

    #[getter]
    fn optimal_weights(&self) -> std::collections::HashMap<String, f64> {
        self.inner
            .optimal_weights
            .iter()
            .map(|(k, v)| (k.as_str().to_owned(), *v))
            .collect()
    }

    #[getter]
    fn weight_deltas(&self) -> std::collections::HashMap<String, f64> {
        self.inner
            .weight_deltas
            .iter()
            .map(|(k, v)| (k.as_str().to_owned(), *v))
            .collect()
    }

    #[getter]
    fn implied_quantities(&self) -> std::collections::HashMap<String, f64> {
        self.inner
            .implied_quantities
            .iter()
            .map(|(k, v)| (k.as_str().to_owned(), *v))
            .collect()
    }

    #[getter]
    fn metric_values(&self) -> std::collections::HashMap<String, f64> {
        self.inner
            .metric_values
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    #[getter]
    fn dual_values(&self) -> std::collections::HashMap<String, f64> {
        self.inner
            .dual_values
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    #[getter]
    fn constraint_slacks(&self) -> std::collections::HashMap<String, f64> {
        self.inner
            .constraint_slacks
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    /// Total turnover (sum of absolute weight changes).
    #[getter]
    fn turnover(&self) -> f64 {
        self.inner.turnover()
    }

    /// Trade list sorted by absolute quantity delta (largest first).
    #[pyo3(text_signature = "(self)")]
    fn to_trade_list(&self) -> Vec<PyTradeSpec> {
        self.inner
            .to_trade_list()
            .into_iter()
            .map(PyTradeSpec::from_inner)
            .collect()
    }

    /// Subset of :meth:`to_trade_list` whose ``trade_type`` is ``NewPosition``.
    #[pyo3(text_signature = "(self)")]
    fn new_position_trades(&self) -> Vec<PyTradeSpec> {
        self.inner
            .new_position_trades()
            .into_iter()
            .map(PyTradeSpec::from_inner)
            .collect()
    }

    /// Binding constraint labels and their slack values.
    #[pyo3(text_signature = "(self)")]
    fn binding_constraints(&self) -> Vec<(String, f64)> {
        self.inner
            .binding_constraints()
            .into_iter()
            .map(|(name, slack)| (name.to_owned(), slack))
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "PortfolioOptimizationResult(status={:?}, objective_value={}, turnover={})",
            match &self.inner.status {
                OptimizationStatus::Optimal => "optimal",
                OptimizationStatus::FeasibleButSuboptimal => "feasible_but_suboptimal",
                OptimizationStatus::Infeasible { .. } => "infeasible",
                OptimizationStatus::Unbounded => "unbounded",
                OptimizationStatus::Error { .. } => "error",
            },
            self.inner.objective_value,
            self.inner.turnover(),
        )
    }
}

// ---------------------------------------------------------------------------
// Free helpers — typed entry points (additive; the JSON entry point in
// `super::optimization` remains unchanged).
// ---------------------------------------------------------------------------

/// Run the optimizer against a typed :class:`PortfolioOptimizationSpec`.
#[pyfunction]
#[pyo3(signature = (spec, market))]
fn optimize_portfolio_typed(
    spec: &PyPortfolioOptimizationSpec,
    market: &Bound<'_, PyAny>,
) -> PyResult<PyPortfolioOptimizationResult> {
    let market = crate::bindings::extract::extract_market_ref(market)?;
    let config = finstack_core::config::FinstackConfig::default();
    let result = opt::optimize_from_spec(&spec.inner, &market, &config)
        .map_err(crate::errors::portfolio_to_py)?;
    Ok(PyPortfolioOptimizationResult::from_inner(result))
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register optimization spec/result classes on the portfolio submodule.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyWeightingScheme>()?;
    m.add_class::<PyMissingMetricPolicy>()?;
    m.add_class::<PyInequality>()?;
    m.add_class::<PyTradeDirection>()?;
    m.add_class::<PyTradeType>()?;
    m.add_class::<PyPerPositionMetric>()?;
    m.add_class::<PyPositionFilter>()?;
    m.add_class::<PyMetricExpr>()?;
    m.add_class::<PyObjective>()?;
    m.add_class::<PyConstraint>()?;
    m.add_class::<PyCandidatePosition>()?;
    m.add_class::<PyTradeUniverse>()?;
    m.add_class::<PyOptimizationStatus>()?;
    m.add_class::<PyTradeSpec>()?;
    m.add_class::<PyOptimizationParameters>()?;
    m.add_class::<PyPortfolioOptimizationSpec>()?;
    m.add_class::<PyPortfolioOptimizationResult>()?;

    m.add_function(wrap_pyfunction!(optimize_portfolio_typed, m)?)?;

    Ok(())
}
