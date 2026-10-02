//! Python wrappers for the statement evaluator and results.

use super::capital_structure::PyCapitalStructureCashflows;
use super::monte_carlo::{extract_config, PyMonteCarloResults};
use super::parse_period_id;
use crate::bindings::core::money::PyMoney;
use crate::bindings::date_utils::{date_to_py, extract_date};
use crate::bindings::extract::{extract_market_ref, extract_model_ref};
use crate::bindings::pandas_utils::{
    selected_table_to_dataframe, table_to_dataframe, values_to_series,
};
use crate::errors::{serde_json_to_py, statements_to_py};
use finstack_quant_statements::evaluator::PeriodDateConvention;
use pyo3::exceptions::PyKeyError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Results from evaluating a financial model.
#[pyclass(
    name = "StatementResult",
    module = "finstack_quant.statements",
    skip_from_py_object
)]
#[derive(Clone)]
pub struct PyStatementResult {
    pub(crate) inner: finstack_quant_statements::evaluator::StatementResult,
}

#[pymethods]
impl PyStatementResult {
    /// Support `pickle` (and therefore `multiprocessing`, `joblib`, `dask`).
    ///
    /// Reconstruction goes through the same strict serde round-trip as
    /// `to_json` / `from_json`, so an unpickled value is exactly what the wire
    /// format defines — there is no second state format that can drift.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Deserialize from JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json, /)")]
    fn from_json(json: &str) -> PyResult<Self> {
        let inner: finstack_quant_statements::evaluator::StatementResult =
            serde_json::from_str(json)
                .map_err(|e| serde_json_to_py(e, "invalid StatementResult JSON"))?;
        Ok(Self { inner })
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| serde_json_to_py(e, "failed to serialize StatementResult"))
    }

    /// Get the value for a node at a specific period.
    ///
    /// Returns the f64 view of whichever source won this period under the
    /// crate's **Value > Forecast > Formula** precedence rule.
    ///
    /// Parameters
    /// ----------
    /// node_id : str
    ///     Node identifier (e.g. ``"revenue"``).
    /// period : str
    ///     Period identifier string (e.g. ``"2025Q1"``).
    ///
    /// Returns
    /// -------
    /// float | None
    ///     The value in the node's own units — a currency amount for
    ///     monetary nodes (currency not carried; use :meth:`get_money` for
    ///     that) and a unitless scalar otherwise. ``None`` when the node or
    ///     period is unknown.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``period`` is not a valid period id; the message restates the
    ///     accepted grammar (``2025Q1``, ``2025M3``, ``2025``, ...).
    #[pyo3(text_signature = "($self, node_id, period)")]
    fn get(&self, node_id: &str, period: &str) -> PyResult<Option<f64>> {
        let pid = parse_period_id(period)?;
        Ok(self.inner.get(node_id, &pid))
    }

    /// Get the monetary value for a node at a specific period.
    ///
    /// Returns the currency-tagged ``Money`` value for ``Money``-typed nodes,
    /// preserving fixed-point precision and currency. Returns ``None`` when
    /// the node is not monetary or has no value for this period.
    ///
    /// Parameters
    /// ----------
    /// node_id : str
    ///     Node identifier (e.g. ``"revenue"``).
    /// period : str
    ///     Period identifier string (e.g. ``"2025Q1"``).
    #[pyo3(text_signature = "($self, node_id, period)")]
    fn get_money(&self, node_id: &str, period: &str) -> PyResult<Option<PyMoney>> {
        let pid = parse_period_id(period)?;
        Ok(self
            .inner
            .get_money(node_id, &pid)
            .map(|inner| PyMoney { inner }))
    }

    /// Get the scalar value for a non-monetary node at a specific period.
    ///
    /// Returns ``None`` when the node is monetary or has no value for this
    /// period.
    ///
    /// Parameters
    /// ----------
    /// node_id : str
    ///     Node identifier (e.g. ``"gross_margin_pct"``).
    /// period : str
    ///     Period identifier string (e.g. ``"2025Q1"``).
    #[pyo3(text_signature = "($self, node_id, period)")]
    fn get_scalar(&self, node_id: &str, period: &str) -> PyResult<Option<f64>> {
        let pid = parse_period_id(period)?;
        Ok(self.inner.get_scalar(node_id, &pid))
    }

    /// Get the value for a node at a period, or a default when missing.
    ///
    /// Parameters
    /// ----------
    /// node_id : str
    ///     Node identifier (e.g. ``"revenue"``).
    /// period : str
    ///     Period identifier string (e.g. ``"2025Q1"``).
    /// default : float
    ///     Value returned when the node or period is unknown.
    ///
    /// Returns
    /// -------
    /// float
    ///     The evaluated value in the node's own units, or ``default``.
    #[pyo3(text_signature = "($self, node_id, period, default)")]
    fn get_or(&self, node_id: &str, period: &str, default: f64) -> PyResult<f64> {
        let pid = parse_period_id(period)?;
        Ok(self.inner.get_or(node_id, &pid, default))
    }

    /// Get every evaluated period for one node as ordered pairs.
    ///
    /// Parameters
    /// ----------
    /// node_id : str
    ///     Node identifier (e.g. ``"revenue"``).
    ///
    /// Returns
    /// -------
    /// list[tuple[str, float]]
    ///     ``(period, value)`` pairs in evaluation order, in the node's own
    ///     units. Empty when the node is not in the result.
    #[pyo3(text_signature = "($self, node_id)")]
    fn all_periods(&self, node_id: &str) -> Vec<(String, f64)> {
        self.inner
            .all_periods(node_id)
            .map(|(pid, value)| (pid.to_string(), value))
            .collect()
    }

    /// Check report attached by an evaluator configured with
    /// :meth:`Evaluator.with_checks`, or ``None`` when no suite ran.
    #[getter]
    fn check_report(&self) -> Option<super::checks::PyCheckReport> {
        self.inner
            .check_report
            .clone()
            .map(|inner| super::checks::PyCheckReport { inner })
    }

    /// Capital-structure cashflows (interest, principal, fees, balances per
    /// instrument and period), or ``None`` when the model has no capital
    /// structure or was evaluated without a market context.
    #[getter]
    fn cs_cashflows(&self) -> Option<PyCapitalStructureCashflows> {
        self.inner
            .cs_cashflows
            .clone()
            .map(|inner| PyCapitalStructureCashflows { inner })
    }

    /// Get every evaluated period for one node as a dict.
    ///
    /// Parameters
    /// ----------
    /// node_id : str
    ///     Node identifier (e.g. ``"revenue"``).
    ///
    /// Returns
    /// -------
    /// dict[str, float] | None
    ///     Period identifier string → value, in evaluation order, in the
    ///     node's own units (currency amount for monetary nodes, unitless
    ///     otherwise). ``None`` when the node is not in the result.
    #[pyo3(text_signature = "($self, node_id)")]
    fn get_node<'py>(
        &self,
        py: Python<'py>,
        node_id: &str,
    ) -> PyResult<Option<Bound<'py, PyDict>>> {
        match self.inner.get_node(node_id) {
            Some(period_map) => {
                let dict = PyDict::new(py);
                for (pid, &val) in period_map {
                    dict.set_item(pid.to_string(), val)?;
                }
                Ok(Some(dict))
            }
            None => Ok(None),
        }
    }

    /// One node's evaluated series as a pandas ``Series``.
    ///
    /// Parameters
    /// ----------
    /// node_id : str
    ///     Node identifier (e.g. ``"revenue"``).
    ///
    /// Returns
    /// -------
    /// pd.Series
    ///     Float64 series named ``node_id``, indexed by period identifier
    ///     string in evaluation order, in the node's own units (currency
    ///     amount for monetary nodes — see :meth:`get_money` for the
    ///     currency — unitless otherwise).
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``node_id`` is not in the result.
    #[pyo3(text_signature = "($self, node_id)")]
    fn to_series<'py>(&self, py: Python<'py>, node_id: &str) -> PyResult<Bound<'py, PyAny>> {
        let period_map = self
            .inner
            .get_node(node_id)
            .ok_or_else(|| PyKeyError::new_err(format!("unknown node: {node_id:?}")))?;
        let labels: Vec<String> = period_map.keys().map(ToString::to_string).collect();
        let values: Vec<f64> = period_map.values().copied().collect();
        values_to_series(py, values, &labels, node_id)
    }

    /// Export one node as a dated cashflow schedule.
    ///
    /// Bridges period-based statement output into dated-cashflow consumers
    /// (real-estate NOI DCFs, valuation instruments). Periods are taken in
    /// ``model`` timeline order; periods without a value are skipped.
    ///
    /// Parameters
    /// ----------
    /// model : FinancialModelSpec | str
    ///     The model that produced this result (its periods supply the
    ///     dates); a typed model or its JSON.
    /// node_id : str
    ///     Node identifier to export.
    /// convention : {"end", "start"}, optional
    ///     ``"end"`` (the Rust default when omitted) dates each period on its
    ///     last inclusive day (``end - 1 day``, since periods are half-open
    ///     ``[start, end)``); ``"start"`` uses the period start date.
    ///
    /// Returns
    /// -------
    /// list[tuple[datetime.date, float]]
    ///     ``(date, value)`` pairs in timeline order, in the node's own
    ///     units.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If ``node_id`` is not in the result.
    /// ValueError
    ///     If ``convention`` is not ``"end"`` or ``"start"``.
    #[pyo3(signature = (model, node_id, convention=None), text_signature = "($self, model, node_id, convention=None)")]
    fn to_dated_schedule<'py>(
        &self,
        py: Python<'py>,
        model: &Bound<'py, PyAny>,
        node_id: &str,
        convention: Option<&str>,
    ) -> PyResult<Vec<(Bound<'py, PyAny>, f64)>> {
        let convention = match convention {
            Some(convention) => convention
                .parse::<PeriodDateConvention>()
                .map_err(statements_to_py)?,
            None => PeriodDateConvention::default(),
        };
        let model = extract_model_ref(model)?;
        let schedule = finstack_quant_statements::evaluator::node_to_dated_schedule(
            &model,
            &self.inner,
            node_id,
            convention,
        )
        .map_err(statements_to_py)?;
        schedule
            .into_iter()
            .map(|(date, value)| Ok((date_to_py(py, date)?, value)))
            .collect()
    }

    /// All node identifiers in the result, in evaluation order.
    ///
    /// Returns
    /// -------
    /// list[str]
    ///     Node ids as declared in the model graph.
    #[pyo3(text_signature = "($self)")]
    fn node_ids(&self) -> Vec<String> {
        self.inner.nodes.keys().cloned().collect()
    }

    /// Number of nodes in the result.
    #[getter]
    fn node_count(&self) -> usize {
        self.inner.nodes.len()
    }

    /// Number of periods evaluated, counted in **periods** on the model's own
    /// cadence (quarters, months, years), not months.
    #[getter]
    fn num_periods(&self) -> usize {
        self.inner.meta.num_periods
    }

    /// Wall-clock evaluation time in milliseconds, or ``None`` when the
    /// producing run did not record it.
    ///
    /// Diagnostic only — it is not part of the deterministic result, so two
    /// identical runs may report different values.
    #[getter]
    fn eval_time_ms(&self) -> Option<u64> {
        self.inner.meta.eval_time_ms
    }

    /// Number of evaluation warnings recorded.
    ///
    /// Warnings never fail an evaluation; see :attr:`warnings` for what was
    /// flagged.
    #[getter]
    fn warning_count(&self) -> usize {
        self.inner.meta.warnings.len()
    }

    /// Evaluation warnings in their serde form, one dict per ``EvalWarning``.
    ///
    /// Each dict has a single snake_case variant key (``"division_by_zero"``,
    /// ``"non_finite_value"``, ``"non_finite_aggregate_input"``, ...) whose
    /// value holds the variant fields: node ids as strings, periods as
    /// ``"2025Q1"``-style ids and non-finite numbers as ``"nan"`` / ``"inf"``
    /// / ``"-inf"``. The same objects appear in ``to_json()["meta"]`` and in
    /// the WASM ``Evaluator.evaluate`` result.
    #[getter]
    fn warnings<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.meta.warnings)
    }

    /// Numeric mode stamped into the result envelope (policy visibility).
    #[getter]
    fn numeric_mode(&self) -> super::types::PyNumericMode {
        super::types::PyNumericMode {
            inner: self.inner.meta.numeric_mode,
        }
    }

    /// Whether the evaluation ran in parallel (policy visibility).
    #[getter]
    fn parallel(&self) -> bool {
        self.inner.meta.parallel
    }

    /// Export to a pandas ``DataFrame``.
    ///
    /// Parameters
    /// ----------
    /// orient : {"long", "wide"}, default "long"
    ///     ``"long"`` yields one row per (node, period) with columns
    ///     ``node_id``, ``period``, ``value``, ``value_money``, ``currency``,
    ///     ``value_type``. ``"wide"`` yields node identifiers as rows and
    ///     period identifiers as columns.
    ///
    /// Notes
    /// -----
    /// In long format the monetary columns are populated for `Money`-typed
    /// nodes and left null for scalar nodes. ``value_money`` is a float64
    /// mirror of the monetary amount, so it carries f64 (not fixed-point
    /// Decimal) precision; use ``to_json()`` or ``get_money()`` when full
    /// fixed-point precision is required.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If ``orient`` is not ``"long"`` or ``"wide"``.
    #[pyo3(signature = (orient = "long"))]
    #[pyo3(text_signature = "($self, orient='long')")]
    fn to_dataframe<'py>(&self, py: Python<'py>, orient: &str) -> PyResult<Bound<'py, PyAny>> {
        match orient {
            "long" => {
                let table = self.inner.to_table_long().map_err(statements_to_py)?;
                selected_table_to_dataframe(
                    py,
                    &table,
                    &[
                        ("node_id", "node_id"),
                        ("period_id", "period"),
                        ("value", "value"),
                        ("value_money", "value_money"),
                        ("currency", "currency"),
                        ("value_type", "value_type"),
                    ],
                )
            }
            "wide" => {
                let table = self.inner.to_table_wide().map_err(statements_to_py)?;
                let df = table_to_dataframe(py, &table)?;
                df.call_method1("set_index", ("period_id",))?.getattr("T")
            }
            other => Err(pyo3::exceptions::PyValueError::new_err(format!(
                "orient must be 'long' or 'wide', got {other:?}"
            ))),
        }
    }

    /// Export the long-format table via Arrow (zero-copy for consumers).
    ///
    /// Returns an :class:`finstack_quant.core.table.ArrowTable` implementing
    /// ``__arrow_c_stream__``; pass it to ``pyarrow.table(...)``,
    /// ``polars.DataFrame(...)``, or DuckDB. Column values and
    /// monetary-mirror semantics match ``to_dataframe("long")``, plus column
    /// roles and table metadata are preserved as Arrow field/schema
    /// metadata. One column name differs: the period column here is
    /// ``period_id`` (the table envelope's native name), whereas
    /// ``to_dataframe("long")`` renames it to ``period``.
    #[pyo3(text_signature = "($self)")]
    fn to_arrow_long(&self) -> PyResult<crate::bindings::core::table::PyArrowTable> {
        let table = self.inner.to_table_long().map_err(statements_to_py)?;
        crate::bindings::core::table::PyArrowTable::from_envelope(&table)
    }

    /// Export the wide-format table via Arrow (zero-copy for consumers).
    ///
    /// Rows are periods (column ``period_id``), one ``float64`` column per
    /// node, matching ``to_dataframe("wide")`` before its transpose.
    #[pyo3(text_signature = "($self)")]
    fn to_arrow_wide(&self) -> PyResult<crate::bindings::core::table::PyArrowTable> {
        let table = self.inner.to_table_wide().map_err(statements_to_py)?;
        crate::bindings::core::table::PyArrowTable::from_envelope(&table)
    }

    /// Render as an HTML table in Jupyter notebooks.
    ///
    /// Delegates to ``to_dataframe("wide")`` (nodes as rows, periods as
    /// columns), so pandas' own row/column truncation applies. Returns
    /// ``None`` if the frame cannot be built, which makes IPython fall back
    /// to ``__repr__`` instead of raising from the display hook.
    fn _repr_html_(&self, py: Python<'_>) -> Option<String> {
        let frame = self.to_dataframe(py, "wide").ok()?;
        frame.call_method0("_repr_html_").ok()?.extract().ok()
    }

    /// Return the representation with node and period counts.
    fn __repr__(&self) -> String {
        format!(
            "StatementResult(nodes={}, periods={})",
            self.inner.nodes.len(),
            self.inner.meta.num_periods
        )
    }
}

/// Evaluator for financial models.
#[pyclass(
    name = "Evaluator",
    module = "finstack_quant.statements",
    skip_from_py_object
)]
pub struct PyEvaluator {
    inner: finstack_quant_statements::evaluator::Evaluator,
}

#[pymethods]
impl PyEvaluator {
    /// Create a new evaluator.
    #[new]
    #[pyo3(text_signature = "()")]
    fn new() -> Self {
        Self {
            inner: finstack_quant_statements::evaluator::Evaluator::new(),
        }
    }

    /// Attach a check suite to run automatically after each evaluation.
    ///
    /// The suite spec is resolved (built-in and formula checks) and the
    /// resulting report is attached to :attr:`StatementResult.check_report`
    /// on every subsequent ``evaluate`` / ``evaluate_with_market`` call.
    ///
    /// Parameters
    /// ----------
    /// suite_spec : CheckSuiteSpec
    ///     The check-suite specification to resolve and attach.
    ///
    /// Returns
    /// -------
    /// Evaluator
    ///     This evaluator, for chaining.
    #[pyo3(text_signature = "($self, suite_spec)")]
    fn with_checks<'py>(
        mut slf: PyRefMut<'py, Self>,
        suite_spec: PyRef<'_, super::checks::PyCheckSuiteSpec>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let suite = suite_spec.inner.resolve().map_err(statements_to_py)?;
        let evaluator = std::mem::replace(
            &mut slf.inner,
            finstack_quant_statements::evaluator::Evaluator::new(),
        );
        slf.inner = evaluator.with_checks(suite);
        Ok(slf)
    }

    /// Evaluate a financial model.
    ///
    /// Releases the GIL for the duration of the DAG traversal so that other
    /// Python threads can make progress while the Rust evaluator runs.
    ///
    /// Parameters
    /// ----------
    /// model : FinancialModelSpec | str
    ///     The model specification to evaluate — a typed model or its
    ///     canonical JSON.
    ///
    /// Returns
    /// -------
    /// StatementResult
    ///     Evaluation results with per-node, per-period values.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If a formula references a node that does not exist.
    /// ValueError
    ///     If the model JSON is malformed or evaluation fails (cyclic
    ///     dependencies, invalid formulas, forecast parameter errors).
    /// RuntimeError
    ///     If capital-structure processing fails.
    #[pyo3(text_signature = "($self, model)")]
    fn evaluate(
        &mut self,
        py: Python<'_>,
        model: &Bound<'_, PyAny>,
    ) -> PyResult<PyStatementResult> {
        let model = extract_model_ref(model)?;
        let model_inner: &finstack_quant_statements::FinancialModelSpec = &model;
        let evaluator = &mut self.inner;
        let result = py
            .detach(|| evaluator.evaluate(model_inner))
            .map_err(statements_to_py)?;
        Ok(PyStatementResult { inner: result })
    }

    /// Evaluate a financial model with market context and an as-of date.
    ///
    /// Use this for capital-structure-aware models and for as-of evaluation
    /// that hides future actual values. Releases the GIL during evaluation.
    ///
    /// Parameters
    /// ----------
    /// model : FinancialModelSpec | str
    ///     The model specification to evaluate — a typed model or its
    ///     canonical JSON.
    /// market : MarketContext | str
    ///     Market data context used for instrument pricing — typed or its
    ///     canonical JSON.
    /// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Valuation/as-of date; ISO ``YYYY-MM-DD`` strings are accepted.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If a formula references an unknown node or a required curve is
    ///     missing from ``market``.
    /// ValueError
    ///     If ``as_of`` is not date-like, the model/market JSON is
    ///     malformed, or evaluation fails.
    /// RuntimeError
    ///     If capital-structure cashflow generation or the waterfall fails.
    #[pyo3(text_signature = "($self, model, market, as_of)")]
    fn evaluate_with_market(
        &mut self,
        py: Python<'_>,
        model: &Bound<'_, PyAny>,
        market: &Bound<'_, PyAny>,
        as_of: &Bound<'_, PyAny>,
    ) -> PyResult<PyStatementResult> {
        let as_of = extract_date(as_of)?;
        let model = extract_model_ref(model)?;
        let market = extract_market_ref(py, market)?;
        let model_inner: &finstack_quant_statements::FinancialModelSpec = &model;
        let market_inner: &finstack_quant_core::market_data::context::MarketContext = &market;
        let evaluator = &mut self.inner;
        let result = py
            .detach(|| evaluator.evaluate_with_market(model_inner, market_inner, as_of))
            .map_err(statements_to_py)?;
        Ok(PyStatementResult { inner: result })
    }

    /// Run a Monte Carlo simulation over the model's forecast periods.
    ///
    /// Each path re-draws every stochastic forecast (normal, log-normal,
    /// mean-reverting, bootstrap) with a per-path seed derived from the
    /// configured base seed, evaluates the model, and aggregates the
    /// configured percentiles per node and period. Releases the GIL while
    /// paths run.
    ///
    /// Parameters
    /// ----------
    /// model : FinancialModelSpec | str
    ///     A typed model or its canonical JSON. Models with a capital
    ///     structure are rejected.
    /// config : MonteCarloConfig | str
    ///     Typed configuration or JSON with ``n_paths``, ``seed``, optional
    ///     ``percentiles`` (decimal fractions in [0, 1]), and optional
    ///     ``include_path_data``.
    ///
    /// Returns
    /// -------
    /// MonteCarloResults
    ///     Percentile fans per metric, forecast periods, warnings, and the
    ///     optional per-path table.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the model or config JSON is malformed, ``n_paths`` is zero, a
    ///     percentile is outside [0, 1], the model has a capital structure,
    ///     or any path fails to evaluate.
    /// KeyError
    ///     If a formula references an unknown node.
    #[pyo3(text_signature = "($self, model, config)")]
    fn evaluate_monte_carlo(
        &mut self,
        py: Python<'_>,
        model: &Bound<'_, PyAny>,
        config: &Bound<'_, PyAny>,
    ) -> PyResult<PyMonteCarloResults> {
        let model = extract_model_ref(model)?;
        let config = extract_config(config)?;
        let model_inner: &finstack_quant_statements::FinancialModelSpec = &model;
        let evaluator = &mut self.inner;
        let inner = py
            .detach(|| evaluator.evaluate_monte_carlo(model_inner, &config))
            .map_err(statements_to_py)?;
        Ok(PyMonteCarloResults { inner })
    }

    /// Return ``Evaluator()``.
    fn __repr__(&self) -> String {
        "Evaluator()".to_string()
    }
}

/// Register evaluator classes.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyStatementResult>()?;
    m.add_class::<PyEvaluator>()?;
    Ok(())
}
