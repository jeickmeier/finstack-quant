//! Python wrapper for the type-state ModelBuilder.
//!
//! Since Python cannot model Rust type-state at the type level, we collapse
//! the two states into a single class and track readiness at runtime. Every
//! fallible configuration step goes through the crate's non-consuming
//! `try_*` twins, so a typo in a formula, period range, or metric id raises
//! without discarding the model accumulated so far.

use super::capital_structure::PyWaterfallSpec;
use super::types::{PyFinancialModelSpec, PyForecastSpec, PyNodeSpec};
use super::{extract_money_series, extract_scalar_series, extract_value_series, PERIOD_GRAMMAR};
use crate::bindings::core::currency::PyCurrency;
use crate::bindings::core::dates::periods::PyPeriod;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::extract_date;
use crate::bindings::pandas_utils::serde_to_py;
use crate::errors::{core_to_py, serde_json_to_py, statements_to_py, value_error};
use finstack_quant_core::dates::{Period, PeriodId};
use finstack_quant_core::money::fx::FxConversionPolicy;
use finstack_quant_statements::builder::{MixedNodeBuilder, ModelBuilder};
use finstack_quant_statements::types::{AmountOrScalar, FinancialStatementInstrument, NodeId};
use pyo3::prelude::*;

/// Builder for financial models (type-state collapsed for Python).
///
/// Every configuration method returns the builder, so calls chain::
///
///     model = (
///         ModelBuilder("Acme Corp")
///         .periods("2025Q1..Q4", "2025Q2")
///         .value("revenue", {"2025Q1": 10_000_000.0, "2025Q2": 11_000_000.0})
///         .compute("cogs", "revenue * 0.6")
///         .build()
///     )
///
/// Statement-per-line style works identically, since each call also mutates in
/// place::
///
///     builder = ModelBuilder("Acme Corp")
///     builder.periods("2025Q1..Q4", "2025Q2")
///     builder.compute("cogs", "revenue * 0.6")
///     model = builder.build()
///
/// ``build()`` and ``mixed()`` are terminal: they consume the builder, so no
/// further configuration call may follow on the same object. A rejected
/// argument (bad period id, invalid formula, unknown metric) raises and
/// leaves the builder usable.
#[pyclass(name = "ModelBuilder", module = "finstack_quant.statements")]
pub struct PyModelBuilder {
    inner: Option<BuilderState>,
}

impl PyModelBuilder {
    /// Shared constructor behind `ModelBuilder(id)` and
    /// `FinancialModelSpec.builder(id)`.
    pub(crate) fn start(id: &str) -> Self {
        Self {
            inner: Some(BuilderState::NeedPeriods(ModelBuilder::new(id))),
        }
    }
}

enum BuilderState {
    NeedPeriods(ModelBuilder<finstack_quant_statements::builder::NeedPeriods>),
    Ready(ModelBuilder<finstack_quant_statements::builder::Ready>),
}

/// Read-only view of one registry metric definition.
///
/// Returned by ``Registry.get``; carries the formula, documentation, and
/// declared dependencies of a reusable metric such as ``fin.gross_margin``.
#[pyclass(
    name = "MetricDefinition",
    module = "finstack_quant.statements",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyMetricDefinition {
    inner: finstack_quant_statements::registry::MetricDefinition,
}

#[pymethods]
impl PyMetricDefinition {
    /// Support `pickle` via the canonical JSON round-trip.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize a metric definition from its canonical JSON form.
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json)
            .map_err(|e| serde_json_to_py(e, "invalid MetricDefinition JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize this definition to canonical JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize MetricDefinition"))
    }

    /// Metric identifier within its namespace (``"gross_margin"``).
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    /// Human-readable metric name.
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    /// Statements DSL formula, with unqualified references to sibling
    /// metrics (``"gross_profit / revenue"``).
    #[getter]
    fn formula(&self) -> &str {
        &self.inner.formula
    }

    /// Description of what the metric represents, or ``None``.
    #[getter]
    fn description(&self) -> Option<&str> {
        self.inner.description.as_deref()
    }

    /// Grouping category (``"margins"``, ``"leverage"``, ...), or ``None``.
    #[getter]
    fn category(&self) -> Option<&str> {
        self.inner.category.as_deref()
    }

    /// Unit type as its snake_case name (``"percentage"``, ``"currency"``,
    /// ``"ratio"``, ``"count"``, ``"time_period"``), or ``None``.
    #[getter]
    fn unit_type(&self) -> Option<String> {
        self.inner
            .unit_type
            .as_ref()
            .map(crate::bindings::statements_analytics::serde_variant_str)
    }

    /// Node identifiers the formula requires as inputs.
    #[getter]
    fn requires(&self) -> Vec<String> {
        self.inner.requires.clone()
    }

    /// Free-form tags for filtering.
    #[getter]
    fn tags(&self) -> Vec<String> {
        self.inner.tags.clone()
    }

    /// Additional metadata as a plain ``dict``.
    #[getter]
    fn meta<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        serde_to_py(py, &self.inner.meta)
    }

    /// Return ``MetricDefinition(id='gross_margin', formula='...')``.
    fn __repr__(&self) -> String {
        format!(
            "MetricDefinition(id={:?}, name={:?}, formula={:?})",
            self.inner.id, self.inner.name, self.inner.formula
        )
    }
}

/// Metric registry used to add reusable statement metrics to a model.
#[pyclass(
    name = "Registry",
    module = "finstack_quant.statements",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyRegistry {
    inner: finstack_quant_statements::registry::Registry,
}

#[pymethods]
impl PyRegistry {
    /// Create an empty metric registry.
    #[new]
    fn new() -> Self {
        Self {
            inner: finstack_quant_statements::registry::Registry::new(),
        }
    }

    /// Create a registry preloaded with built-in metrics.
    #[staticmethod]
    fn with_builtins() -> PyResult<Self> {
        let inner = finstack_quant_statements::registry::Registry::with_builtins()
            .map_err(statements_to_py)?;
        Ok(Self { inner })
    }

    /// Load built-in metrics into this registry.
    fn load_builtins(&mut self) -> PyResult<()> {
        self.inner.load_builtins().map_err(statements_to_py)
    }

    /// Load metrics from a JSON string.
    fn load_from_json_str(&mut self, json: &str) -> PyResult<()> {
        self.inner
            .load_from_json_str(json)
            .map(|_| ())
            .map_err(statements_to_py)
    }

    /// Return whether a fully qualified metric exists.
    fn has(&self, qualified_id: &str) -> bool {
        self.inner.has(qualified_id)
    }

    /// Fully qualified identifiers of every registered metric
    /// (``["fin.gross_profit", "fin.gross_margin", ...]``), in load order.
    #[pyo3(text_signature = "($self)")]
    fn metric_ids(&self) -> Vec<String> {
        self.inner
            .all_metrics()
            .map(|(id, _)| id.to_string())
            .collect()
    }

    /// Look up one metric definition.
    ///
    /// Parameters
    /// ----------
    /// qualified_id : str
    ///     Fully qualified ``namespace.metric_id`` (``"fin.gross_margin"``).
    ///
    /// Returns
    /// -------
    /// MetricDefinition
    ///     The registered definition.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If no metric with that id is registered.
    #[pyo3(text_signature = "($self, qualified_id)")]
    fn get(&self, qualified_id: &str) -> PyResult<PyMetricDefinition> {
        let stored = self.inner.get(qualified_id).map_err(statements_to_py)?;
        Ok(PyMetricDefinition {
            inner: stored.definition.clone(),
        })
    }

    /// Transitive registry dependencies of a metric, dependencies first.
    ///
    /// Parameters
    /// ----------
    /// qualified_id : str
    ///     Fully qualified metric identifier.
    ///
    /// Returns
    /// -------
    /// list[str]
    ///     Fully qualified ids in an order suitable for model construction;
    ///     the metric itself is excluded, as are model inputs that are not
    ///     registry metrics.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``qualified_id`` is not registered.
    #[pyo3(text_signature = "($self, qualified_id)")]
    fn dependencies(&self, qualified_id: &str) -> PyResult<Vec<String>> {
        self.inner
            .get_metric_dependencies(qualified_id)
            .map_err(statements_to_py)
    }

    /// Number of metrics in the registry.
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    /// Return ``Registry(metrics=22)``.
    fn __repr__(&self) -> String {
        format!("Registry(metrics={})", self.inner.len())
    }
}

/// Fluent builder for a mixed statement node.
#[pyclass(name = "MixedNodeBuilder", module = "finstack_quant.statements")]
pub struct PyMixedNodeBuilder {
    inner: Option<MixedNodeBuilder>,
}

#[pymethods]
impl PyMixedNodeBuilder {
    /// Set explicit values for the mixed node.
    ///
    /// Parameters
    /// ----------
    /// values : Mapping[str, float | Money] | Sequence[tuple[str, float | Money]] | pd.Series
    ///     Period id to value; ``Money`` cells make the node monetary.
    ///
    /// Returns
    /// -------
    /// MixedNodeBuilder
    ///     This builder, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a period id does not parse or the builder was consumed.
    #[pyo3(text_signature = "($self, values)")]
    fn values<'py>(
        mut slf: PyRefMut<'py, Self>,
        values: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let parsed = extract_value_series(values)?;
        let builder = slf.take()?;
        slf.inner = Some(builder.values(&parsed));
        Ok(slf)
    }

    /// Set monetary explicit values for the mixed node.
    ///
    /// Parameters
    /// ----------
    /// values : Mapping[str, Money] | Sequence[tuple[str, Money]] | pd.Series
    ///     Period id to ``Money`` amount.
    ///
    /// Returns
    /// -------
    /// MixedNodeBuilder
    ///     This builder, for chaining.
    #[pyo3(text_signature = "($self, values)")]
    fn values_money<'py>(
        mut slf: PyRefMut<'py, Self>,
        values: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let parsed: Vec<(PeriodId, AmountOrScalar)> = extract_money_series(values)?
            .into_iter()
            .map(|(pid, money)| (pid, AmountOrScalar::Amount(money)))
            .collect();
        let builder = slf.take()?;
        slf.inner = Some(builder.values(&parsed));
        Ok(slf)
    }

    /// Set the forecast specification.
    ///
    /// Returns
    /// -------
    /// MixedNodeBuilder
    ///     This builder, for chaining.
    fn forecast<'py>(
        mut slf: PyRefMut<'py, Self>,
        forecast_spec: PyRef<'_, PyForecastSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let builder = slf.take()?;
        slf.inner = Some(builder.forecast(forecast_spec.inner.clone()));
        Ok(slf)
    }

    /// Set the fallback formula.
    ///
    /// The formula is syntax-checked immediately; an invalid formula raises
    /// and leaves the builder (and its parent model) intact.
    ///
    /// Returns
    /// -------
    /// MixedNodeBuilder
    ///     This builder, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the formula is blank or does not parse and compile.
    fn formula<'py>(mut slf: PyRefMut<'py, Self>, formula: &str) -> PyResult<PyRefMut<'py, Self>> {
        slf.builder_mut()?
            .try_formula(formula)
            .map_err(statements_to_py)?;
        Ok(slf)
    }

    /// Set the human-readable name.
    ///
    /// Returns
    /// -------
    /// MixedNodeBuilder
    ///     This builder, for chaining.
    fn name<'py>(mut slf: PyRefMut<'py, Self>, name: &str) -> PyResult<PyRefMut<'py, Self>> {
        let builder = slf.take()?;
        slf.inner = Some(builder.name(name));
        Ok(slf)
    }

    /// Build the mixed node and return a ready model builder.
    fn build(&mut self) -> PyResult<PyModelBuilder> {
        let builder = self.take()?;
        let ready = builder.build().map_err(statements_to_py)?;
        Ok(PyModelBuilder {
            inner: Some(BuilderState::Ready(ready)),
        })
    }

    /// Return ``MixedNodeBuilder(node_id='revenue')`` (or ``consumed``).
    fn __repr__(&self) -> String {
        match &self.inner {
            Some(builder) => format!("MixedNodeBuilder(node_id={:?})", builder.node_id().as_str()),
            None => "MixedNodeBuilder(state='consumed')".to_string(),
        }
    }
}

impl PyMixedNodeBuilder {
    fn take(&mut self) -> PyResult<MixedNodeBuilder> {
        self.inner
            .take()
            .ok_or_else(|| value_error("MixedNodeBuilder has already been consumed"))
    }

    fn builder_mut(&mut self) -> PyResult<&mut MixedNodeBuilder> {
        self.inner
            .as_mut()
            .ok_or_else(|| value_error("MixedNodeBuilder has already been consumed"))
    }
}

#[pymethods]
impl PyModelBuilder {
    /// Create a new model builder.
    ///
    /// :meth:`FinancialModelSpec.builder` is the canonical entry point and
    /// returns exactly this value.
    #[new]
    #[pyo3(text_signature = "(id)")]
    fn new(id: &str) -> Self {
        Self::start(id)
    }

    /// Reconstruct a ready builder from an existing model specification.
    ///
    /// Parameters
    /// ----------
    /// spec : FinancialModelSpec
    ///     Typed model whose periods, nodes, metadata, and capital structure
    ///     seed the builder.
    ///
    /// Returns
    /// -------
    /// ModelBuilder
    ///     Ready builder that can accept additional transformations.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the model has no periods.
    #[staticmethod]
    fn from_spec(spec: &PyFinancialModelSpec) -> PyResult<Self> {
        let builder = ModelBuilder::from_spec(spec.inner.clone()).map_err(statements_to_py)?;
        Ok(Self {
            inner: Some(BuilderState::Ready(builder)),
        })
    }

    /// Define periods using a range expression (e.g. ``"2025Q1..Q4"``).
    ///
    /// Parameters
    /// ----------
    /// range : str
    ///     Period range ``<start>..<end>`` in one period kind:
    ///     ``"2025Q1..Q4"``, ``"2024M10..2025M03"``, ``"2025..2030"``. Period
    ///     ids have no separators (``2025Q1``, ``2025M3``, ``2025W7``,
    ///     ``2025H1``, ``2025``/``FY2025``).
    /// actuals_until : str | None
    ///     Optional inclusive cutoff (same kind as ``range``) through which
    ///     periods are labelled actuals; later periods are forecasts.
    ///
    /// Returns
    /// -------
    /// ModelBuilder
    ///     This builder, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the range or cutoff does not parse (the message restates the
    ///     accepted grammar), the kinds differ, the range is reversed or
    ///     empty, periods were already set, or the builder was consumed. The
    ///     builder stays usable after a rejected range.
    #[pyo3(signature = (range, actuals_until=None), text_signature = "($self, range, actuals_until=None)")]
    fn periods<'py>(
        mut slf: PyRefMut<'py, Self>,
        range: &str,
        actuals_until: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let state = slf.take_any()?;
        match state {
            BuilderState::NeedPeriods(b) => match b.try_periods(range, actuals_until) {
                Ok(ready) => {
                    slf.inner = Some(BuilderState::Ready(ready));
                    Ok(slf)
                }
                Err((b, error)) => {
                    slf.inner = Some(BuilderState::NeedPeriods(*b));
                    Err(value_error(format!(
                        "invalid period range {range:?}: {error}; {PERIOD_GRAMMAR}"
                    )))
                }
            },
            BuilderState::Ready(b) => {
                slf.inner = Some(BuilderState::Ready(b));
                Err(value_error("Periods already set"))
            }
        }
    }

    /// Define the timeline from explicit ``Period`` objects.
    ///
    /// Use this for timelines the range grammar cannot express (a fiscal
    /// calendar from ``core.dates.build_fiscal_periods``, or a hand-built
    /// list). The periods are kept in the given order; ``build()`` rejects
    /// timelines that are not strictly increasing or whose actuals do not
    /// form a prefix.
    ///
    /// Parameters
    /// ----------
    /// periods : list[finstack_quant.core.dates.Period]
    ///     Non-empty period list, typically ``plan.periods`` from a
    ///     ``PeriodPlan``.
    ///
    /// Returns
    /// -------
    /// ModelBuilder
    ///     This builder, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``periods`` is empty, periods were already set, or the builder
    ///     was consumed.
    #[pyo3(text_signature = "($self, periods)")]
    fn periods_explicit<'py>(
        mut slf: PyRefMut<'py, Self>,
        periods: Vec<PyRef<'_, PyPeriod>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let periods: Vec<Period> = periods.iter().map(|p| p.inner.clone()).collect();
        if periods.is_empty() {
            return Err(value_error("Period list must contain at least one period"));
        }
        let state = slf.take_any()?;
        match state {
            BuilderState::NeedPeriods(b) => {
                let ready = b.periods_explicit(periods).map_err(statements_to_py)?;
                slf.inner = Some(BuilderState::Ready(ready));
                Ok(slf)
            }
            BuilderState::Ready(b) => {
                slf.inner = Some(BuilderState::Ready(b));
                Err(value_error("Periods already set"))
            }
        }
    }

    /// Add a value node with explicit period values.
    ///
    /// Parameters
    /// ----------
    /// node_id : str
    ///     Node identifier.
    /// values : Mapping[str, float | Money] | Sequence[tuple[str, float | Money]] | pd.Series
    ///     Period id (``"2025Q1"``) to value. A ``dict``, a pandas ``Series``
    ///     indexed by period id, or ``(period, value)`` pairs are all
    ///     accepted; ``Money`` cells make the node monetary.
    ///
    /// Returns
    /// -------
    /// ModelBuilder
    ///     This builder, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a period id does not parse, periods were not set, or the
    ///     builder was consumed.
    #[pyo3(text_signature = "($self, node_id, values)")]
    fn value<'py>(
        mut slf: PyRefMut<'py, Self>,
        node_id: &str,
        values: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        // Parse arguments BEFORE `take_ready()` so a bad period string does not
        // permanently consume the in-progress builder.
        let parsed = extract_value_series(values)?;
        let state = slf.take_ready()?;
        let ready = state.value(node_id, &parsed);
        slf.inner = Some(BuilderState::Ready(ready));
        Ok(slf)
    }

    /// Add a scalar value node with explicit period values.
    ///
    /// Parameters
    /// ----------
    /// node_id : str
    ///     Node identifier.
    /// values : Mapping[str, float] | Sequence[tuple[str, float]] | pd.Series
    ///     Period id to unitless scalar (ratio, count, percentage as a
    ///     decimal fraction).
    ///
    /// Returns
    /// -------
    /// ModelBuilder
    ///     This builder, for chaining.
    #[pyo3(text_signature = "($self, node_id, values)")]
    fn value_scalar<'py>(
        mut slf: PyRefMut<'py, Self>,
        node_id: &str,
        values: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let parsed = extract_scalar_series(values)?;
        let state = slf.take_ready()?;
        let ready = state.value_scalar(node_id, &parsed);
        slf.inner = Some(BuilderState::Ready(ready));
        Ok(slf)
    }

    /// Add a monetary value node with explicit period values.
    ///
    /// Parameters
    /// ----------
    /// node_id : str
    ///     Node identifier.
    /// values : Mapping[str, Money] | Sequence[tuple[str, Money]] | pd.Series
    ///     Period id to ``Money`` amount; all amounts must share a currency.
    ///
    /// Returns
    /// -------
    /// ModelBuilder
    ///     This builder, for chaining.
    #[pyo3(text_signature = "($self, node_id, values)")]
    fn value_money<'py>(
        mut slf: PyRefMut<'py, Self>,
        node_id: &str,
        values: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let parsed = extract_money_series(values)?;
        let state = slf.take_ready()?;
        let ready = state.value_money(node_id, &parsed);
        slf.inner = Some(BuilderState::Ready(ready));
        Ok(slf)
    }

    /// Bulk-load value nodes from a wide pandas ``DataFrame``.
    ///
    /// Rows are nodes and columns are period ids. When periods have not been
    /// set yet, the timeline is taken from the first and last column
    /// (``"<first>..<last>"``) with ``actuals_until`` as the cutoff; when
    /// they have, the columns must belong to the existing timeline. Each row
    /// becomes a scalar ``value`` node; ``NaN`` / ``None`` cells are skipped.
    ///
    /// Parameters
    /// ----------
    /// df : pd.DataFrame
    ///     Wide frame with period-id column labels (``"2025Q1"``, ...) and
    ///     node ids as the index (or in ``node_id_column``).
    /// actuals_until : str | None
    ///     Inclusive actuals cutoff used only when the timeline is derived
    ///     from the columns.
    /// node_id_column : str | None
    ///     Column holding node ids when they are not the index.
    ///
    /// Returns
    /// -------
    /// ModelBuilder
    ///     This builder, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the frame has no columns, a column label is not a period id,
    ///     a cell is not numeric, or the builder was consumed.
    #[pyo3(signature = (df, actuals_until=None, node_id_column=None), text_signature = "($self, df, actuals_until=None, node_id_column=None)")]
    fn from_dataframe<'py>(
        mut slf: PyRefMut<'py, Self>,
        df: &Bound<'py, PyAny>,
        actuals_until: Option<&str>,
        node_id_column: Option<&str>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let frame = match node_id_column {
            Some(column) => df.call_method1("set_index", (column,))?,
            None => df.clone(),
        };
        let columns: Vec<String> = frame
            .getattr("columns")?
            .try_iter()?
            .map(|column| Ok(column?.str()?.to_string()))
            .collect::<PyResult<Vec<_>>>()?;
        let (Some(first), Some(last)) = (columns.first(), columns.last()) else {
            return Err(value_error(
                "from_dataframe requires at least one period column",
            ));
        };
        if matches!(slf.inner, Some(BuilderState::NeedPeriods(_))) {
            let range = format!("{first}..{last}");
            slf = Self::periods(slf, &range, actuals_until)?;
        }

        let mut rows: Vec<(String, Vec<(PeriodId, AmountOrScalar)>)> = Vec::new();
        for row in frame.call_method0("iterrows")?.try_iter()? {
            let (index, series): (Bound<'py, PyAny>, Bound<'py, PyAny>) = row?.extract()?;
            let node_id = index.str()?.to_string();
            let mut pairs = Vec::new();
            for item in series.call_method0("items")?.try_iter()? {
                let (column, cell): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
                if cell.is_none() {
                    continue;
                }
                let value: f64 = cell.extract().map_err(|_| {
                    value_error(format!(
                        "from_dataframe cell ({node_id:?}, {column}) is not numeric"
                    ))
                })?;
                if value.is_nan() {
                    continue;
                }
                let period = super::parse_period_id(&column.str()?.to_string())?;
                pairs.push((period, AmountOrScalar::scalar(value)));
            }
            rows.push((node_id, pairs));
        }

        for (node_id, pairs) in rows {
            let state = slf.take_ready()?;
            let ready = state.value(node_id, &pairs);
            slf.inner = Some(BuilderState::Ready(ready));
        }
        Ok(slf)
    }

    /// Set point-in-time availability dates for explicit observations.
    ///
    /// Parameters
    /// ----------
    /// node_id : str
    ///     Existing value or mixed node.
    /// availability_dates : Mapping[str, date | str] | Sequence[tuple[str, date | str]]
    ///     Period ids paired with the date each observation became
    ///     available; ``datetime.date``, ``datetime.datetime``,
    ///     ``pandas.Timestamp`` and ISO ``YYYY-MM-DD`` strings are accepted.
    ///     Unspecified observations default to the period's exclusive end.
    ///
    /// Returns
    /// -------
    /// ModelBuilder
    ///     This builder, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If a period/date is invalid or the node does not exist. The
    ///     builder stays usable.
    #[pyo3(text_signature = "($self, node_id, availability_dates)")]
    fn availability_dates<'py>(
        mut slf: PyRefMut<'py, Self>,
        node_id: &str,
        availability_dates: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let parsed = super::extract_period_pairs(availability_dates)?
            .into_iter()
            .map(|(period, date)| Ok((super::parse_period_id(&period)?, extract_date(&date)?)))
            .collect::<PyResult<Vec<_>>>()?;
        slf.ready_mut()?
            .try_availability_dates(node_id, &parsed)
            .map_err(statements_to_py)?;
        Ok(slf)
    }

    /// Add a computed node with a formula.
    ///
    /// The node id and formula are validated before anything changes, so an
    /// invalid formula raises and leaves the builder usable.
    ///
    /// Parameters
    /// ----------
    /// node_id : str
    ///     Node identifier.
    /// formula : str
    ///     DSL formula expression (e.g. ``"revenue - cogs"``).
    ///
    /// Returns
    /// -------
    /// ModelBuilder
    ///     This builder, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the node id is reserved, the formula is blank or does not
    ///     compile, periods were not set, or the builder was consumed.
    #[pyo3(text_signature = "($self, node_id, formula)")]
    fn compute<'py>(
        mut slf: PyRefMut<'py, Self>,
        node_id: &str,
        formula: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.ready_mut()?
            .try_compute(node_id, formula)
            .map_err(statements_to_py)?;
        Ok(slf)
    }

    /// Insert a pre-built node specification, replacing any node with the
    /// same id.
    ///
    /// This is the escape hatch for template code that assembles
    /// ``NodeSpec`` objects directly; prefer ``value`` / ``compute`` /
    /// ``mixed`` for ordinary construction. The node is validated with the
    /// rest of the model at ``build()``.
    ///
    /// Parameters
    /// ----------
    /// node : NodeSpec
    ///     Fully configured node specification.
    ///
    /// Returns
    /// -------
    /// ModelBuilder
    ///     This builder, for chaining.
    #[pyo3(text_signature = "($self, node)")]
    fn insert_node<'py>(
        mut slf: PyRefMut<'py, Self>,
        node: PyRef<'_, PyNodeSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec = node.inner.clone();
        let id: NodeId = spec.node_id.clone();
        match slf.inner.as_mut() {
            Some(BuilderState::NeedPeriods(b)) => {
                b.insert_node(id, spec);
            }
            Some(BuilderState::Ready(b)) => {
                b.insert_node(id, spec);
            }
            None => return Err(Self::consumed_error()),
        }
        Ok(slf)
    }

    /// Start configuring a mixed node.
    #[pyo3(text_signature = "($self, node_id)")]
    fn mixed(&mut self, node_id: &str) -> PyResult<PyMixedNodeBuilder> {
        let state = self.take_ready()?;
        Ok(PyMixedNodeBuilder {
            inner: Some(state.mixed(node_id)),
        })
    }

    /// Attach a forecast specification to an existing or new node.
    ///
    /// Returns
    /// -------
    /// ModelBuilder
    ///     This builder, for chaining.
    #[pyo3(text_signature = "($self, node_id, forecast_spec)")]
    fn forecast<'py>(
        mut slf: PyRefMut<'py, Self>,
        node_id: &str,
        forecast_spec: PyRef<'_, PyForecastSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let state = slf.take_ready()?;
        let ready = state.forecast(node_id, forecast_spec.inner.clone());
        slf.inner = Some(BuilderState::Ready(ready));
        Ok(slf)
    }

    /// Attach a where clause to the last added node.
    ///
    /// Returns
    /// -------
    /// ModelBuilder
    ///     This builder, for chaining.
    #[pyo3(text_signature = "($self, where_clause)")]
    fn where_clause<'py>(
        mut slf: PyRefMut<'py, Self>,
        where_clause: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let state = slf.take_ready()?;
        let ready = state.where_clause(where_clause);
        slf.inner = Some(BuilderState::Ready(ready));
        Ok(slf)
    }

    /// Add model-level metadata.
    ///
    /// Parameters
    /// ----------
    /// key : str
    ///     Metadata key (``"currency"`` is read by the evaluator to infer a
    ///     reporting currency for ``cs.*`` formulas).
    /// value : Any
    ///     Any JSON-serializable value: ``"USD"``, ``42``, ``{"source":
    ///     "erp"}``, a list. A string that parses as JSON is stored as that
    ///     JSON value; any other string is stored verbatim.
    ///     For ``key="currency"``, supply a supported three-letter currency-code
    ///     string such as ``"USD"``; ``build()`` raises ``ValueError`` for malformed
    ///     currency metadata.
    ///
    /// Returns
    /// -------
    /// ModelBuilder
    ///     This builder, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``value`` is not JSON-serializable, periods were not set, or
    ///     the builder was consumed.
    #[pyo3(text_signature = "($self, key, value)")]
    fn with_meta<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        key: &str,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::module_utils::py_to_json_value(py, value, "meta value")?;
        let state = slf.take_ready()?;
        let ready = state.with_meta(key, value);
        slf.inner = Some(BuilderState::Ready(ready));
        Ok(slf)
    }

    /// Add all built-in statement metrics (``fin.*`` namespace).
    ///
    /// Returns
    /// -------
    /// ModelBuilder
    ///     This builder, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the built-in catalog cannot be loaded, periods were not set, or
    ///     the builder was consumed.
    #[pyo3(text_signature = "($self)")]
    fn with_builtin_metrics(mut slf: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        slf.ready_mut()?
            .try_with_builtin_metrics()
            .map_err(statements_to_py)?;
        Ok(slf)
    }

    /// Add one metric and its dependencies from a registry.
    ///
    /// Returns
    /// -------
    /// ModelBuilder
    ///     This builder, for chaining.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``qualified_id`` or one of its dependencies is not in
    ///     ``registry``. The builder stays usable.
    /// ValueError
    ///     If the id is not ``namespace.metric`` shaped, periods were not
    ///     set, or the builder was consumed.
    #[pyo3(text_signature = "($self, qualified_id, registry)")]
    fn add_metric_from_registry<'py>(
        mut slf: PyRefMut<'py, Self>,
        qualified_id: &str,
        registry: PyRef<'_, PyRegistry>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.ready_mut()?
            .try_add_metric_from_registry(qualified_id, &registry.inner)
            .map_err(statements_to_py)?;
        Ok(slf)
    }

    /// Add a fixed-rate bond to the capital structure (US conventions: 30/360, semi-annual).
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique instrument identifier.
    /// notional : Money
    ///     Principal amount (must be in a valid Currency).
    /// coupon_rate : float
    ///     Annual coupon rate as a decimal fraction (e.g. 0.05 for 5%).
    /// issue_date, maturity_date : datetime.date | str
    ///     Bond issue and maturity dates (date-like or ISO ``YYYY-MM-DD``).
    /// discount_curve_id : str
    ///     Discount curve identifier used for pricing.
    #[pyo3(
        text_signature = "($self, id, notional, coupon_rate, issue_date, maturity_date, discount_curve_id)"
    )]
    fn add_bond<'py>(
        mut slf: PyRefMut<'py, Self>,
        id: &str,
        notional: PyRef<'_, PyMoney>,
        coupon_rate: f64,
        issue_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        discount_curve_id: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let notional = notional.inner;
        let issue = extract_date(issue_date)?;
        let maturity = extract_date(maturity_date)?;
        let state = slf.take_any()?;
        let next = match state {
            BuilderState::NeedPeriods(b) => BuilderState::NeedPeriods(
                b.add_bond(
                    id,
                    notional,
                    coupon_rate,
                    issue,
                    maturity,
                    discount_curve_id,
                )
                .map_err(statements_to_py)?,
            ),
            BuilderState::Ready(b) => BuilderState::Ready(
                b.add_bond(
                    id,
                    notional,
                    coupon_rate,
                    issue,
                    maturity,
                    discount_curve_id,
                )
                .map_err(statements_to_py)?,
            ),
        };
        slf.inner = Some(next);
        Ok(slf)
    }

    /// Add an interest rate swap to the capital structure (US conventions).
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique instrument identifier.
    /// notional : Money
    ///     Swap notional.
    /// fixed_rate : float
    ///     Fixed leg rate as a decimal fraction (e.g. 0.04 for 4%).
    /// start_date, maturity_date : datetime.date | str
    ///     Effective and maturity dates (date-like or ISO ``YYYY-MM-DD``).
    /// discount_curve_id, forward_curve_id : str
    ///     Discount curve and floating-leg forward curve identifiers.
    #[pyo3(
        text_signature = "($self, id, notional, fixed_rate, start_date, maturity_date, discount_curve_id, forward_curve_id)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn add_swap<'py>(
        mut slf: PyRefMut<'py, Self>,
        id: &str,
        notional: PyRef<'_, PyMoney>,
        fixed_rate: f64,
        start_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        discount_curve_id: &str,
        forward_curve_id: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let notional = notional.inner;
        let start = extract_date(start_date)?;
        let maturity = extract_date(maturity_date)?;
        let state = slf.take_any()?;
        let next = match state {
            BuilderState::NeedPeriods(b) => BuilderState::NeedPeriods(
                b.add_swap(
                    id,
                    notional,
                    fixed_rate,
                    start,
                    maturity,
                    discount_curve_id,
                    forward_curve_id,
                )
                .map_err(statements_to_py)?,
            ),
            BuilderState::Ready(b) => BuilderState::Ready(
                b.add_swap(
                    id,
                    notional,
                    fixed_rate,
                    start,
                    maturity,
                    discount_curve_id,
                    forward_curve_id,
                )
                .map_err(statements_to_py)?,
            ),
        };
        slf.inner = Some(next);
        Ok(slf)
    }

    /// Add a fixed-rate bond with a market convention preset.
    ///
    /// Applies regional day-count, coupon-frequency, and calendar conventions
    /// automatically; ``add_bond`` uses US corporate conventions instead.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique instrument identifier.
    /// notional : Money
    ///     Principal amount (must be in a valid Currency).
    /// coupon_rate : float
    ///     Annual coupon rate as a decimal fraction (e.g. ``0.03`` for 3%).
    /// issue_date, maturity_date : datetime.date | str
    ///     Bond issue and maturity dates (date-like or ISO ``YYYY-MM-DD``).
    /// convention : str
    ///     Regional convention preset, as the canonical snake_case
    ///     identifier: ``"us_treasury"``, ``"us_agency"``, ``"german_bund"``,
    ///     ``"uk_gilt"``, ``"french_oat"``, ``"jgb"``, ``"us_corporate"``,
    ///     or ``"eur_corporate"``.
    /// discount_curve_id : str
    ///     Discount curve identifier used for pricing.
    #[pyo3(
        text_signature = "($self, id, notional, coupon_rate, issue_date, maturity_date, convention, discount_curve_id)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn add_bond_with_convention<'py>(
        mut slf: PyRefMut<'py, Self>,
        id: &str,
        notional: PyRef<'_, PyMoney>,
        coupon_rate: f64,
        issue_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        convention: &str,
        discount_curve_id: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let notional = notional.inner;
        let issue = extract_date(issue_date)?;
        let maturity = extract_date(maturity_date)?;
        let convention: finstack_quant_valuations::instruments::BondConvention =
            finstack_quant_core::wire::serde_parse(convention).map_err(|e| {
                value_error(format!(
                    "invalid bond convention {convention:?}: {e}; expected one of \
                     us_treasury, us_agency, german_bund, uk_gilt, french_oat, jgb, \
                     us_corporate, eur_corporate"
                ))
            })?;
        let rate = finstack_quant_core::types::Rate::from_decimal(coupon_rate)
            .map_err(crate::errors::core_to_py)?;
        let state = slf.take_any()?;
        let next = match state {
            BuilderState::NeedPeriods(b) => BuilderState::NeedPeriods(
                b.add_bond_with_convention(
                    id,
                    notional,
                    rate,
                    issue,
                    maturity,
                    convention,
                    discount_curve_id,
                )
                .map_err(statements_to_py)?,
            ),
            BuilderState::Ready(b) => BuilderState::Ready(
                b.add_bond_with_convention(
                    id,
                    notional,
                    rate,
                    issue,
                    maturity,
                    convention,
                    discount_curve_id,
                )
                .map_err(statements_to_py)?,
            ),
        };
        slf.inner = Some(next);
        Ok(slf)
    }

    /// Add an interest rate swap with custom leg conventions.
    ///
    /// Exposes day-count, frequency, and business-day-convention parameters
    /// for non-USD swaps (e.g. EUR annual ACT/360 fixed legs); ``add_swap``
    /// uses US conventions instead.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique instrument identifier.
    /// notional : Money
    ///     Swap notional.
    /// fixed_rate : float
    ///     Fixed leg rate as a decimal fraction (e.g. ``0.04`` for 4%).
    /// start_date, maturity_date : datetime.date | str
    ///     Effective and maturity dates of the swap schedule.
    /// discount_curve_id, forward_curve_id : str
    ///     Discount curve and floating-leg forward curve identifiers.
    /// fixed_frequency : Tenor or str
    ///     Payment frequency of the fixed leg (e.g. ``"1Y"``).
    /// fixed_day_count : DayCount
    ///     Day-count convention applied to the fixed leg.
    /// float_frequency : Tenor or str
    ///     Payment / fixing frequency of the floating leg (e.g. ``"3M"``).
    /// float_day_count : DayCount
    ///     Day-count convention applied to the floating leg.
    /// business_day_convention : BusinessDayConvention or str, optional
    ///     Schedule-date rolling convention (default Modified Following).
    #[pyo3(
        signature = (id, notional, fixed_rate, start_date, maturity_date, discount_curve_id, forward_curve_id, fixed_frequency, fixed_day_count, float_frequency, float_day_count, business_day_convention=None),
        text_signature = "($self, id, notional, fixed_rate, start_date, maturity_date, discount_curve_id, forward_curve_id, fixed_frequency, fixed_day_count, float_frequency, float_day_count, business_day_convention=None)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn add_swap_with_conventions<'py>(
        mut slf: PyRefMut<'py, Self>,
        id: &str,
        notional: PyRef<'_, PyMoney>,
        fixed_rate: f64,
        start_date: &Bound<'_, PyAny>,
        maturity_date: &Bound<'_, PyAny>,
        discount_curve_id: &str,
        forward_curve_id: &str,
        fixed_frequency: &Bound<'_, PyAny>,
        fixed_day_count: PyRef<'_, crate::bindings::core::dates::daycount::PyDayCount>,
        float_frequency: &Bound<'_, PyAny>,
        float_day_count: PyRef<'_, crate::bindings::core::dates::daycount::PyDayCount>,
        business_day_convention: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let notional = notional.inner;
        let start = extract_date(start_date)?;
        let maturity = extract_date(maturity_date)?;
        let fixed_frequency = crate::bindings::core::dates::tenor::extract_tenor(fixed_frequency)?;
        let float_frequency = crate::bindings::core::dates::tenor::extract_tenor(float_frequency)?;
        let fixed_day_count = fixed_day_count.inner;
        let float_day_count = float_day_count.inner;
        let resolved_business_day_convention = match business_day_convention {
            Some(obj) => {
                crate::bindings::core::dates::calendar::extract_business_day_convention(obj)?
            }
            None => finstack_quant_core::dates::BusinessDayConvention::ModifiedFollowing,
        };
        let state = slf.take_any()?;
        let next = match state {
            BuilderState::NeedPeriods(b) => BuilderState::NeedPeriods(
                b.add_swap_with_conventions(
                    id,
                    notional,
                    fixed_rate,
                    start,
                    maturity,
                    discount_curve_id,
                    forward_curve_id,
                    fixed_frequency,
                    fixed_day_count,
                    float_frequency,
                    float_day_count,
                    resolved_business_day_convention,
                )
                .map_err(statements_to_py)?,
            ),
            BuilderState::Ready(b) => BuilderState::Ready(
                b.add_swap_with_conventions(
                    id,
                    notional,
                    fixed_rate,
                    start,
                    maturity,
                    discount_curve_id,
                    forward_curve_id,
                    fixed_frequency,
                    fixed_day_count,
                    float_frequency,
                    float_day_count,
                    resolved_business_day_convention,
                )
                .map_err(statements_to_py)?,
            ),
        };
        slf.inner = Some(next);
        Ok(slf)
    }

    /// Add a debt instrument to the capital structure.
    ///
    /// Use this for supported capital-structure instruments not covered by the
    /// convenience constructors: bonds, convertible bonds, term loans, RCFs,
    /// interest-rate swaps, caps/floors, and swaptions.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Unique instrument identifier.
    /// instrument : Bond | TermLoan | RevolvingCredit | ConvertibleBond | InterestRateSwap | CapFloor | Swaption | str
    ///     A typed ``finstack_quant.valuations.instruments`` object, or its
    ///     ``finstack_quant.instrument/1`` envelope JSON string. Bare
    ///     instrument payloads without the envelope are rejected.
    ///
    /// Returns
    /// -------
    /// ModelBuilder
    ///     This builder, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the envelope is invalid or the instrument type is not supported
    ///     in a financial-statement capital structure (e.g. equity, CDS).
    #[pyo3(text_signature = "($self, id, instrument)")]
    fn add_debt<'py>(
        mut slf: PyRefMut<'py, Self>,
        id: &str,
        instrument: &Bound<'_, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let envelope = crate::bindings::extract::extract_instrument_json(instrument)?;
        let spec = finstack_quant_valuations::pricer::json::parse_instrument_from_json(&envelope)
            .map_err(core_to_py)?;
        let spec = FinancialStatementInstrument::try_from(spec).map_err(statements_to_py)?;
        let state = slf.take_any()?;
        let next = match state {
            BuilderState::NeedPeriods(b) => BuilderState::NeedPeriods(b.add_debt(id, spec)),
            BuilderState::Ready(b) => BuilderState::Ready(b.add_debt(id, spec)),
        };
        slf.inner = Some(next);
        Ok(slf)
    }

    /// Set the reporting currency used for capital-structure totals.
    ///
    /// Returns
    /// -------
    /// ModelBuilder
    ///     This builder, for chaining.
    #[pyo3(text_signature = "($self, currency)")]
    fn reporting_currency<'py>(
        mut slf: PyRefMut<'py, Self>,
        currency: PyRef<'_, PyCurrency>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let ccy = currency.inner;
        let state = slf.take_any()?;
        let next = match state {
            BuilderState::NeedPeriods(b) => BuilderState::NeedPeriods(b.reporting_currency(ccy)),
            BuilderState::Ready(b) => BuilderState::Ready(b.reporting_currency(ccy)),
        };
        slf.inner = Some(next);
        Ok(slf)
    }

    /// Set the FX conversion policy for capital-structure cashflows.
    ///
    /// Parameters
    /// ----------
    /// policy : str
    ///     One of ``"cashflow_date"``, ``"period_end"``, ``"period_average"``.
    ///
    /// Returns
    /// -------
    /// ModelBuilder
    ///     This builder, for chaining.
    #[pyo3(text_signature = "($self, policy)")]
    fn fx_policy<'py>(mut slf: PyRefMut<'py, Self>, policy: &str) -> PyResult<PyRefMut<'py, Self>> {
        let parsed: FxConversionPolicy =
            finstack_quant_core::wire::serde_parse(policy).map_err(|e| {
                value_error(format!(
                    "invalid fx_policy {policy:?}: {e}; expected one of cashflow_date, period_end, period_average"
                ))
            })?;
        let state = slf.take_any()?;
        let next = match state {
            BuilderState::NeedPeriods(b) => BuilderState::NeedPeriods(b.fx_policy(parsed)),
            BuilderState::Ready(b) => BuilderState::Ready(b.fx_policy(parsed)),
        };
        slf.inner = Some(next);
        Ok(slf)
    }

    /// Attach a waterfall specification (priority-of-payments, ECF sweep,
    /// PIK toggle, payment classes, and optional prepay nodes).
    ///
    /// Parameters
    /// ----------
    /// waterfall_spec : WaterfallSpec
    ///     Validated or pending waterfall configuration attached to the
    ///     capital structure. Bond / ConvertibleBond plus a sweep is rejected
    ///     at model build.
    ///
    /// Returns
    /// -------
    /// ModelBuilder
    ///     This builder, for chaining.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder has already been consumed.
    #[pyo3(text_signature = "($self, waterfall_spec)")]
    fn waterfall<'py>(
        mut slf: PyRefMut<'py, Self>,
        waterfall_spec: PyRef<'_, PyWaterfallSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let spec = waterfall_spec.inner.clone();
        let state = slf.take_any()?;
        let next = match state {
            BuilderState::NeedPeriods(b) => BuilderState::NeedPeriods(b.waterfall(spec)),
            BuilderState::Ready(b) => BuilderState::Ready(b.waterfall(spec)),
        };
        slf.inner = Some(next);
        Ok(slf)
    }

    /// Build the model specification.
    ///
    /// Returns
    /// -------
    /// FinancialModelSpec
    ///     The completed model specification.
    #[pyo3(text_signature = "($self)")]
    fn build(&mut self) -> PyResult<PyFinancialModelSpec> {
        let state = self.take_ready()?;
        let spec = state.build().map_err(statements_to_py)?;
        Ok(PyFinancialModelSpec { inner: spec })
    }

    /// Return ``ModelBuilder(id='acme', state='ready', periods=4, nodes=3)``.
    ///
    /// ``state`` is ``'need_periods'`` before ``periods()``, ``'ready'``
    /// afterwards, and ``'consumed'`` once ``build()`` / ``mixed()`` ran.
    fn __repr__(&self) -> String {
        match &self.inner {
            Some(BuilderState::NeedPeriods(b)) => format!(
                "ModelBuilder(id={:?}, state='need_periods', periods=0, nodes={})",
                b.id(),
                b.nodes().len()
            ),
            Some(BuilderState::Ready(b)) => format!(
                "ModelBuilder(id={:?}, state='ready', periods={}, nodes={})",
                b.id(),
                b.periods_slice().len(),
                b.nodes().len()
            ),
            None => "ModelBuilder(state='consumed')".to_string(),
        }
    }
}

impl PyModelBuilder {
    fn consumed_error() -> PyErr {
        value_error(
            "Builder is no longer usable: it was consumed by build()/mixed(). Construct a new \
             ModelBuilder.",
        )
    }

    fn take_any(&mut self) -> PyResult<BuilderState> {
        self.inner.take().ok_or_else(Self::consumed_error)
    }

    fn take_ready(&mut self) -> PyResult<ModelBuilder<finstack_quant_statements::builder::Ready>> {
        let state = self.take_any()?;
        match state {
            BuilderState::Ready(b) => Ok(b),
            BuilderState::NeedPeriods(b) => {
                self.inner = Some(BuilderState::NeedPeriods(b));
                Err(value_error("Must call periods() before adding nodes"))
            }
        }
    }

    /// Mutable access to the ready builder without taking ownership, so a
    /// fallible Rust call cannot leave the wrapper empty.
    fn ready_mut(
        &mut self,
    ) -> PyResult<&mut ModelBuilder<finstack_quant_statements::builder::Ready>> {
        match self.inner.as_mut() {
            Some(BuilderState::Ready(b)) => Ok(b),
            Some(BuilderState::NeedPeriods(_)) => {
                Err(value_error("Must call periods() before adding nodes"))
            }
            None => Err(Self::consumed_error()),
        }
    }
}

/// Register builder classes.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyModelBuilder>()?;
    m.add_class::<PyMixedNodeBuilder>()?;
    m.add_class::<PyRegistry>()?;
    m.add_class::<PyMetricDefinition>()?;
    Ok(())
}
