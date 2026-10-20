//! Python wrapper for `ModelBuilder`.
//!
//! Every fallible configuration step goes through the crate's non-consuming
//! `try_*` twins, so a typo in a formula, period range, or metric id raises
//! without discarding the model accumulated so far.

use super::capital_structure::PyWaterfallSpec;
use super::registry::PyRegistry;
use super::types::{PyFinancialModelSpec, PyForecastSpec, PyNodeSpec};
use super::{extract_money_series, extract_scalar_series, extract_value_series, PERIOD_GRAMMAR};
use crate::bindings::core::currency::PyCurrency;
use crate::bindings::core::dates::periods::PyPeriod;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::py_to_date;
use crate::errors::{core_to_py, statements_to_py, value_error};
use finstack_quant_core::dates::{Period, PeriodId};
use finstack_quant_core::money::fx::FxConversionPolicy;
use finstack_quant_statements::builder::{MixedNodeBuilder, ModelBuilder};
use finstack_quant_statements::capital_structure::{
    BondConventionParams, SwapConventions, SwapParams,
};
use finstack_quant_statements::types::{AmountOrScalar, FinancialStatementInstrument};
use pyo3::prelude::*;

/// Builder for financial models.
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
    inner: Option<ModelBuilder>,
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
        Ok(PyModelBuilder { inner: Some(ready) })
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
    /// Create a new model builder awaiting ``periods(...)``.
    #[new]
    #[pyo3(text_signature = "(id)")]
    fn new(id: &str) -> Self {
        Self {
            inner: Some(ModelBuilder::new(id)),
        }
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
            inner: Some(builder),
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
        slf.builder_mut()?
            .try_periods(range, actuals_until)
            .map_err(|error| {
                value_error(format!(
                    "invalid period range {range:?}: {error}; {PERIOD_GRAMMAR}"
                ))
            })?;
        Ok(slf)
    }

    /// Define the timeline from explicit ``Period`` objects.
    ///
    /// Use this for timelines the range grammar cannot express (a fiscal
    /// calendar from ``core.dates.build_fiscal_periods``, or a hand-built
    /// list). The periods are kept in the given order; ``build()`` rejects
    /// timelines that are not strictly increasing, whose intervals are empty
    /// or overlapping, or whose actuals do not form a prefix. Gaps are allowed.
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
        if !slf.builder_mut()?.periods_slice().is_empty() {
            return Err(value_error("Periods already set"));
        }
        let builder = slf.take()?;
        slf.inner = Some(
            builder
                .periods_explicit(periods)
                .map_err(statements_to_py)?,
        );
        Ok(slf)
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
    ///     If a period id does not parse or the
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
        let state = slf.take()?;
        let ready = state.value(node_id, &parsed);
        slf.inner = Some(ready);
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
        let state = slf.take()?;
        let ready = state.value_scalar(node_id, &parsed);
        slf.inner = Some(ready);
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
        let state = slf.take()?;
        let ready = state.value_money(node_id, &parsed);
        slf.inner = Some(ready);
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
    ///     If the frame has no columns, a column label is invalid or outside
    ///     the model timeline, a node id is reserved, a cell is not numeric,
    ///     or the builder was consumed. Column labels are checked even when
    ///     all their cells are missing. Rejected input leaves the builder
    ///     unchanged.
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
        let columns: Vec<PeriodId> = frame
            .getattr("columns")?
            .try_iter()?
            .map(|column| super::parse_period_id(&column?.str()?.to_string()))
            .collect::<PyResult<Vec<_>>>()?;
        let periods = slf
            .builder_mut()?
            .resolve_period_columns(&columns, actuals_until)
            .map_err(statements_to_py)?;

        let mut rows: Vec<(String, Vec<(PeriodId, AmountOrScalar)>)> = Vec::new();
        for row in frame.call_method0("iterrows")?.try_iter()? {
            let (index, series): (Bound<'py, PyAny>, Bound<'py, PyAny>) = row?.extract()?;
            let node_id = index.str()?.to_string();
            finstack_quant_statements::builder::validate_node_id(&node_id)
                .map_err(statements_to_py)?;
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

        let mut builder = slf.take()?;
        if builder.periods_slice().is_empty() {
            builder = builder
                .periods_explicit(periods)
                .map_err(statements_to_py)?;
        }
        for (node_id, pairs) in rows {
            builder = builder.value(node_id, &pairs);
        }
        slf.inner = Some(builder);
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
            .map(|(period, date)| Ok((super::parse_period_id(&period)?, py_to_date(&date)?)))
            .collect::<PyResult<Vec<_>>>()?;
        slf.builder_mut()?
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
    ///     compile or the builder was consumed.
    #[pyo3(text_signature = "($self, node_id, formula)")]
    fn compute<'py>(
        mut slf: PyRefMut<'py, Self>,
        node_id: &str,
        formula: &str,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.builder_mut()?
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
        slf.builder_mut()?.insert_node(spec);
        Ok(slf)
    }

    /// Start configuring a mixed node.
    #[pyo3(text_signature = "($self, node_id)")]
    fn mixed(&mut self, node_id: &str) -> PyResult<PyMixedNodeBuilder> {
        let state = self.take()?;
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
        let state = slf.take()?;
        let ready = state.forecast(node_id, forecast_spec.inner.clone());
        slf.inner = Some(ready);
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
        let state = slf.take()?;
        let ready = state.where_clause(where_clause);
        slf.inner = Some(ready);
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
    ///     If ``value`` is not JSON-serializable or
    ///     the builder was consumed.
    #[pyo3(text_signature = "($self, key, value)")]
    fn with_meta<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
        key: &str,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let value = crate::bindings::module_utils::py_to_json_value(py, value, "meta value")?;
        let state = slf.take()?;
        let ready = state.with_meta(key, value);
        slf.inner = Some(ready);
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
    ///     If the built-in catalog cannot be loaded or
    ///     the builder was consumed.
    #[pyo3(text_signature = "($self)")]
    fn with_builtin_metrics(mut slf: PyRefMut<'_, Self>) -> PyResult<PyRefMut<'_, Self>> {
        slf.builder_mut()?
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
    ///     If the id is not ``namespace.metric`` shaped or the builder was
    ///     consumed.
    #[pyo3(text_signature = "($self, qualified_id, registry)")]
    fn add_metric_from_registry<'py>(
        mut slf: PyRefMut<'py, Self>,
        qualified_id: &str,
        registry: PyRef<'_, PyRegistry>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        slf.builder_mut()?
            .try_add_metric_from_registry(qualified_id, &registry.inner)
            .map_err(statements_to_py)?;
        Ok(slf)
    }

    /// Add a fixed-rate bond to the capital structure (US conventions: 30/360, semi-annual).
    ///
    /// Rejected input leaves this builder usable with its accumulated model intact.
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
        let issue = py_to_date(issue_date)?;
        let maturity = py_to_date(maturity_date)?;
        slf.builder_mut()?
            .try_add_bond(
                id,
                notional,
                coupon_rate,
                issue,
                maturity,
                discount_curve_id,
            )
            .map_err(statements_to_py)?;
        Ok(slf)
    }

    /// Add an interest rate swap to the capital structure (US conventions).
    ///
    /// Rejected input leaves this builder usable with its accumulated model intact.
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
        let start = py_to_date(start_date)?;
        let maturity = py_to_date(maturity_date)?;
        let params = SwapParams {
            id: id.to_string(),
            notional,
            fixed_rate,
            start_date: start,
            maturity_date: maturity,
            discount_curve_id: discount_curve_id.to_string(),
            forward_curve_id: forward_curve_id.to_string(),
        };
        slf.builder_mut()?
            .try_add_swap(params)
            .map_err(statements_to_py)?;
        Ok(slf)
    }

    /// Add a fixed-rate bond with a market convention preset.
    ///
    /// Applies regional day-count, coupon-frequency, and calendar conventions
    /// automatically; ``add_bond`` uses US corporate conventions instead.
    ///
    /// Rejected input leaves this builder usable with its accumulated model intact.
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
        let issue = py_to_date(issue_date)?;
        let maturity = py_to_date(maturity_date)?;
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
        let params = BondConventionParams {
            id: id.to_string(),
            notional,
            coupon_rate: rate,
            issue_date: issue,
            maturity_date: maturity,
            convention,
            discount_curve_id: discount_curve_id.to_string(),
        };
        slf.builder_mut()?
            .try_add_bond_with_convention(params)
            .map_err(statements_to_py)?;
        Ok(slf)
    }

    /// Add an interest rate swap with custom leg conventions.
    ///
    /// Exposes day-count, frequency, and business-day-convention parameters
    /// for non-USD swaps (e.g. EUR annual ACT/360 fixed legs); ``add_swap``
    /// uses US conventions instead.
    ///
    /// Rejected input leaves this builder usable with its accumulated model intact.
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
        let start = py_to_date(start_date)?;
        let maturity = py_to_date(maturity_date)?;
        let fixed_frequency = crate::bindings::core::dates::tenor::extract_tenor(fixed_frequency)?;
        let float_frequency = crate::bindings::core::dates::tenor::extract_tenor(float_frequency)?;
        let fixed_day_count = fixed_day_count.inner;
        let float_day_count = float_day_count.inner;
        let resolved_business_day_convention = match business_day_convention {
            Some(obj) => {
                crate::bindings::core::dates::calendar::extract_business_day_convention(obj)?
            }
            None => finstack_quant_core::dates::BusinessDayConvention::default(),
        };
        let params = SwapParams {
            id: id.to_string(),
            notional,
            fixed_rate,
            start_date: start,
            maturity_date: maturity,
            discount_curve_id: discount_curve_id.to_string(),
            forward_curve_id: forward_curve_id.to_string(),
        };
        let conventions = SwapConventions {
            fixed_frequency,
            fixed_day_count,
            float_frequency,
            float_day_count,
            business_day_convention: resolved_business_day_convention,
        };
        slf.builder_mut()?
            .try_add_swap_with_conventions(params, conventions)
            .map_err(statements_to_py)?;
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
        let spec = crate::bindings::extract::extract_instrument(instrument)?
            .into_instrument_json()
            .map_err(core_to_py)?;
        let spec = FinancialStatementInstrument::try_from(spec).map_err(statements_to_py)?;
        let builder = slf.take()?;
        slf.inner = Some(builder.add_debt(id, spec));
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
        let builder = slf.take()?;
        slf.inner = Some(builder.reporting_currency(ccy));
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
        let builder = slf.take()?;
        slf.inner = Some(builder.fx_policy(parsed));
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
        let builder = slf.take()?;
        slf.inner = Some(builder.waterfall(spec));
        Ok(slf)
    }

    /// Build the model specification.
    ///
    /// Validates node identities, explicit-value periods, forecasts, formula
    /// dimensions, and positive, nonoverlapping timeline intervals. The node
    /// identifier ``period_id`` is reserved for the result-export timeline.
    ///
    /// Returns
    /// -------
    /// FinancialModelSpec
    ///     The completed model specification.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the builder was consumed or its model (including its timeline) violates a
    ///     semantic invariant. This terminal method consumes the ready builder.
    #[pyo3(text_signature = "($self)")]
    fn build(&mut self) -> PyResult<PyFinancialModelSpec> {
        let state = self.take()?;
        let spec = state.build().map_err(statements_to_py)?;
        Ok(PyFinancialModelSpec { inner: spec })
    }

    /// Return ``ModelBuilder(id='acme', state='ready', periods=4, nodes=3)``.
    ///
    /// ``state`` is ``'need_periods'`` before ``periods()``, ``'ready'``
    /// afterwards, and ``'consumed'`` once ``build()`` / ``mixed()`` ran.
    fn __repr__(&self) -> String {
        match &self.inner {
            Some(b) => format!(
                "ModelBuilder(id={:?}, state='{}', periods={}, nodes={})",
                b.id(),
                if b.periods_slice().is_empty() {
                    "need_periods"
                } else {
                    "ready"
                },
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

    fn take(&mut self) -> PyResult<ModelBuilder> {
        self.inner.take().ok_or_else(Self::consumed_error)
    }

    /// Mutable access to the builder without taking ownership, so a
    /// fallible Rust call cannot leave the wrapper empty.
    fn builder_mut(&mut self) -> PyResult<&mut ModelBuilder> {
        self.inner.as_mut().ok_or_else(Self::consumed_error)
    }
}

/// Register builder classes.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyModelBuilder>()?;
    m.add_class::<PyMixedNodeBuilder>()?;
    Ok(())
}
