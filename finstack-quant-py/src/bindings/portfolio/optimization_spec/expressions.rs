use pyo3::prelude::*;
use pyo3::types::PyType;

use finstack_quant_portfolio::optimization::{
    Constraint, Inequality, MetricExpr, Objective, PerPositionMetric, PositionFilter,
};
use finstack_quant_portfolio::types::{AttributeTest, AttributeValue, ComparisonOp, PositionId};
use finstack_quant_valuations::metrics::MetricId;

use crate::errors::portfolio_to_py;

use super::super::json_bridge::{deserialize_json, serialize_json};
use super::enums::PyInequality;

fn parse_metric_id(id: &str) -> PyResult<MetricId> {
    // `FromStr::from_str` never fails for `MetricId` — unknown names silently
    // become custom metrics, which the optimizer resolves to all-zero
    // objective coefficients while still reporting `Optimal`. API boundaries
    // therefore parse strictly; genuine custom measures go through
    // `PerPositionMetric.custom_key`.
    MetricId::parse_strict(id).map_err(crate::errors::core_to_py)
}

fn parse_attribute_value(text: Option<String>, number: Option<f64>) -> PyResult<AttributeValue> {
    match (text, number) {
        (Some(t), None) => Ok(AttributeValue::Text(t)),
        (None, Some(n)) => Ok(AttributeValue::Number(n)),
        (Some(_), Some(_)) => Err(crate::errors::value_error(
            "AttributeTest accepts either text= or number=, not both",
        )),
        (None, None) => Err(crate::errors::value_error(
            "AttributeTest requires text= or number=",
        )),
    }
}

fn parse_comparison_op(op: &str) -> PyResult<ComparisonOp> {
    op.parse().map_err(portfolio_to_py)
}

/// Accept an ``Inequality`` object or one of its string spellings
/// (``"le"``/``"<="``, ``"ge"``/``">="``, ``"eq"``/``"=="``).
fn extract_inequality(op: &Bound<'_, PyAny>) -> PyResult<Inequality> {
    if let Ok(typed) = op.cast::<PyInequality>() {
        return Ok(typed.borrow().inner);
    }
    let text: String = op.extract().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err(
            "expected an Inequality or one of 'le', '<=', 'ge', '>=', 'eq', '=='",
        )
    })?;
    text.parse().map_err(portfolio_to_py)
}

/// Per-position metric source (clone-only declarative wrapper).
#[pyclass(
    name = "PerPositionMetric",
    module = "finstack_quant.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyPerPositionMetric {
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
    /// Names are validated strictly against the standard metric set; an
    /// unknown identifier raises ``ValueError`` listing the available
    /// metrics. For custom measures stored in
    /// ``ValuationResult::measures``, use :meth:`custom_key` instead.
    #[classmethod]
    #[pyo3(text_signature = "(cls, metric_id)")]
    fn metric(_cls: &Bound<'_, PyType>, metric_id: &str) -> PyResult<Self> {
        Ok(Self::from_inner(PerPositionMetric::Metric(
            parse_metric_id(metric_id)?,
        )))
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
        Ok(Self::from_inner(PerPositionMetric::AttributeIndicator(
            test,
        )))
    }

    /// Constant scalar applied to every position.
    #[classmethod]
    #[pyo3(text_signature = "(cls, value)")]
    fn constant(_cls: &Bound<'_, PyType>, value: f64) -> Self {
        Self::from_inner(PerPositionMetric::Constant(value))
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

    /// Parse from a serde-JSON object (e.g. ``{"Metric": "dv01"}``).
    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: PerPositionMetric = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
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

/// Declarative filter selecting which positions a rule applies to.
#[pyclass(
    name = "PositionFilter",
    module = "finstack_quant.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyPositionFilter {
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
    #[pyo3(name = "not_", text_signature = "(cls, inner)")]
    fn not_(_cls: &Bound<'_, PyType>, inner: PyPositionFilter) -> Self {
        Self::from_inner(PositionFilter::Not(Box::new(inner.inner)))
    }

    /// Match positions matched by ALL of the supplied filters.
    #[classmethod]
    #[pyo3(text_signature = "(cls, filters)")]
    fn and_(_cls: &Bound<'_, PyType>, filters: Vec<PyPositionFilter>) -> Self {
        Self::from_inner(PositionFilter::And(
            filters.into_iter().map(|f| f.inner).collect(),
        ))
    }

    /// Match positions matched by ANY of the supplied filters.
    #[classmethod]
    #[pyo3(text_signature = "(cls, filters)")]
    fn or_(_cls: &Bound<'_, PyType>, filters: Vec<PyPositionFilter>) -> Self {
        Self::from_inner(PositionFilter::Or(
            filters.into_iter().map(|f| f.inner).collect(),
        ))
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

    /// Parse from JSON (matches the on-wire Rust shape).
    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: PositionFilter = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
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

/// Portfolio-level metric expression (`WeightedSum` / `ValueWeightedAverage`).
#[pyclass(
    name = "MetricExpr",
    module = "finstack_quant.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyMetricExpr {
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

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: MetricExpr = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
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

/// Optimization direction and target.
#[pyclass(
    name = "Objective",
    module = "finstack_quant.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyObjective {
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

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: Objective = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
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
            Objective::Maximize(e) | Objective::Minimize(e) => PyMetricExpr::from_inner(e.clone()),
        }
    }

    fn __repr__(&self) -> String {
        format!("Objective.{}(...)", self.direction())
    }
}

/// Declarative constraint specification.
#[pyclass(
    name = "Constraint",
    module = "finstack_quant.portfolio",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyConstraint {
    pub(crate) inner: Constraint,
}

impl PyConstraint {
    pub(crate) fn from_inner(inner: Constraint) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyConstraint {
    /// Generic ``metric op rhs`` constraint.
    ///
    /// ``op`` accepts an :class:`Inequality` or its string spelling
    /// (``"le"``/``"<="``, ``"ge"``/``">="``, ``"eq"``/``"=="``).
    #[classmethod]
    #[pyo3(text_signature = "(cls, metric, op, rhs, label=None)")]
    #[pyo3(signature = (metric, op, rhs, label=None))]
    fn metric_bound(
        _cls: &Bound<'_, PyType>,
        metric: PyMetricExpr,
        op: &Bound<'_, PyAny>,
        rhs: f64,
        label: Option<String>,
    ) -> PyResult<Self> {
        Ok(Self::from_inner(Constraint::MetricBound {
            label,
            metric: metric.inner,
            op: extract_inequality(op)?,
            rhs,
        }))
    }

    /// Weight bounds for positions matching the filter. Raises
    /// ``PortfolioError`` when ``min > max``.
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
        let mut c = Constraint::weight_bounds(filter.inner, min, max).map_err(portfolio_to_py)?;
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
        let mut c = Constraint::max_turnover(max_turnover).map_err(portfolio_to_py)?;
        if let Some(lbl) = label {
            c = c.with_label(lbl);
        }
        Ok(Self::from_inner(c))
    }

    /// Budget / normalization constraint (typically ``rhs = 1.0``).
    #[classmethod]
    #[pyo3(text_signature = "(cls, rhs)")]
    fn budget(_cls: &Bound<'_, PyType>, rhs: f64) -> PyResult<Self> {
        let c = Constraint::budget(rhs).map_err(portfolio_to_py)?;
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
        let mut c = Constraint::exposure_limit(key, value, max_share).map_err(portfolio_to_py)?;
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
        let mut c = Constraint::exposure_minimum(key, value, min_share).map_err(portfolio_to_py)?;
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

    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: Constraint = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
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
