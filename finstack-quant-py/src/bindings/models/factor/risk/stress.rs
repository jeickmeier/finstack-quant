use super::functions::extract_pnl_input;
use crate::bindings::core::table::PyArrowTable;
use crate::bindings::macros::{tabular_methods, wire_methods};
use crate::bindings::repr_support::repr_from_serde;
use crate::bindings::table_builders::TableBuilder;
use crate::errors::{core_to_py, value_error};
use finstack_quant_factor::risk::{
    self as model_risk, StressAttribution, StressPositionEntry, TailScenarioBreakdown,
};
use pyo3::prelude::*;

/// Single position's contribution to tail stress.
#[pyclass(
    name = "StressPositionEntry",
    module = "finstack_quant.models.factor.risk",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyStressPositionEntry {
    pub(crate) inner: StressPositionEntry,
}

impl PyStressPositionEntry {
    fn from_inner(inner: StressPositionEntry) -> Self {
        Self { inner }
    }
}

wire_methods!(
    PyStressPositionEntry,
    StressPositionEntry,
    "StressPositionEntry"
);

#[pymethods]
impl PyStressPositionEntry {
    /// Portfolio position identifier.
    #[getter]
    fn position_id(&self) -> String {
        self.inner.position_id.as_str().to_owned()
    }

    /// Average P&L contribution across the tail scenarios; a loss is negative.
    #[getter]
    fn avg_tail_pnl(&self) -> f64 {
        self.inner.avg_tail_pnl
    }

    /// Share of total portfolio tail loss attributable to this position, as a
    /// **fraction** (not a percentage) despite the ``pct_`` name.
    #[getter]
    fn pct_of_tail_loss(&self) -> f64 {
        self.inner.pct_of_tail_loss
    }

    /// Worst single-scenario P&L for this position; a loss is negative.
    #[getter]
    fn worst_scenario_pnl(&self) -> f64 {
        self.inner.worst_scenario_pnl
    }

    fn __repr__(&self) -> String {
        repr_from_serde("StressPositionEntry", &self.inner)
    }
}

impl PyStressPositionEntry {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        TableBuilder::new()
            .utf8(
                "position_id",
                vec![self.inner.position_id.as_str().to_owned()],
            )?
            .f64("avg_tail_pnl", vec![self.inner.avg_tail_pnl])?
            .f64("pct_of_tail_loss", vec![self.inner.pct_of_tail_loss])?
            .f64("worst_scenario_pnl", vec![self.inner.worst_scenario_pnl])?
            .build()
    }
}

tabular_methods!(PyStressPositionEntry);

/// Breakdown of a single tail scenario.
#[pyclass(
    name = "TailScenarioBreakdown",
    module = "finstack_quant.models.factor.risk",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyTailScenarioBreakdown {
    pub(crate) inner: TailScenarioBreakdown,
    /// Position ordering inherited from the parent ``StressAttribution``;
    /// ``None`` for a breakdown built from JSON on its own.
    position_ids: Option<Vec<String>>,
}

impl PyTailScenarioBreakdown {
    fn from_inner(inner: TailScenarioBreakdown) -> Self {
        Self {
            inner,
            position_ids: None,
        }
    }

    fn with_position_ids(inner: TailScenarioBreakdown, position_ids: Vec<String>) -> Self {
        Self {
            inner,
            position_ids: Some(position_ids),
        }
    }
}

// Hand-written rather than `wire_methods!`: the wrapper carries the parent's
// `position_ids` beside `inner`, which a breakdown loaded on its own lacks.
#[pymethods]
impl PyTailScenarioBreakdown {
    /// Serialize this TailScenarioBreakdown to its canonical JSON wire form.
    ///
    /// Returns
    /// -------
    /// str
    ///     Compact JSON that ``from_json`` accepts.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the value cannot be serialized.
    #[pyo3(text_signature = "($self)")]
    fn to_json(&self) -> PyResult<String> {
        crate::bindings::json_bridge::serialize_json_with_context(
            &self.inner,
            "failed to serialize TailScenarioBreakdown",
        )
    }

    /// Deserialize a TailScenarioBreakdown from its canonical JSON wire form.
    ///
    /// Parameters
    /// ----------
    /// json : str
    ///     JSON produced by ``to_json`` (strict field names).
    ///
    /// Returns
    /// -------
    /// TailScenarioBreakdown
    ///     The reconstructed value, without the parent's position ordering.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If the JSON is malformed or carries unknown fields.
    #[staticmethod]
    #[pyo3(text_signature = "(json)")]
    fn from_json(json: &str) -> PyResult<Self> {
        crate::bindings::json_bridge::deserialize_json_with_context(
            json,
            "invalid TailScenarioBreakdown JSON",
        )
        .map(Self::from_inner)
    }

    /// Support ``pickle`` through the same strict serde round-trip as
    /// ``to_json`` / ``from_json``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Zero-based index of this scenario in the original P&L history.
    #[getter]
    fn scenario_index(&self) -> usize {
        self.inner.scenario_index
    }

    /// Total portfolio P&L for this scenario; a loss is negative.
    #[getter]
    fn portfolio_pnl(&self) -> f64 {
        self.inner.portfolio_pnl
    }

    /// Per-position P&L for this scenario, index-aligned to
    /// :attr:`StressAttribution.position_ids` (entry ``i`` is the P&L for
    /// ``position_ids[i]``).
    #[getter]
    fn position_pnls(&self) -> Vec<f64> {
        self.inner.position_pnls.clone()
    }

    /// Position identifiers inherited from the parent ``StressAttribution``,
    /// or ``None`` for a breakdown reconstructed from JSON on its own.
    #[getter]
    fn position_ids(&self) -> Option<Vec<String>> {
        self.position_ids.clone()
    }

    /// Export this scenario's per-position P&L as a pandas ``DataFrame``.
    ///
    /// Columns: ``position_id``, ``pnl`` (portfolio-currency amount; a loss is
    /// negative).
    ///
    /// Parameters
    /// ----------
    /// position_ids : list[str], optional
    ///     Position identifiers aligned with :attr:`position_pnls`. Defaults
    ///     to the ordering inherited from the parent
    ///     :attr:`StressAttribution.position_ids` when this breakdown was read
    ///     from ``tail_scenarios``; required for a breakdown built via
    ///     :meth:`from_json`.
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If no identifiers are available or their count does not match
    ///     ``len(position_pnls)``.
    #[pyo3(signature = (position_ids = None), text_signature = "($self, position_ids=None)")]
    fn to_arrow(&self, position_ids: Option<Vec<String>>) -> PyResult<PyArrowTable> {
        self.to_arrow_table_with_ids(position_ids)
    }

    #[pyo3(signature = (position_ids = None), text_signature = "($self, position_ids=None)")]
    fn to_pandas<'py>(
        &self,
        py: Python<'py>,
        position_ids: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let table = Bound::new(py, self.to_arrow_table_with_ids(position_ids)?)?;
        PyArrowTable::to_pandas(&table, py)
    }

    #[pyo3(signature = (position_ids = None), text_signature = "($self, position_ids=None)")]
    fn to_polars<'py>(
        &self,
        py: Python<'py>,
        position_ids: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let table = Bound::new(py, self.to_arrow_table_with_ids(position_ids)?)?;
        PyArrowTable::to_polars(&table, py)
    }

    #[pyo3(signature = (position_ids = None), text_signature = "($self, position_ids=None)")]
    fn to_pyarrow<'py>(
        &self,
        py: Python<'py>,
        position_ids: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let table = Bound::new(py, self.to_arrow_table_with_ids(position_ids)?)?;
        PyArrowTable::to_pyarrow(&table, py)
    }

    #[pyo3(signature = (requested_schema = None))]
    fn __arrow_c_stream__<'py>(
        &self,
        py: Python<'py>,
        requested_schema: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, pyo3::types::PyCapsule>> {
        let _ = requested_schema;
        self.to_arrow_table_with_ids(None)?.arrow_c_stream(py)
    }

    fn __arrow_c_schema__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, pyo3::types::PyCapsule>> {
        self.to_arrow_table_with_ids(None)?.arrow_c_schema(py)
    }

    fn _repr_html_(&self) -> Option<String> {
        self.to_arrow_table_with_ids(None)
            .ok()
            .map(|t| t.render_html())
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

impl PyTailScenarioBreakdown {
    fn to_arrow_table_with_ids(&self, position_ids: Option<Vec<String>>) -> PyResult<PyArrowTable> {
        let Some(position_ids) = position_ids.or_else(|| self.position_ids.clone()) else {
            return Err(value_error(
                "position_ids is required: this TailScenarioBreakdown carries no parent \
                 StressAttribution ordering",
            ));
        };
        let expected = self.inner.position_pnls.len();
        if position_ids.len() != expected {
            return Err(value_error(format!(
                "position_ids length ({}) does not match position_pnls length ({expected})",
                position_ids.len(),
            )));
        }
        TableBuilder::new()
            .utf8("position_id", position_ids)?
            .f64("pnl", self.inner.position_pnls.clone())?
            .build()
    }
}

/// Per-position attribution of portfolio losses in tail scenarios.
#[pyclass(
    name = "StressAttribution",
    module = "finstack_quant.models.factor.risk",
    frozen,
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyStressAttribution {
    pub(crate) inner: StressAttribution,
}

impl PyStressAttribution {
    pub(super) fn from_inner(inner: StressAttribution) -> Self {
        Self { inner }
    }
}

wire_methods!(PyStressAttribution, StressAttribution, "StressAttribution");

#[pymethods]
impl PyStressAttribution {
    /// Portfolio VaR threshold separating tail scenarios from the rest.
    ///
    /// Returns
    /// -------
    /// float
    ///     Loss convention: reported as a **negative** number. Scenarios whose
    ///     portfolio P&L is at or below this value are tail events.
    #[getter]
    fn var_threshold(&self) -> f64 {
        self.inner.var_threshold
    }

    /// Number of scenarios classified as tail events.
    #[getter]
    fn n_tail_scenarios(&self) -> usize {
        self.inner.n_tail_scenarios
    }

    /// Canonical position ordering shared by every entry of
    /// :attr:`tail_scenarios`. ``tail_scenarios[k].position_pnls[i]`` is the
    /// P&L for ``position_ids[i]``.
    #[getter]
    fn position_ids(&self) -> Vec<String> {
        self.inner
            .position_ids
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect()
    }

    /// Per-position average tail-loss contributions, sorted by absolute
    /// contribution (largest risk driver first).
    #[getter]
    fn position_contributions(&self) -> Vec<PyStressPositionEntry> {
        self.inner
            .position_contributions
            .iter()
            .cloned()
            .map(PyStressPositionEntry::from_inner)
            .collect()
    }

    /// Per-scenario breakdowns for every tail event. Each breakdown carries
    /// this attribution's ``position_ids`` so its ``to_pandas()`` / ``to_arrow()``
    /// needs no argument.
    #[getter]
    fn tail_scenarios(&self) -> Vec<PyTailScenarioBreakdown> {
        let ids = self.position_ids();
        self.inner
            .tail_scenarios
            .iter()
            .cloned()
            .map(|scenario| PyTailScenarioBreakdown::with_position_ids(scenario, ids.clone()))
            .collect()
    }

    /// Export the tail scenario × position P&L matrix as an ``ArrowTable``.
    ///
    /// Column 0 is ``scenario_index`` (:attr:`TailScenarioBreakdown.scenario_index`);
    /// remaining columns are the position identifiers from :attr:`position_ids`,
    /// in that order. Every P&L cell is in portfolio currency (a loss is negative).
    ///
    /// Raises
    /// ------
    /// ValueError
    ///     If any tail scenario's ``position_pnls`` width disagrees with
    ///     ``len(position_ids)`` (only reachable through a hand-built
    ///     :meth:`from_json` payload).
    #[pyo3(text_signature = "($self)")]
    fn to_scenario_table(&self) -> PyResult<PyArrowTable> {
        let n_positions = self.inner.position_ids.len();
        for scenario in &self.inner.tail_scenarios {
            if scenario.position_pnls.len() != n_positions {
                return Err(value_error(format!(
                    "tail scenario {} has {} position P&Ls but {n_positions} position ids",
                    scenario.scenario_index,
                    scenario.position_pnls.len(),
                )));
            }
        }
        let scenario_indices: Vec<i64> = self
            .inner
            .tail_scenarios
            .iter()
            .map(|scenario| {
                i64::try_from(scenario.scenario_index)
                    .map_err(|e| value_error(format!("scenario_index overflow: {e}")))
            })
            .collect::<PyResult<_>>()?;
        let mut builder = TableBuilder::new().i64("scenario_index", scenario_indices)?;
        for (i, position_id) in self.inner.position_ids.iter().enumerate() {
            let column: Vec<f64> = self
                .inner
                .tail_scenarios
                .iter()
                .map(|scenario| scenario.position_pnls[i])
                .collect();
            builder = builder.f64(position_id.as_str(), column)?;
        }
        builder.build()
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

impl PyStressAttribution {
    fn to_arrow_table(&self) -> PyResult<PyArrowTable> {
        let rows = &self.inner.position_contributions;
        let position_ids: Vec<String> = rows
            .iter()
            .map(|e| e.position_id.as_str().to_owned())
            .collect();
        let avg_tail_pnl: Vec<f64> = rows.iter().map(|e| e.avg_tail_pnl).collect();
        let pct_of_tail_loss: Vec<f64> = rows.iter().map(|e| e.pct_of_tail_loss).collect();
        let worst_scenario_pnl: Vec<f64> = rows.iter().map(|e| e.worst_scenario_pnl).collect();
        TableBuilder::new()
            .utf8("position_id", position_ids)?
            .f64("avg_tail_pnl", avg_tail_pnl)?
            .f64("pct_of_tail_loss", pct_of_tail_loss)?
            .f64("worst_scenario_pnl", worst_scenario_pnl)?
            .build()
    }
}

tabular_methods!(PyStressAttribution);

/// Build tail-scenario stress attribution from position x scenario P&Ls.
///
/// Args:
///     position_ids: Position identifiers; may be ``None`` when
///         ``position_pnls`` is a ``pandas.DataFrame`` (column labels are
///         used).
///     position_pnls: P&L matrix. A ``pandas.DataFrame`` is read as rows =
///         scenarios, columns = positions. A nested list or 2-D NumPy array is
///         read as ``n_positions x n_scenarios`` (position-major, one row per
///         position). Losses are negative.
///     confidence: Tail confidence as a decimal probability strictly inside
///         ``(0.5, 1)``; scenarios at or below the VaR quantile are tail
///         events. ``None`` uses the default ``0.95``.
///
/// Returns:
///     ``StressAttribution`` with per-position tail-loss contributions and
///     per-scenario breakdowns.
///
/// Raises:
///     ValueError: If the matrix is empty, ragged, does not have one row per
///         position id, or ``confidence`` is outside ``(0.5, 1)``.
#[pyfunction]
#[pyo3(signature = (position_ids, position_pnls, confidence = None))]
pub(super) fn build_stress_attribution(
    py: Python<'_>,
    position_ids: Option<Vec<String>>,
    position_pnls: &Bound<'_, PyAny>,
    confidence: Option<f64>,
) -> PyResult<PyStressAttribution> {
    let (position_ids, position_pnls) = extract_pnl_input(py, position_ids, position_pnls)?;
    let n_positions = position_ids.len();
    let n_scenarios = position_pnls.n_scenarios();
    let result = py
        .detach(move || {
            let flat = position_pnls.into_scenario_major(n_positions);
            model_risk::build_stress_attribution(&position_ids, &flat, n_scenarios, confidence)
        })
        .map_err(core_to_py)?;
    Ok(PyStressAttribution::from_inner(result))
}
