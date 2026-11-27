//! Python wrappers for the statement evaluator and results.

use super::capital_structure::PyCapitalStructureCashflows;
use super::monte_carlo::{PyMonteCarloConfig, PyMonteCarloResults};
use super::parse_period_id;
use crate::bindings::core::dates::periods::PyPeriod;
use crate::bindings::core::market_data::context::PyMarketContext;
use crate::bindings::core::money::PyMoney;
use crate::bindings::core::table::{PyArrowTable, PyLabeledSeries};
use crate::bindings::date_utils::{date_to_py, py_to_date};
use crate::bindings::macros::wire_methods;
use crate::bindings::statements::types::PyFinancialModelSpec;
use crate::errors::statements_to_py;
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

wire_methods!(
    PyStatementResult,
    finstack_quant_statements::evaluator::StatementResult,
    "StatementResult"
);

#[pymethods]
impl PyStatementResult {
    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements._evaluator", "StatementResult.get")]
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements._evaluator", "StatementResult.get_or")]
    #[pyo3(text_signature = "($self, node_id, period, default)")]
    fn get_or(&self, node_id: &str, period: &str, default: f64) -> PyResult<f64> {
        let pid = parse_period_id(period)?;
        self.inner
            .get_or(node_id, &pid, default)
            .map_err(statements_to_py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements._evaluator", "StatementResult.periods")]
    #[getter]
    fn periods(&self) -> Vec<PyPeriod> {
        self.inner
            .get_periods()
            .iter()
            .cloned()
            .map(PyPeriod::from_inner)
            .collect()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements._evaluator", "StatementResult.all_periods")]
    #[pyo3(text_signature = "($self, node_id)")]
    fn all_periods(&self, node_id: &str) -> Vec<(String, f64)> {
        self.inner
            .all_periods(node_id)
            .map(|(pid, value)| (pid.to_string(), value))
            .collect()
    }

    /// Roll these results up to a coarser frequency, such as quarterly to annual.
    ///
    /// Each target period combines its source periods node by node: summed
    /// by default (flows), the last source value for ``"period_end"`` nodes
    /// (balances, LTM metrics), and the node's formula re-evaluated on the
    /// rolled-up values for ``"formula"`` nodes (ratios). ``lag`` and other
    /// time-series functions then step in target periods. Every target
    /// period must be complete.
    ///
    /// Parameters
    /// ----------
    /// model : FinancialModelSpec
    ///     The model these results were evaluated from. It supplies the
    ///     period dates and the formulas of ``"formula"`` nodes.
    /// spec : RollupSpec
    ///     Target frequency and per-node methods.
    ///
    /// Returns
    /// -------
    /// StatementResult
    ///     Values keyed by the target periods (``"2025"``, ``"2025H1"``,
    ///     ``"2025Q1"``; fiscal periods keep their ``FY`` prefix), with
    ///     monetary values, value types and those periods in ``periods`` (each
    ///     actual only when every source period is). It carries no
    ///     per-cell ``source``, capital-structure cashflows or check report.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the target frequency is not a whole
    ///     multiple of the model's, a target period is incomplete, a method
    ///     names an unknown node, a ``"formula"`` node has no formula or reads
    ///     ``cs.*`` values, or a re-evaluated formula fails.
    ///
    /// Examples
    /// --------
    /// >>> from finstack_quant.statements import Evaluator, ModelBuilder, RollupSpec
    /// >>> b = ModelBuilder("m")
    /// >>> _ = b.periods("2025Q1..Q4", None)
    /// >>> _ = b.value("revenue", [(f"2025Q{q}", 100.0) for q in range(1, 5)])
    /// >>> _ = b.value("cash", [("2025Q1", 10.0), ("2025Q2", 20.0), ("2025Q3", 30.0), ("2025Q4", 40.0)])
    /// >>> _ = b.compute("ebitda", "revenue * 0.25")
    /// >>> _ = b.compute("margin", "ebitda / revenue")
    /// >>> model = b.build()
    /// >>> annual = Evaluator().evaluate(model).rollup(model, RollupSpec("annual", {"cash": "period_end", "margin": "formula"}))
    /// >>> (annual.get("revenue", "2025"), annual.get("cash", "2025"), annual.get("margin", "2025"))
    /// (400.0, 40.0, 0.25)
    #[pyo3(text_signature = "($self, model, spec)")]
    fn rollup(
        &self,
        py: Python<'_>,
        model: PyRef<'_, PyFinancialModelSpec>,
        spec: PyRef<'_, super::rollup::PyRollupSpec>,
    ) -> PyResult<Self> {
        let model = &model.inner;
        let spec = spec.inner.clone();
        let model_inner: &finstack_quant_statements::types::FinancialModelSpec = model;
        let result = &self.inner;
        let inner = py
            .detach(|| result.rollup(model_inner, &spec))
            .map_err(statements_to_py)?;
        Ok(Self { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements._evaluator", "StatementResult.cells")]
    #[getter]
    fn cells<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.cells)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements._evaluator", "StatementResult.get_cell")]
    #[pyo3(text_signature = "($self, node_id, period)")]
    fn get_cell<'py>(
        &self,
        py: Python<'py>,
        node_id: &str,
        period: &str,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let pid = parse_period_id(period)?;
        self.inner
            .get_cell(node_id, &pid)
            .map(|cell| crate::bindings::json_bridge::serde_to_py(py, cell))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements._evaluator", "StatementResult.provenance")]
    #[getter]
    fn provenance<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .provenance
            .as_ref()
            .map(|provenance| crate::bindings::json_bridge::serde_to_py(py, provenance))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements._evaluator", "StatementResult.rollup_provenance")]
    #[getter]
    fn rollup_provenance<'py>(&self, py: Python<'py>) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.inner
            .rollup_provenance
            .as_ref()
            .map(|provenance| crate::bindings::json_bridge::serde_to_py(py, provenance))
            .transpose()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements._evaluator", "StatementResult.normalizations")]
    #[getter]
    fn normalizations<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.normalizations)
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
    /// structure.
    #[getter]
    fn cs_cashflows(&self) -> Option<PyCapitalStructureCashflows> {
        self.inner
            .cs_cashflows
            .clone()
            .map(|inner| PyCapitalStructureCashflows { inner })
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements._evaluator", "StatementResult.numeric_series")]
    #[pyo3(text_signature = "($self, node_id)")]
    fn numeric_series<'py>(
        &self,
        py: Python<'py>,
        node_id: &str,
    ) -> PyResult<Option<Bound<'py, PyDict>>> {
        match self.inner.numeric_series(node_id) {
            Some(period_map) => {
                let dict = PyDict::new(py);
                for (pid, val) in period_map {
                    dict.set_item(pid.to_string(), val)?;
                }
                Ok(Some(dict))
            }
            None => Ok(None),
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements._evaluator", "StatementResult.to_series")]
    #[pyo3(text_signature = "($self, node_id)")]
    fn to_series(&self, node_id: &str) -> PyResult<PyLabeledSeries> {
        let period_map = self
            .inner
            .get_node(node_id)
            .ok_or_else(|| PyKeyError::new_err(format!("unknown node: {node_id:?}")))?;
        let labels: Vec<String> = period_map.keys().map(ToString::to_string).collect();
        let values: Vec<Option<f64>> = period_map.values().map(|cell| cell.numeric()).collect();
        Ok(PyLabeledSeries::from_opt_f64(node_id, labels, values)?.with_index_name("period_id"))
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements._evaluator", "StatementResult.to_dated_schedule")]
    #[pyo3(text_signature = "($self, node_id, convention)")]
    fn to_dated_schedule<'py>(
        &self,
        py: Python<'py>,
        node_id: &str,
        convention: &str,
    ) -> PyResult<Vec<Bound<'py, PyDict>>> {
        let convention = convention
            .parse::<PeriodDateConvention>()
            .map_err(statements_to_py)?;
        finstack_quant_statements::evaluator::node_to_dated_schedule(
            &self.inner,
            node_id,
            convention,
        )
        .map_err(statements_to_py)?
        .into_iter()
        .map(|row| {
            let out = PyDict::new(py);
            out.set_item("date", date_to_py(py, row.date)?)?;
            out.set_item("period_id", row.period_id.to_string())?;
            out.set_item(
                "cell",
                crate::bindings::json_bridge::serde_to_py(py, &row.cell)?,
            )?;
            Ok(out)
        })
        .collect()
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements._evaluator", "StatementResult.to_pricing_flows")]
    #[pyo3(text_signature = "($self, selection)")]
    fn to_pricing_flows<'py>(
        &self,
        py: Python<'py>,
        selection: &Bound<'py, PyAny>,
    ) -> PyResult<Vec<(Bound<'py, PyAny>, PyMoney)>> {
        let selection: finstack_quant_statements::evaluator::PricingFlowSelection =
            crate::bindings::module_utils::spec_from_py(py, selection, "pricing flow selection")?;
        finstack_quant_statements::evaluator::node_to_pricing_flows(&self.inner, &selection)
            .map_err(statements_to_py)?
            .into_iter()
            .map(|(date, inner)| Ok((date_to_py(py, date)?, PyMoney { inner })))
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
        self.inner.cells.keys().cloned().collect()
    }

    /// Number of nodes in the result.
    #[getter]
    fn node_count(&self) -> usize {
        self.inner.cells.len()
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
        crate::bindings::json_bridge::serde_to_py(py, &self.inner.meta.warnings)
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

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements._evaluator", "StatementResult.to_arrow")]
    #[pyo3(signature = (orient = "long"))]
    #[pyo3(text_signature = "($self, orient='long')")]
    fn to_arrow(&self, orient: &str) -> PyResult<PyArrowTable> {
        self.to_arrow_table(orient)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements._evaluator", "StatementResult.to_pandas")]
    #[pyo3(signature = (orient = "long"))]
    #[pyo3(text_signature = "($self, orient='long')")]
    fn to_pandas<'py>(&self, py: Python<'py>, orient: &str) -> PyResult<Bound<'py, PyAny>> {
        let table = Bound::new(py, self.to_arrow_table(orient)?)?;
        let df = PyArrowTable::to_pandas(&table, py)?;
        if orient == "wide" {
            df.call_method1("set_index", ("period_id",))?.getattr("T")
        } else {
            Ok(df)
        }
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements._evaluator", "StatementResult.to_polars")]
    #[pyo3(signature = (orient = "long"))]
    #[pyo3(text_signature = "($self, orient='long')")]
    fn to_polars<'py>(&self, py: Python<'py>, orient: &str) -> PyResult<Bound<'py, PyAny>> {
        let table = Bound::new(py, self.to_arrow_table(orient)?)?;
        PyArrowTable::to_polars(&table, py)
    }

    #[doc = crate::bindings::pydoc::pydoc!("finstack_quant.statements._evaluator", "StatementResult.to_pyarrow")]
    #[pyo3(signature = (orient = "long"))]
    #[pyo3(text_signature = "($self, orient='long')")]
    fn to_pyarrow<'py>(&self, py: Python<'py>, orient: &str) -> PyResult<Bound<'py, PyAny>> {
        let table = Bound::new(py, self.to_arrow_table(orient)?)?;
        PyArrowTable::to_pyarrow(&table, py)
    }

    /// Export via the Arrow PyCapsule C-stream protocol (`arrow_array_stream`).
    #[pyo3(signature = (requested_schema = None))]
    fn __arrow_c_stream__<'py>(
        &self,
        py: Python<'py>,
        requested_schema: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, pyo3::types::PyCapsule>> {
        let _ = requested_schema;
        self.to_arrow_table("long")?.arrow_c_stream(py)
    }

    /// Export the Arrow schema via the Arrow PyCapsule interface (`arrow_schema`).
    fn __arrow_c_schema__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, pyo3::types::PyCapsule>> {
        self.to_arrow_table("long")?.arrow_c_schema(py)
    }

    /// Render as an HTML table preview in Jupyter notebooks.
    fn _repr_html_(&self) -> Option<String> {
        self.to_arrow_table("wide").ok().map(|t| t.render_html())
    }

    /// Return the representation with node and period counts.
    fn __repr__(&self) -> String {
        format!(
            "StatementResult(nodes={}, periods={})",
            self.inner.cells.len(),
            self.inner.meta.num_periods
        )
    }
}

impl PyStatementResult {
    fn to_arrow_table(&self, orient: &str) -> PyResult<PyArrowTable> {
        match orient {
            "long" => {
                let table = self.inner.to_table_long().map_err(statements_to_py)?;
                PyArrowTable::from_envelope(&table)
            }
            "wide" => {
                let table = self.inner.to_table_wide().map_err(statements_to_py)?;
                PyArrowTable::from_envelope(&table)
            }
            other => Err(pyo3::exceptions::PyValueError::new_err(format!(
                "orient must be 'long' or 'wide', got {other:?}"
            ))),
        }
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
    /// Python threads can make progress while the Rust evaluator runs. A
    /// capital structure is evaluated from an empty market as of the first
    /// period's start, so fixed-rate debt yields ``cs_cashflows``;
    /// instruments that need market data need ``evaluate_with_market``.
    ///
    /// Parameters
    /// ----------
    /// model : FinancialModelSpec
    ///     The model specification to evaluate.
    ///
    /// Returns
    /// -------
    /// StatementResult
    ///     Evaluation results with per-node, per-period values and any
    ///     capital-structure cashflows.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If a formula references a node that does not exist, or a
    ///     capital-structure instrument needs market data.
    /// ValueError
    ///     If evaluation fails (cyclic
    ///     dependencies, invalid formulas, forecast parameter errors).
    /// RuntimeError
    ///     If capital-structure processing fails.
    #[pyo3(text_signature = "($self, model)")]
    fn evaluate(
        &mut self,
        py: Python<'_>,
        model: PyRef<'_, PyFinancialModelSpec>,
    ) -> PyResult<PyStatementResult> {
        let model = &model.inner;
        let model_inner: &finstack_quant_statements::types::FinancialModelSpec = model;
        let evaluator = &mut self.inner;
        let result = py
            .detach(|| evaluator.evaluate(model_inner))
            .map_err(statements_to_py)?;
        Ok(PyStatementResult { inner: result })
    }

    /// Evaluate a financial model with market context and an as-of date.
    ///
    /// Use this for capital-structure-aware models and for point-in-time
    /// evaluation. ``as_of`` dates pricing; ``observation_cutoff`` separately
    /// selects which explicit observations the evaluation may read. Releases
    /// the GIL during evaluation.
    ///
    /// Parameters
    /// ----------
    /// model : FinancialModelSpec
    ///     The model specification to evaluate.
    /// market : MarketContext
    ///     Market data context used for instrument pricing.
    /// as_of : datetime.date | datetime.datetime | pandas.Timestamp | str
    ///     Valuation date for pricing the capital structure; ISO
    ///     ``YYYY-MM-DD`` strings are accepted.
    /// observation_cutoff : str | datetime.date | datetime.datetime | pandas.Timestamp
    ///     ``"full_snapshot"`` reads every observation in the model. A date
    ///     (or its ISO string) reads an observation only when its availability
    ///     date is on or before that date, inclusive; the model must then
    ///     name its ``source_snapshot`` and give an availability date to
    ///     every observation in an actual period. An undated explicit value
    ///     in a forecast period is a forecast assumption and stays visible.
    ///
    /// Returns
    /// -------
    /// StatementResult
    ///     Evaluation results with per-node, per-period values and any
    ///     capital-structure cashflows.
    ///
    /// Raises
    /// ------
    /// KeyError
    ///     If a formula references an unknown node or a required curve is
    ///     missing from ``market``.
    /// ValueError
    ///     If ``as_of`` or ``observation_cutoff`` is not date-like, a
    ///     cutoff date is used on a model
    ///     with no ``source_snapshot`` or with an undated observation in an
    ///     actual period, an observation hidden by the cutoff has no forecast
    ///     or formula to fall back on, or evaluation fails.
    /// RuntimeError
    ///     If capital-structure cashflow generation or the waterfall fails.
    #[pyo3(text_signature = "($self, model, market, as_of, observation_cutoff)")]
    fn evaluate_with_market(
        &mut self,
        py: Python<'_>,
        model: PyRef<'_, PyFinancialModelSpec>,
        market: PyRef<'_, PyMarketContext>,
        as_of: &Bound<'_, PyAny>,
        observation_cutoff: &Bound<'_, PyAny>,
    ) -> PyResult<PyStatementResult> {
        use finstack_quant_statements::evaluator::ObservationCutoff;

        let as_of = py_to_date(as_of)?;
        let cutoff = match observation_cutoff.extract::<String>() {
            Ok(mode) if mode == "full_snapshot" => ObservationCutoff::FullSnapshot,
            _ => ObservationCutoff::Through(py_to_date(observation_cutoff)?),
        };
        let model = &model.inner;
        let market = &market.inner;
        let model_inner: &finstack_quant_statements::types::FinancialModelSpec = model;
        let market_inner: &finstack_quant_core::market_data::context::MarketContext = market;
        let evaluator = &mut self.inner;
        let result = py
            .detach(|| evaluator.evaluate_with_market(model_inner, market_inner, as_of, cutoff))
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
    /// model : FinancialModelSpec
    ///     The model.
    /// config : MonteCarloConfig
    ///     Configuration with ``n_paths``, ``seed``,
    ///     ``metrics``, optional ``percentiles`` (decimal fractions in
    ///     [0, 1]) and optional ``include_path_data``.
    /// market, as_of, observation_cutoff
    ///     The market context of ``evaluate_with_market``; omit all three to
    ///     evaluate as ``evaluate`` does.
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
    ///     If the model or config is rejected, only one of ``market`` and
    ///     ``as_of`` is given, or any path fails to evaluate.
    /// KeyError
    ///     If a formula references an unknown node.
    #[pyo3(
        signature = (model, config, market=None, as_of=None, observation_cutoff=None),
        text_signature = "($self, model, config, market=None, as_of=None, observation_cutoff=None)"
    )]
    fn evaluate_monte_carlo(
        &mut self,
        py: Python<'_>,
        model: PyRef<'_, PyFinancialModelSpec>,
        config: PyRef<'_, PyMonteCarloConfig>,
        market: Option<PyRef<'_, PyMarketContext>>,
        as_of: Option<&Bound<'_, PyAny>>,
        observation_cutoff: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyMonteCarloResults> {
        use finstack_quant_statements::evaluator::{MonteCarloContext, ObservationCutoff};

        let model = &model.inner;
        let config = config.inner.clone();
        let model_inner: &finstack_quant_statements::types::FinancialModelSpec = model;
        let run = match (market.as_ref(), as_of) {
            (Some(market), Some(as_of)) => {
                let cutoff = match observation_cutoff {
                    None => ObservationCutoff::FullSnapshot,
                    Some(cutoff) => match cutoff.extract::<String>() {
                        Ok(mode) if mode == "full_snapshot" => ObservationCutoff::FullSnapshot,
                        _ => ObservationCutoff::Through(py_to_date(cutoff)?),
                    },
                };
                Some((&market.inner, py_to_date(as_of)?, cutoff))
            }
            (None, None) if observation_cutoff.is_none() => None,
            _ => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "evaluate_monte_carlo: give `market` and `as_of` together; \
                     `observation_cutoff` needs both",
                ));
            }
        };
        let context = run.as_ref().map(|(market, as_of, cutoff)| {
            let market: &finstack_quant_core::market_data::context::MarketContext = market;
            MonteCarloContext {
                market,
                as_of: *as_of,
                cutoff: *cutoff,
            }
        });
        let evaluator = &mut self.inner;
        let inner = py
            .detach(|| evaluator.evaluate_monte_carlo(model_inner, &config, context))
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
