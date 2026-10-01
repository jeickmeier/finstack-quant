use std::collections::HashMap;

use indexmap::IndexSet;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyType};

use finstack_quant_portfolio::optimization::{
    CandidatePosition, OptimizationStatus, PortfolioOptimizationResult,
    PortfolioOptimizationResultWire, PortfolioOptimizationSpec, TradeType, TradeUniverse,
};
use finstack_quant_portfolio::types::PositionId;

use crate::bindings::pandas_utils::{
    dict_to_dataframe, serde_rows_to_dataframe_with_schema, ColumnSchema,
};

use super::super::json_bridge::{deserialize_json, serialize_json};
use super::enums::{PyMissingMetricPolicy, PyWeightingScheme};
use super::expressions::{PyConstraint, PyObjective, PyPositionFilter};
use super::status_trade::{PyOptimizationStatus, PyTradeSpec};

/// Candidate instrument that could be added to the portfolio by the
/// optimizer (starts at weight zero; bounded by ``min_weight`` /
/// ``max_weight``).
#[pyclass(
    name = "CandidatePosition",
    module = "finstack_quant.portfolio",
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyCandidatePosition {
    pub(crate) inner: CandidatePosition,
}

impl PyCandidatePosition {
    pub(crate) fn from_inner(inner: CandidatePosition) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyCandidatePosition {
    /// Create a candidate.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     Identifier that becomes the position id if the optimizer trades it.
    ///     Must be unique among candidates and absent from existing positions;
    ///     optimization raises ``PortfolioError`` for a collision.
    /// entity_id : str
    ///     Owning entity for the candidate.
    /// instrument : Bond | InterestRateSwap | ... | str
    ///     Typed instrument wrapper or canonical instrument-envelope JSON.
    /// unit : str | dict | None
    ///     Position unit (``"units"`` default, ``"face_value"``,
    ///     ``"percentage"``, ``"notional"`` or ``{"notional": "USD"}``).
    /// max_weight : float
    ///     Maximum weight the candidate may receive (default ``1.0``).
    /// min_weight : float
    ///     Minimum weight when included (default ``0.0`` lets the optimizer
    ///     skip the candidate).
    /// attributes : dict[str, str | float] | None
    ///     Attributes used by filters and exposure constraints.
    #[new]
    #[pyo3(
        signature = (id, entity_id, instrument, unit=None, max_weight=1.0, min_weight=0.0, attributes=None),
        text_signature = "(id, entity_id, instrument, unit=None, max_weight=1.0, min_weight=0.0, attributes=None)"
    )]
    // Arity is fixed by the documented Python keyword signature; grouping into a
    // params struct would change the public API.
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        id: &str,
        entity_id: &str,
        instrument: &Bound<'_, PyAny>,
        unit: Option<&Bound<'_, PyAny>>,
        max_weight: f64,
        min_weight: f64,
        attributes: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let envelope_json = crate::bindings::extract::extract_instrument_json(instrument)?;
        let envelope: finstack_quant_valuations::instruments::InstrumentEnvelope =
            serde_json::from_str(&envelope_json).map_err(crate::errors::display_to_py)?;
        let boxed = envelope
            .into_boxed()
            .map_err(crate::errors::display_to_py)?;
        let unit: finstack_quant_portfolio::position::PositionUnit =
            match unit {
                None => finstack_quant_portfolio::position::PositionUnit::Units,
                Some(obj) => match obj.extract::<String>() {
                    Ok(ref s) if s == "notional" => {
                        finstack_quant_portfolio::position::PositionUnit::Notional(None)
                    }
                    Ok(s) => serde_json::from_value(serde_json::Value::String(s.clone())).map_err(
                        |_| crate::errors::value_error(format!("unknown position unit {s:?}")),
                    )?,
                    Err(_) => crate::bindings::module_utils::py_to_serde(py, obj, "position unit")?,
                },
            };
        let mut inner = CandidatePosition::new(id, entity_id, std::sync::Arc::from(boxed), unit)
            .with_max_weight(max_weight)
            .with_min_weight(min_weight);
        if let Some(attributes) = attributes {
            inner.attributes =
                crate::bindings::module_utils::py_to_serde(py, attributes.as_any(), "attributes")?;
        }
        Ok(Self::from_inner(inner))
    }

    /// Support `pickle` via the same serde round-trip as ``to_json``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Parse from JSON (instrument carried as its canonical tagged payload).
    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: CandidatePosition = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

    /// Candidate identifier, unique among candidates and absent from existing
    /// positions; becomes the position id when traded.
    #[getter]
    fn id(&self) -> String {
        self.inner.id.as_str().to_owned()
    }

    /// Owning entity identifier.
    #[getter]
    fn entity_id(&self) -> String {
        self.inner.entity_id.as_str().to_owned()
    }

    /// Maximum admissible weight (fraction).
    #[getter]
    fn max_weight(&self) -> f64 {
        self.inner.max_weight
    }

    /// Minimum admissible weight when included (fraction).
    #[getter]
    fn min_weight(&self) -> f64 {
        self.inner.min_weight
    }

    /// Instrument id, taken from the underlying ``Instrument::id()``.
    #[getter]
    fn instrument_id(&self) -> String {
        self.inner.instrument.id().to_owned()
    }

    /// Candidate attributes as a ``dict[str, str | float]``.
    #[getter]
    fn attributes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        crate::bindings::pandas_utils::serde_to_py(py, &self.inner.attributes)
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
/// Build with :meth:`all_positions` or :meth:`filtered`, then chain
/// :meth:`with_candidate` / :meth:`allow_shorting_candidates` and attach the
/// universe to a :class:`PortfolioOptimizationSpec` via
/// ``with_trade_universe``.
#[pyclass(
    name = "TradeUniverse",
    module = "finstack_quant.portfolio",
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyTradeUniverse {
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

    /// Universe where only positions matching ``filter`` may trade.
    #[classmethod]
    #[pyo3(text_signature = "(cls, filter)")]
    fn filtered(_cls: &Bound<'_, PyType>, filter: PyPositionFilter) -> Self {
        Self::from_inner(TradeUniverse::filtered(filter.inner))
    }

    /// Return a copy with ``candidate`` appended.
    #[pyo3(text_signature = "(self, candidate)")]
    fn with_candidate(&self, candidate: PyCandidatePosition) -> Self {
        Self::from_inner(self.inner.clone().with_candidate(candidate.inner))
    }

    /// Return a copy that allows candidates to receive negative weights.
    #[pyo3(text_signature = "(self)")]
    fn allow_shorting_candidates(&self) -> Self {
        Self::from_inner(self.inner.clone().allow_shorting_candidates())
    }

    /// Support `pickle` via the same serde round-trip as ``to_json``.
    fn __reduce__<'py>(&self, py: Python<'py>) -> PyResult<(Bound<'py, PyAny>, (String,))> {
        let from_json = py.get_type::<Self>().getattr("from_json")?;
        crate::bindings::pickle_support::reduce_via_json(from_json, self.to_json()?)
    }

    /// Parse from JSON.
    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: TradeUniverse = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

    /// Filter selecting the existing positions the optimizer may resize.
    #[getter]
    fn tradeable_filter(&self) -> PyPositionFilter {
        PyPositionFilter::from_inner(self.inner.tradeable_filter.clone())
    }

    /// Filter selecting positions whose exact quantities and weights remain
    /// fixed under every weighting scheme, including zero-PV holdings, if any.
    #[getter]
    fn held_filter(&self) -> Option<PyPositionFilter> {
        self.inner
            .held_filter
            .clone()
            .map(PyPositionFilter::from_inner)
    }

    /// Candidate instruments not currently held.
    #[getter]
    fn candidates(&self) -> Vec<PyCandidatePosition> {
        self.inner
            .candidates
            .iter()
            .cloned()
            .map(PyCandidatePosition::from_inner)
            .collect()
    }

    /// Whether candidates may take negative (short) weights.
    #[getter]
    fn allow_short_candidates(&self) -> bool {
        self.inner.allow_short_candidates
    }

    fn __repr__(&self) -> String {
        format!(
            "TradeUniverse(candidates={}, allow_short_candidates={})",
            self.inner.candidates.len(),
            if self.inner.allow_short_candidates {
                "True"
            } else {
                "False"
            },
        )
    }
}

/// JSON-serializable portfolio optimization specification, mirroring the
/// Rust builder pattern.
///
/// The portfolio body is held as a ``PortfolioSpec`` JSON payload so this
/// wrapper does not depend on the larger ``PortfolioSpec`` binding (which
/// remains JSON-first elsewhere in the portfolio bindings).
#[pyclass(
    name = "PortfolioOptimizationSpec",
    module = "finstack_quant.portfolio",
    from_py_object
)]
#[derive(Clone)]
pub(super) struct PyPortfolioOptimizationSpec {
    pub(crate) inner: PortfolioOptimizationSpec,
}

impl PyPortfolioOptimizationSpec {
    pub(crate) fn from_inner(inner: PortfolioOptimizationSpec) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyPortfolioOptimizationSpec {
    /// Build a spec from a portfolio and an objective. Constraints,
    /// weighting, policy and trade universe start at the Rust defaults.
    ///
    /// Parameters
    /// ----------
    /// portfolio : Portfolio | str
    ///     Built :class:`Portfolio` (its canonical spec is captured) or a
    ///     ``PortfolioSpec`` JSON string.
    /// objective : Objective
    ///     Optimization objective.
    #[classmethod]
    #[pyo3(text_signature = "(cls, portfolio, objective)")]
    fn new(
        _cls: &Bound<'_, PyType>,
        portfolio: &Bound<'_, PyAny>,
        objective: PyObjective,
    ) -> PyResult<Self> {
        let portfolio: finstack_quant_portfolio::portfolio::PortfolioSpec =
            if let Ok(built) = portfolio.cast::<crate::bindings::portfolio::types::PyPortfolio>() {
                built.borrow().inner.to_spec()
            } else {
                let json: String = portfolio.extract().map_err(|_| {
                    pyo3::exceptions::PyTypeError::new_err(
                        "expected a Portfolio or a canonical PortfolioSpec JSON string",
                    )
                })?;
                deserialize_json(&json)?
            };
        Ok(Self::from_inner(PortfolioOptimizationSpec::new(
            portfolio,
            objective.inner,
        )))
    }

    /// Replace the trade universe (tradeable/held filters and candidates).
    #[pyo3(text_signature = "(self, universe)")]
    fn with_trade_universe(&self, universe: PyTradeUniverse) -> Self {
        Self::from_inner(self.inner.clone().with_trade_universe(universe.inner))
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
        let inner: PortfolioOptimizationSpec = deserialize_json(json_str)?;
        Ok(Self::from_inner(inner))
    }

    /// Serialize to JSON.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
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

    /// Trade universe restricting the optimizer, or ``None`` for the default
    /// (every position tradeable, no candidates).
    #[getter]
    fn trade_universe(&self) -> Option<PyTradeUniverse> {
        self.inner
            .trade_universe
            .clone()
            .map(PyTradeUniverse::from_inner)
    }

    /// Portfolio specification body (raw JSON).
    #[pyo3(text_signature = "(self)")]
    fn portfolio_spec_json(&self) -> PyResult<String> {
        serialize_json(&self.inner.portfolio)
    }

    fn __repr__(&self) -> String {
        format!(
            "PortfolioOptimizationSpec(constraints={}, label={:?})",
            self.inner.constraints.len(),
            self.inner.label,
        )
    }
}

/// Column schema for the trade-list DataFrame, in `TradeSpec` field order.
///
/// Pinned so a solution with no trades still yields a frame carrying every
/// documented column instead of a schema-less empty frame.
const TRADE_COLUMNS: [ColumnSchema<'static>; 9] = [
    ("position_id", "str"),
    ("instrument_id", "str"),
    ("trade_type", "str"),
    ("current_quantity", "float64"),
    ("target_quantity", "float64"),
    ("delta_quantity", "float64"),
    ("direction", "str"),
    ("current_weight", "float64"),
    ("target_weight", "float64"),
];

/// Result of an optimization run.
///
/// The wrapper holds the canonical `PortfolioOptimizationResultWire` rather
/// than `PortfolioOptimizationResult` itself: the latter echoes the original
/// problem, whose `Arc<dyn Instrument>` values do not round-trip through
/// serde. The wire type carries every field this class exposes, so storing it
/// is what lets the wrapper satisfy the result-return contract — typed
/// getters, `to_json`, `from_json`, and pickle — like its 154 siblings.
#[pyclass(
    name = "PortfolioOptimizationResult",
    module = "finstack_quant.portfolio"
)]
pub(super) struct PyPortfolioOptimizationResult {
    pub(crate) inner: PortfolioOptimizationResultWire,
    /// Rebalanced portfolio built eagerly from the live problem (which does
    /// not survive the wire round-trip), or the reason it is unavailable.
    rebalanced: Result<std::sync::Arc<finstack_quant_portfolio::Portfolio>, String>,
}

impl PyPortfolioOptimizationResult {
    pub(crate) fn from_inner(inner: PortfolioOptimizationResult) -> Self {
        let rebalanced = inner
            .to_rebalanced_portfolio()
            .map(std::sync::Arc::new)
            .map_err(|e| e.to_string());
        Self {
            inner: PortfolioOptimizationResultWire::from(&inner),
            rebalanced,
        }
    }
}

#[pymethods]
impl PyPortfolioOptimizationResult {
    /// Serialize to the canonical JSON wire format.
    #[pyo3(text_signature = "(self)")]
    fn to_json(&self) -> PyResult<String> {
        serialize_json(&self.inner)
    }

    /// Rebuild from :meth:`to_json` output.
    ///
    /// Parameters
    /// ----------
    /// json_str : str
    ///     Canonical JSON produced by :meth:`to_json`.
    ///
    /// Returns
    /// -------
    /// PortfolioOptimizationResult
    ///     The reconstructed result, field for field.
    #[staticmethod]
    #[pyo3(text_signature = "(json_str)")]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner = serde_json::from_str(json_str)
            .map_err(|e| crate::errors::serde_json_to_py(e, "invalid optimization result JSON"))?;
        Ok(Self {
            inner,
            rebalanced: Err(
                "rebalanced portfolio is only available on a result returned by \
                 optimize_portfolio, not on one rebuilt from JSON"
                    .to_owned(),
            ),
        })
    }

    /// Rebuild the portfolio with the implied post-trade quantities.
    ///
    /// Existing positions take their ``implied_quantities`` entry (positions
    /// outside the trade universe keep their quantity) and traded candidates
    /// become new positions.
    ///
    /// Raises
    /// ------
    /// RuntimeError
    ///     If the solution is infeasible, or this result was rebuilt from
    ///     JSON / unpickled (the live problem is not part of the wire form).
    #[pyo3(text_signature = "(self)")]
    fn to_rebalanced_portfolio(&self) -> PyResult<crate::bindings::portfolio::types::PyPortfolio> {
        match &self.rebalanced {
            Ok(portfolio) => Ok(crate::bindings::portfolio::types::PyPortfolio {
                inner: std::sync::Arc::clone(portfolio),
            }),
            Err(message) => Err(pyo3::exceptions::PyRuntimeError::new_err(message.clone())),
        }
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

    /// Solver outcome (optimal, feasible-but-suboptimal, infeasible, ...).
    #[getter]
    fn status(&self) -> PyOptimizationStatus {
        PyOptimizationStatus::from_inner(self.inner.status.clone())
    }

    /// Whether :attr:`status` represents a solution that may be consumed.
    #[getter]
    fn is_feasible(&self) -> bool {
        self.inner.is_feasible
    }

    /// Value of the objective function at the solution, in the objective's own
    /// units (which depend on the configured ``Objective``).
    #[getter]
    fn objective_value(&self) -> f64 {
        self.inner.objective_value
    }

    /// Pre-trade weights by position id.
    ///
    /// Returns
    /// -------
    /// dict[str, float]
    ///     Weights are **fractions** of the portfolio, not percentages.
    #[getter]
    fn current_weights(&self) -> HashMap<String, f64> {
        self.inner
            .current_weights
            .iter()
            .map(|(k, v)| (k.as_str().to_owned(), *v))
            .collect()
    }

    /// Post-trade target weights by position id.
    ///
    /// Returns
    /// -------
    /// dict[str, float]
    ///     Fractions, not percentages. Only covers positions in the trade
    ///     universe; positions outside it implicitly keep their current weight.
    #[getter]
    fn optimal_weights(&self) -> HashMap<String, f64> {
        self.inner
            .optimal_weights
            .iter()
            .map(|(k, v)| (k.as_str().to_owned(), *v))
            .collect()
    }

    /// Weight changes ``optimal - current`` by position id, as fractions.
    #[getter]
    fn weight_deltas(&self) -> HashMap<String, f64> {
        self.inner
            .weight_deltas
            .iter()
            .map(|(k, v)| (k.as_str().to_owned(), *v))
            .collect()
    }

    /// Implied target quantities by position id.
    ///
    /// Returns
    /// -------
    /// dict[str, float]
    ///     Quantities in each position's unit convention, including percentage
    ///     points for ``percentage``. Held existing positions retain their exact
    ///     current quantity under every weighting scheme, including zero-PV
    ///     holdings.
    #[getter]
    fn implied_quantities(&self) -> HashMap<String, f64> {
        self.inner
            .implied_quantities
            .iter()
            .map(|(k, v)| (k.as_str().to_owned(), *v))
            .collect()
    }

    /// Evaluated portfolio-level metric values at the solution, keyed by metric id.
    #[getter]
    fn metric_values(&self) -> HashMap<String, f64> {
        self.inner
            .metric_values
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    /// Constraint slack by constraint label.
    ///
    /// Returns
    /// -------
    /// dict[str, float]
    ///     Positive means slack remains; approximately zero means the
    ///     constraint is binding.
    #[getter]
    fn constraint_slacks(&self) -> HashMap<String, f64> {
        self.inner
            .constraint_slacks
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    /// Total turnover (sum of absolute weight changes).
    #[getter]
    fn turnover(&self) -> f64 {
        self.inner.turnover
    }

    /// Trade list sorted by absolute quantity delta (largest first).
    #[pyo3(text_signature = "(self)")]
    fn to_trade_list(&self) -> Vec<PyTradeSpec> {
        self.inner
            .trades
            .iter()
            .cloned()
            .map(PyTradeSpec::from_inner)
            .collect()
    }

    /// Subset of :meth:`to_trade_list` whose ``trade_type`` is ``NewPosition``.
    #[pyo3(text_signature = "(self)")]
    fn new_position_trades(&self) -> Vec<PyTradeSpec> {
        self.inner
            .trades
            .iter()
            .filter(|t| t.trade_type == TradeType::NewPosition)
            .cloned()
            .map(PyTradeSpec::from_inner)
            .collect()
    }

    /// Export the per-position weight and quantity solution as a pandas
    /// ``DataFrame`` indexed by position id.
    ///
    /// ``current_weights``, ``optimal_weights``, ``weight_deltas`` and
    /// ``implied_quantities`` share one position key space, so they are joined
    /// into a single frame. The row axis is the union of their keys, ordered by
    /// first appearance (``current_weights`` first); a value missing from one
    /// map becomes ``None`` in that column — this is how candidate positions
    /// with no current weight show up.
    ///
    /// Columns: ``current_weight``, ``optimal_weight``, ``weight_delta`` (all
    /// fractions of the portfolio, not percentages), ``implied_quantity``
    /// (units / face / notional per the weighting scheme). The index holds the
    /// position ids.
    #[pyo3(text_signature = "(self)")]
    fn to_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let mut positions: IndexSet<&PositionId> = IndexSet::new();
        for map in [
            &self.inner.current_weights,
            &self.inner.optimal_weights,
            &self.inner.weight_deltas,
            &self.inner.implied_quantities,
        ] {
            positions.extend(map.keys());
        }
        let column = |source: &indexmap::IndexMap<PositionId, f64>| -> Vec<Option<f64>> {
            positions
                .iter()
                .map(|id| source.get(*id).copied())
                .collect()
        };
        let data = PyDict::new(py);
        data.set_item("current_weight", column(&self.inner.current_weights))?;
        data.set_item("optimal_weight", column(&self.inner.optimal_weights))?;
        data.set_item("weight_delta", column(&self.inner.weight_deltas))?;
        data.set_item("implied_quantity", column(&self.inner.implied_quantities))?;
        let index: Vec<&str> = positions.iter().map(|id| id.as_str()).collect();
        let index = PyList::new(py, index)?;
        dict_to_dataframe(py, &data, Some(index.into_any()))
    }

    /// Export :meth:`to_trade_list` as a pandas ``DataFrame``.
    ///
    /// One row per trade, in the same order as :meth:`to_trade_list` (sorted by
    /// absolute quantity delta, largest first). A solution that requires no
    /// trades yields a zero-row frame that still carries the column schema.
    ///
    /// Columns: ``position_id``, ``instrument_id``, ``trade_type``
    /// (``"existing"``, ``"new_position"``, ``"close_out"``),
    /// ``current_quantity``, ``target_quantity``, ``delta_quantity``,
    /// ``direction`` (``"buy"``, ``"sell"``, ``"hold"``), ``current_weight``,
    /// ``target_weight`` (weights are fractions, not percentages).
    #[pyo3(text_signature = "(self)")]
    fn to_trade_dataframe<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let trades = self.inner.trades.clone();
        serde_rows_to_dataframe_with_schema(py, &trades, &TRADE_COLUMNS)
    }

    /// Binding constraint labels and their slack values.
    #[pyo3(text_signature = "(self)")]
    fn binding_constraints(&self) -> Vec<(String, f64)> {
        self.inner
            .binding_constraints
            .iter()
            .map(|name| {
                let slack = self
                    .inner
                    .constraint_slacks
                    .get(name)
                    .copied()
                    .unwrap_or(0.0);
                (name.clone(), slack)
            })
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
            self.inner.turnover,
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
