//! Results types for statement evaluation.

use crate::evaluator::cell::UnavailableReason;
use crate::types::{AmountOrScalar, NodeValueType};
use finstack_quant_core::dates::{Date, PeriodId, PeriodPlan};
use finstack_quant_core::money::Money;
use finstack_quant_core::wire::SchemaVersion;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::types::NodeId;

/// Results from evaluating a financial model.
///
/// [`cells`](Self::cells) is the authoritative typed result: one
/// [`ResultCell`] per node and period, in declaration and timeline order. A
/// cell is a finite scalar, an exact currency-tagged [`Money`], or a cell
/// without a value that says whether it is missing, masked or unavailable and
/// why. A zero is a value; a cell without a value is never a zero.
///
/// # Example
///
/// ```rust
/// # use finstack_quant_statements::builder::ModelBuilder;
/// # use finstack_quant_statements::evaluator::Evaluator;
/// # use finstack_quant_core::dates::PeriodId;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let model = {
///     let mut model_builder = ModelBuilder::new("demo");
///     model_builder.periods("2025Q1..Q2", None)?;
///     model_builder.value("revenue", &[
///         (PeriodId::quarter(2025, 1).expect("valid period fixture"), 100_000.0.into()),
///         (PeriodId::quarter(2025, 2).expect("valid period fixture"), 105_000.0.into()),
///     ]);
///     model_builder.compute("gross_profit", "revenue * 0.6")?;
///     model_builder.build()?
/// };
///
/// let mut evaluator = Evaluator::new();
/// let result = evaluator.evaluate(&model)?;
/// assert!(result.get("gross_profit", &PeriodId::quarter(2025, 1).expect("valid period fixture")).is_some());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StatementResult {
    /// Required wire-format schema version. Only numeric `1` is accepted.
    pub schema_version: SchemaVersion,

    /// The period plan `cells` is keyed by: the periods in timeline order,
    /// with their dates and actual/forecast split, and their calendar context.
    ///
    /// [`Evaluator`](crate::evaluator::Evaluator) records the model's plan and
    /// [`rollup`](Self::rollup) records its target periods in the model's
    /// calendar, where a target is actual only when every source period is.
    /// `None` only for hand-built results, such as [`new`](Self::new).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub periods: Option<PeriodPlan>,

    /// Map of node_id → (period_id → [`ResultCell`]): the typed result.
    ///
    /// Every evaluated node has a cell for every period of the plan.
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "IndexMap<String, IndexMap<String, ResultCell>>")
    )]
    pub cells: IndexMap<String, IndexMap<PeriodId, ResultCell>>,

    /// Units of each node: scalar, or monetary in one currency.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub node_value_types: IndexMap<String, NodeValueType>,

    /// What was evaluated: the model, its source snapshot, the observation
    /// cutoff and the run. `None` only for hand-built results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<RunProvenance>,

    /// Capital structure cashflows (populated when model has a capital_structure)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cs_cashflows: Option<crate::capital_structure::CapitalStructureCashflows>,

    /// Check report from inline validation (None if no checks configured)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_report: Option<crate::checks::CheckReport>,

    /// Metadata about the evaluation
    pub meta: EvalStats,
}

/// Which value source produced a statement cell (one node in one period).
///
/// The evaluator resolves every unmasked cell by the precedence
/// `Value > Forecast > Formula`. The selected source is recorded on the cell,
/// including when that source's result is unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CellSource {
    /// The explicit observation stored on the node for this period (and
    /// visible under the observation cutoff).
    Value,
    /// The node's forecast method (forecast periods only).
    Forecast,
    /// The node's formula.
    Formula,
}

impl CellSource {
    /// Stable snake_case label, identical to the serde wire form
    /// (`"value"`, `"forecast"`, `"formula"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Value => "value",
            Self::Forecast => "forecast",
            Self::Formula => "formula",
        }
    }
}

/// Whether a cell has a value, and if not, which kind of absence it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CellStatus {
    /// The cell has a value, which may be a known zero.
    Value,
    /// No observation, forecast or formula supplies the period.
    Missing,
    /// The node's `where` predicate is false for the period.
    Masked,
    /// The cell has no value for the reason the cell states.
    Unavailable,
}

impl CellStatus {
    /// Stable snake_case label, identical to the serde wire form
    /// (`"value"`, `"missing"`, `"masked"`, `"unavailable"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Value => "value",
            Self::Missing => "missing",
            Self::Masked => "masked",
            Self::Unavailable => "unavailable",
        }
    }
}

/// One published cell: a node in a period.
///
/// A `value` cell carries a finite scalar or an exact [`Money`]; every other
/// status carries no value. An `unavailable` cell carries its reason.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "ResultCellWire")]
#[cfg_attr(feature = "json-schema", schemars(!try_from, deny_unknown_fields))]
pub struct ResultCell {
    /// Whether the cell has a value.
    pub status: CellStatus,
    /// The value of a `value` cell: a finite number for a scalar node, or
    /// exact tagged money for a monetary node. Absent otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<AmountOrScalar>,
    /// Why an `unavailable` cell has no value. Absent otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<UnavailableReason>,
    /// The value source selected for the cell. Absent for a masked cell, a
    /// missing cell, a cell left unavailable before a source was selected,
    /// and cells of hand-built results.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<CellSource>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResultCellWire {
    status: CellStatus,
    #[serde(default)]
    value: Option<AmountOrScalar>,
    #[serde(default)]
    reason: Option<UnavailableReason>,
    #[serde(default)]
    source: Option<CellSource>,
}

impl TryFrom<ResultCellWire> for ResultCell {
    type Error = String;

    fn try_from(wire: ResultCellWire) -> std::result::Result<Self, String> {
        let cell = Self {
            status: wire.status,
            value: wire.value,
            reason: wire.reason,
            source: wire.source,
        };
        let consistent = match cell.status {
            CellStatus::Value => {
                cell.reason.is_none() && cell.value.is_some_and(|value| value.value().is_finite())
            }
            CellStatus::Unavailable => cell.value.is_none() && cell.reason.is_some(),
            CellStatus::Missing | CellStatus::Masked => {
                cell.value.is_none() && cell.reason.is_none()
            }
        };
        if consistent {
            Ok(cell)
        } else {
            Err(format!(
                "a {} cell must carry {}",
                cell.status.as_str(),
                match cell.status {
                    CellStatus::Value => "a finite value and no reason",
                    CellStatus::Unavailable => "a reason and no value",
                    CellStatus::Missing | CellStatus::Masked => "no value and no reason",
                }
            ))
        }
    }
}

impl ResultCell {
    /// A value cell holding a finite scalar.
    ///
    /// # Arguments
    ///
    /// * `value` - Finite unitless value, in the node's own units.
    ///
    /// # Errors
    ///
    /// Returns an evaluation error if `value` is not finite.
    pub fn scalar(value: f64) -> crate::error::Result<Self> {
        if !value.is_finite() {
            return Err(crate::error::Error::eval(format!(
                "a result cell value must be finite, got {value}"
            )));
        }
        Ok(Self {
            status: CellStatus::Value,
            value: Some(AmountOrScalar::Scalar(value)),
            reason: None,
            source: None,
        })
    }

    /// A value cell holding an exact monetary amount.
    ///
    /// # Arguments
    ///
    /// * `money` - Exact amount in the node's currency.
    #[must_use]
    pub fn money(money: Money) -> Self {
        Self {
            status: CellStatus::Value,
            value: Some(AmountOrScalar::Amount(money)),
            reason: None,
            source: None,
        }
    }

    /// A cell without a value.
    ///
    /// # Arguments
    ///
    /// * `status` - `Missing` or `Masked`. Use [`Self::unavailable`] for an
    ///   unavailable cell; a `Value` status here gives a missing cell.
    #[must_use]
    pub fn without_value(status: CellStatus) -> Self {
        Self {
            status: match status {
                CellStatus::Masked => CellStatus::Masked,
                CellStatus::Value | CellStatus::Missing | CellStatus::Unavailable => {
                    CellStatus::Missing
                }
            },
            value: None,
            reason: None,
            source: None,
        }
    }

    /// An unavailable cell.
    ///
    /// # Arguments
    ///
    /// * `reason` - Why the cell has no value.
    #[must_use]
    pub fn unavailable(reason: UnavailableReason) -> Self {
        Self {
            status: CellStatus::Unavailable,
            value: None,
            reason: Some(reason),
            source: None,
        }
    }

    /// The same cell with its value source recorded.
    ///
    /// # Arguments
    ///
    /// * `source` - Value source selected for the cell, if any.
    #[must_use]
    pub fn with_source(mut self, source: Option<CellSource>) -> Self {
        self.source = source;
        self
    }

    /// The cell's value as a float: exact for a scalar, the nearest float for
    /// money. `None` when the cell has no value.
    #[must_use]
    pub fn numeric(&self) -> Option<f64> {
        self.value.map(|value| value.value())
    }

    /// The evaluator's typed cell for this published cell.
    pub(crate) fn to_cell(self) -> super::cell::Cell {
        use super::cell::Cell;
        match (self.status, self.value, self.reason) {
            (CellStatus::Value, Some(AmountOrScalar::Scalar(value)), _) => Cell::Scalar(value),
            (CellStatus::Value, Some(AmountOrScalar::Amount(money)), _) => Cell::Money(money),
            (CellStatus::Masked, _, _) => Cell::Masked,
            (CellStatus::Unavailable, _, Some(reason)) => Cell::Unavailable(reason),
            (CellStatus::Unavailable, _, None) => {
                Cell::Unavailable(UnavailableReason::MissingInput)
            }
            (CellStatus::Missing, _, _) | (CellStatus::Value, None, _) => Cell::Missing,
        }
    }

    pub(crate) fn from_cell(cell: super::cell::Cell, source: Option<CellSource>) -> Self {
        use super::cell::Cell;
        match cell {
            Cell::Scalar(value) => Self {
                status: CellStatus::Value,
                value: Some(AmountOrScalar::Scalar(value)),
                reason: None,
                source,
            },
            Cell::Money(money) => Self::money(money).with_source(source),
            Cell::Missing => Self::without_value(CellStatus::Missing),
            Cell::Masked => Self::without_value(CellStatus::Masked),
            Cell::Unavailable(reason) => Self::unavailable(reason).with_source(source),
        }
    }
}

/// What a result was evaluated from.
///
/// A consumer that is handed a result checks this against the model, period
/// plan and run it expects before trusting the cells.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct RunProvenance {
    /// Identifier of the evaluated model.
    pub model_id: String,
    /// Canonical content hash of the evaluated model
    /// ([`FinancialModelSpec::content_hash`](crate::types::FinancialModelSpec::content_hash)).
    pub model_content_hash: String,
    /// Identity of the source snapshot the model's observations came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_snapshot: Option<String>,
    /// Observations the run was allowed to read.
    pub observation_cutoff: super::ObservationCutoff,
    /// Valuation date the capital structure was priced at, when the run used
    /// a market.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "Option<finstack_quant_core::wire::DateWire>")
    )]
    pub valuation_date: Option<Date>,
    /// The random stream of every stochastic forecast node in a single run,
    /// in model node order: enough to replay its draws.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forecast_streams: Vec<ForecastStream>,
    /// Identity of this evaluation run: the canonical content hash of the
    /// fields above. Two runs of the same model content, snapshot, cutoff
    /// and valuation date share it.
    pub run_id: String,
}

/// Replay provenance of one stochastic forecast node in a single run.
///
/// The generator is the shared PCG64 started at `state` on stream `stream`
/// (`Pcg64Rng::new_with_stream(state, stream)`). Both are derived from the
/// versioned key: the key version, the configured seed, the node and the
/// single-run mode. Unsigned 64-bit values are decimal strings in JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ForecastStream {
    /// The stochastic forecast node.
    pub node_id: String,
    /// Version of the mapping from the key to the stream.
    pub stream_version: u32,
    /// The forecast method's exact configured seed.
    #[serde(with = "crate::types::forecast_seed")]
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    pub seed: u64,
    /// Effective PCG64 starting state.
    #[serde(with = "crate::types::forecast_seed")]
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    pub state: u64,
    /// Effective PCG64 stream selector.
    #[serde(with = "crate::types::forecast_seed")]
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    pub stream: u64,
}

impl RunProvenance {
    /// Build the provenance of a run and derive its identity.
    ///
    /// # Arguments
    ///
    /// * `model` - The evaluated model; its id, content hash and source
    ///   snapshot are recorded.
    /// * `observation_cutoff` - Observations the run was allowed to read.
    /// * `valuation_date` - Pricing date when the run used a market.
    ///
    /// # Errors
    ///
    /// Returns an evaluation error if the model cannot be hashed.
    pub fn for_run(
        model: &crate::types::FinancialModelSpec,
        observation_cutoff: super::ObservationCutoff,
        valuation_date: Option<Date>,
    ) -> crate::error::Result<Self> {
        let hash_error = |error: finstack_quant_core::Error| {
            crate::error::Error::eval(format!("cannot derive run provenance: {error}"))
        };
        let mut provenance = Self {
            model_id: model.id.clone(),
            model_content_hash: model.content_hash().map_err(hash_error)?,
            source_snapshot: model.source_snapshot.clone(),
            observation_cutoff,
            valuation_date,
            forecast_streams: forecast_streams(model)?,
            run_id: String::new(),
        };
        provenance.run_id =
            finstack_quant_core::canonical::content_hash(&provenance).map_err(hash_error)?;
        Ok(provenance)
    }
}

/// The single-run stream of every stochastic forecast node of `model`.
fn forecast_streams(
    model: &crate::types::FinancialModelSpec,
) -> crate::error::Result<Vec<ForecastStream>> {
    use crate::forecast::stream::{STREAM_VERSION, StreamKey};
    model
        .nodes
        .iter()
        .filter_map(|(node_id, node)| Some((node_id, node.forecast.as_ref()?.seed()?)))
        .map(|(node_id, seed)| {
            let (state, stream) = StreamKey {
                seed,
                node_id: node_id.as_str(),
                path: None,
            }
            .derive()?;
            Ok(ForecastStream {
                node_id: node_id.to_string(),
                stream_version: STREAM_VERSION,
                seed,
                state,
                stream,
            })
        })
        .collect()
}

/// Execution statistics for a statement-model evaluation.
///
/// Distinct from [`finstack_quant_core::config::ResultsMeta`], which is the
/// workspace-wide *audit* stamp (numeric mode, rounding context, FX policy).
/// This type records how the evaluation *ran* — timing, graph size, warnings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "json-schema", schemars(rename = "StatementEvalStats"))]
pub struct EvalStats {
    /// Evaluation time in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eval_time_ms: Option<u64>,

    /// Number of nodes evaluated
    pub num_nodes: usize,

    /// Number of periods evaluated
    pub num_periods: usize,

    /// Numeric mode used for evaluation
    #[serde(default)]
    pub numeric_mode: NumericMode,

    /// Whether parallel evaluation was used
    #[serde(default)]
    pub parallel: bool,

    /// Warnings encountered during evaluation (division by zero, NaN propagation, etc.)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<EvalWarning>,
}

impl Default for EvalStats {
    fn default() -> Self {
        Self {
            eval_time_ms: None,
            num_nodes: 0,
            num_periods: 0,
            numeric_mode: NumericMode::Float64,
            parallel: false,
            warnings: Vec::new(),
        }
    }
}

/// Numeric mode used for evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "json-schema", schemars(rename = "StatementNumericMode"))]
pub enum NumericMode {
    /// f64 floating-point mode (current default)
    #[default]
    Float64,
}

impl Default for StatementResult {
    fn default() -> Self {
        Self {
            schema_version: SchemaVersion::CURRENT,
            periods: None,
            cells: IndexMap::new(),
            node_value_types: IndexMap::new(),
            provenance: None,
            cs_cashflows: None,
            check_report: None,
            meta: EvalStats::default(),
        }
    }
}

impl StatementResult {
    /// Create empty results.
    ///
    /// Useful in tests or when you need a placeholder structure before running
    /// an evaluation.
    pub fn new() -> Self {
        Self::default()
    }

    /// The periods `nodes` is keyed by, in timeline order; empty for a
    /// hand-built result without a period plan.
    #[must_use]
    pub fn get_periods(&self) -> &[finstack_quant_core::dates::Period] {
        self.periods
            .as_ref()
            .map_or(&[], |plan| plan.periods.as_slice())
    }

    /// The numeric value of a cell: a scalar as stored, money as the nearest
    /// float.
    ///
    /// Returns `None` when the node or period is absent **and** when the cell
    /// has no value (missing, masked or unavailable); use
    /// [`get_cell`](Self::get_cell) to tell those apart. Use
    /// [`get_money`](Self::get_money) for the exact amount of a monetary
    /// cell.
    ///
    /// # Arguments
    /// * `node_id` - Node identifier as declared in the model
    /// * `period_id` - Period identifier of the cell
    #[must_use]
    pub fn get(&self, node_id: &str, period_id: &PeriodId) -> Option<f64> {
        self.get_cell(node_id, period_id)
            .and_then(ResultCell::numeric)
    }

    /// The typed cell of a node in a period, with its status, reason and
    /// source.
    ///
    /// # Arguments
    /// * `node_id` - Node identifier as declared in the model
    /// * `period_id` - Period identifier of the cell
    #[must_use]
    pub fn get_cell(&self, node_id: &str, period_id: &PeriodId) -> Option<&ResultCell> {
        self.cells
            .get(node_id)
            .and_then(|period_map| period_map.get(period_id))
    }

    /// The value source selected for a cell, if one was.
    ///
    /// # Arguments
    /// * `node_id` - Node identifier as declared in the model
    /// * `period_id` - Period identifier of the cell
    #[must_use]
    pub fn get_source(&self, node_id: &str, period_id: &PeriodId) -> Option<CellSource> {
        self.get_cell(node_id, period_id)
            .and_then(|cell| cell.source)
    }

    /// The exact amount of a monetary value cell.
    ///
    /// # Arguments
    /// * `node_id` - Node identifier as declared in the model
    /// * `period_id` - Period identifier of the cell
    #[must_use]
    pub fn get_money(&self, node_id: &str, period_id: &PeriodId) -> Option<Money> {
        self.get_cell(node_id, period_id)
            .and_then(|cell| cell.value)
            .and_then(|value| value.as_money())
    }

    /// The value of a scalar value cell; `None` for a monetary cell.
    ///
    /// # Arguments
    /// * `node_id` - Node identifier as declared in the model
    /// * `period_id` - Period identifier of the cell
    #[must_use]
    pub fn get_scalar(&self, node_id: &str, period_id: &PeriodId) -> Option<f64> {
        match self.get_cell(node_id, period_id)?.value? {
            AmountOrScalar::Scalar(value) => Some(value),
            AmountOrScalar::Amount(_) => None,
        }
    }

    /// The numeric value of a cell, with an explicit fallback for a missing
    /// observation.
    ///
    /// The fallback applies only to a `missing` cell at a node and period the
    /// result holds. It never stands in for a masked or unavailable cell, or
    /// for a node or period the result does not have.
    ///
    /// # Arguments
    /// * `node_id` - Node identifier as declared in the model
    /// * `period_id` - Period identifier of the cell
    /// * `fallback` - Value returned for a missing observation, in the
    ///   node's own units
    ///
    /// # Errors
    ///
    /// Returns a node-not-found error if the result has no cell for the node
    /// and period, and an evaluation error naming the cell's status and
    /// reason if the cell is masked or unavailable.
    pub fn get_or(
        &self,
        node_id: &str,
        period_id: &PeriodId,
        fallback: f64,
    ) -> crate::error::Result<f64> {
        let cell = self.get_cell(node_id, period_id).ok_or_else(|| {
            crate::error::Error::node_not_found(format!("{node_id} in period {period_id}"))
        })?;
        match (cell.numeric(), cell.status) {
            (Some(value), _) => Ok(value),
            (None, CellStatus::Missing) => Ok(fallback),
            (None, status) => Err(crate::error::Error::eval(format!(
                "Node '{node_id}' in period {period_id} is {}{}; a fallback applies only to a \
                 missing observation",
                status.as_str(),
                cell.reason
                    .map(|reason| format!(" ({})", reason.as_str()))
                    .unwrap_or_default()
            ))),
        }
    }

    /// Every cell of a node, in timeline order.
    ///
    /// # Arguments
    /// * `node_id` - Node identifier as declared in the model
    #[must_use]
    pub fn get_node(&self, node_id: &str) -> Option<&IndexMap<PeriodId, ResultCell>> {
        self.cells.get(node_id)
    }

    /// Whether the result holds cells for a node.
    ///
    /// # Arguments
    /// * `node_id` - Node identifier as declared in the model
    #[must_use]
    pub fn has_node(&self, node_id: &str) -> bool {
        self.cells.contains_key(node_id)
    }

    /// Node identifiers in declaration order.
    pub fn node_ids(&self) -> impl Iterator<Item = &str> + '_ {
        self.cells.keys().map(String::as_str)
    }

    /// The numeric values of a node's value cells, in timeline order. Cells
    /// without a value are skipped, not reported as zero.
    ///
    /// # Arguments
    /// * `node_id` - Node identifier as declared in the model
    pub fn all_periods(&self, node_id: &str) -> impl Iterator<Item = (&PeriodId, f64)> + '_ {
        self.get_node(node_id).into_iter().flat_map(|cells| {
            cells
                .iter()
                .filter_map(|(period, cell)| cell.numeric().map(|value| (period, value)))
        })
    }

    /// The numeric values of a node's value cells, keyed by period in
    /// timeline order: a scalar as stored, money as the nearest float. Cells
    /// without a value are left out, so a missing period is absent, never
    /// zero. `None` when the result has no such node.
    ///
    /// # Arguments
    /// * `node_id` - Node identifier as declared in the model
    #[must_use]
    pub fn numeric_series(&self, node_id: &str) -> Option<IndexMap<PeriodId, f64>> {
        self.cells.get(node_id).map(|cells| {
            cells
                .iter()
                .filter_map(|(period, cell)| cell.numeric().map(|value| (*period, value)))
                .collect()
        })
    }

    /// Store a finite scalar value cell, replacing any cell at that slot.
    ///
    /// For building or amending a result by hand. The node's units are not
    /// changed; declare them in [`node_value_types`](Self::node_value_types).
    ///
    /// # Arguments
    /// * `node_id` - Node identifier the cell belongs to
    /// * `period_id` - Period identifier of the cell
    /// * `value` - Finite unitless value
    ///
    /// # Errors
    ///
    /// Returns an evaluation error if `value` is not finite.
    pub fn set_scalar(
        &mut self,
        node_id: &str,
        period_id: PeriodId,
        value: f64,
    ) -> crate::error::Result<()> {
        self.set_cell(node_id, period_id, ResultCell::scalar(value)?);
        Ok(())
    }

    /// Replace a node's cells with finite scalar value cells.
    ///
    /// For building a result by hand.
    ///
    /// # Arguments
    /// * `node_id` - Node identifier the cells belong to
    /// * `values` - Finite unitless value per period, in timeline order
    ///
    /// # Errors
    ///
    /// Returns an evaluation error if a value is not finite. The node's
    /// existing cells are then unchanged.
    pub fn set_series(
        &mut self,
        node_id: &str,
        values: IndexMap<PeriodId, f64>,
    ) -> crate::error::Result<()> {
        let cells = values
            .into_iter()
            .map(|(period, value)| Ok((period, ResultCell::scalar(value)?)))
            .collect::<crate::error::Result<IndexMap<_, _>>>()?;
        self.cells.insert(node_id.to_string(), cells);
        Ok(())
    }

    /// Replace a node's cells with exact monetary value cells.
    ///
    /// For building a result by hand. The node's units are not changed;
    /// declare them in [`node_value_types`](Self::node_value_types).
    ///
    /// # Arguments
    /// * `node_id` - Node identifier the cells belong to
    /// * `values` - Exact amount per period in the node's currency, in
    ///   timeline order
    pub fn set_money_series(&mut self, node_id: &str, values: IndexMap<PeriodId, Money>) {
        self.cells.insert(
            node_id.to_string(),
            values
                .into_iter()
                .map(|(period, money)| (period, ResultCell::money(money)))
                .collect(),
        );
    }

    /// Store a cell, replacing any cell at that slot.
    ///
    /// # Arguments
    /// * `node_id` - Node identifier the cell belongs to
    /// * `period_id` - Period identifier of the cell
    /// * `cell` - Cell to store
    pub fn set_cell(&mut self, node_id: &str, period_id: PeriodId, cell: ResultCell) {
        self.cells
            .entry(node_id.to_string())
            .or_default()
            .insert(period_id, cell);
    }

    /// Float values of one node's cells in timeline order, with `NaN`
    /// standing for a cell without a value. For consumers that still compute
    /// on floats; the typed cells stay authoritative.
    pub(crate) fn float_series(&self, node_id: &str) -> Option<IndexMap<PeriodId, f64>> {
        self.cells.get(node_id).map(|cells| {
            cells
                .iter()
                .map(|(period, cell)| (*period, cell.numeric().unwrap_or(f64::NAN)))
                .collect()
        })
    }

    /// Float values of every cell for Monte Carlo path aggregation, with
    /// `NaN` standing for a cell without a value.
    pub(crate) fn path_values(&self) -> IndexMap<String, IndexMap<PeriodId, f64>> {
        self.cells
            .iter()
            .map(|(node_id, cells)| {
                (
                    node_id.clone(),
                    cells
                        .iter()
                        .map(|(period, cell)| (*period, cell.numeric().unwrap_or(f64::NAN)))
                        .collect(),
                )
            })
            .collect()
    }

    /// Give every scalar value cell of a monetary node its exact amount.
    ///
    /// # Errors
    ///
    /// Returns an evaluation error naming the node and period when a value
    /// cannot be represented as `Money`.
    pub(crate) fn retype_monetary_cells(&mut self) -> crate::error::Result<()> {
        for (node_id, cells) in &mut self.cells {
            let Some(NodeValueType::Monetary { currency }) = self.node_value_types.get(node_id)
            else {
                continue;
            };
            for (period_id, cell) in cells.iter_mut() {
                if let Some(AmountOrScalar::Scalar(value)) = cell.value {
                    let money = Money::from_model_value(value, *currency).map_err(|cause| {
                        crate::error::Error::eval(format!(
                            "Node '{node_id}' in period {period_id}: result {value} cannot be \
                             represented as {currency} money: {cause}"
                        ))
                    })?;
                    cell.value = Some(AmountOrScalar::Amount(money));
                }
            }
        }
        Ok(())
    }

    /// Record the resolved units of every evaluated node.
    ///
    /// # Arguments
    ///
    /// * `value_types` - Canonical units for every evaluated node, captured by
    ///   semantic validation when the evaluation plan was prepared.
    pub(crate) fn populate_value_types(&mut self, value_types: &IndexMap<NodeId, NodeValueType>) {
        for (node_id, value_type) in value_types {
            self.node_value_types
                .insert(node_id.as_str().to_string(), *value_type);
        }
    }
}

/// Cash claim category affected by a capital-structure warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CapitalStructureClaimCategory {
    /// Fee claims such as commitment or facility fees.
    Fees,
    /// Cash-interest claims.
    Interest,
}

impl std::fmt::Display for CapitalStructureClaimCategory {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Fees => "fees",
            Self::Interest => "interest",
        })
    }
}

/// Typed reason for a capital-structure evaluation warning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CapitalStructureWarning {
    /// The residual schedule opening differs from the stateful opening balance.
    ResidualScaleMismatch {
        /// Actual ratio of stateful to scheduled opening applied to the cashflows.
        applied_ratio: f64,
    },
    /// A negative creditor claim was neutralized instead of reducing other claims.
    NegativeClaimNeutralized {
        /// Payment category containing the invalid negative claim.
        category: CapitalStructureClaimCategory,
        /// Instrument whose claim was neutralized.
        instrument_id: String,
        /// Negative claim amount in the waterfall currency.
        amount: f64,
    },
    /// A negative available-cash pool was floored to zero before allocation.
    NegativeAvailableCashFloored {
        /// Statement node supplying the available-cash amount.
        node_id: String,
        /// Negative amount that was floored, in the waterfall currency.
        amount: f64,
    },
    /// Available cash was insufficient to pay a cash-interest claim.
    InterestShortfall {
        /// Instrument carrying the unpaid claim forward.
        instrument_id: String,
        /// Unpaid amount in the waterfall currency.
        amount: f64,
    },
    /// Available cash was insufficient to pay a fee claim.
    FeeShortfall {
        /// Instrument carrying the unpaid claim forward.
        instrument_id: String,
        /// Unpaid amount in the waterfall currency.
        amount: f64,
    },
    /// Available cash was insufficient to pay scheduled principal.
    PrincipalShortfall {
        /// Instrument carrying the unpaid claim forward.
        instrument_id: String,
        /// Unpaid amount in the waterfall currency.
        amount: f64,
    },
}

/// Warning emitted during evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum EvalWarning {
    /// A percentage change or growth rate was measured from a zero base, so
    /// its cell is unavailable. A division by zero in a formula is an error,
    /// not this warning.
    DivisionByZero {
        /// Identifier of the node that triggered the warning.
        node_id: String,
        /// Period in which the warning occurred.
        #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
        period: PeriodId,
    },
    /// Non-finite value (NaN, Inf, -Inf) detected when storing a node result.
    ///
    /// This warning is emitted by the finiteness validation pipeline so that
    /// consumers can identify which node/period introduced bad values.
    NonFiniteValue {
        /// Identifier of the node that produced the non-finite value.
        node_id: String,
        /// Period in which the warning occurred.
        #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
        period: PeriodId,
        /// The actual non-finite value, encoded as `"nan"`, `"inf"`, or `"-inf"`.
        #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
        #[cfg_attr(feature = "json-schema", schemars(with = "ResultNumber"))]
        value: f64,
    },
    /// Capital-structure extraction or waterfall processing required a guarded fallback.
    CapitalStructure {
        /// Period in which the warning was raised.
        #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
        period: PeriodId,
        /// Typed reason and associated diagnostic values.
        warning: CapitalStructureWarning,
    },
    /// A forecast's published level was clipped to its `min` / `max` bound.
    /// The raw level still drives the forecast's later steps.
    ForecastClipped {
        /// Identifier of the forecast node.
        node_id: String,
        /// Period whose published level was clipped.
        #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
        period: PeriodId,
        /// Level the forecast generated, in the node's units.
        raw: f64,
        /// Level published after the bound, in the node's units.
        published: f64,
    },
    /// A full-history statistic (`std`, `var`, `median`) was computed over a
    /// history with gaps, so its sample is smaller than its slot count.
    GappedSample {
        /// Identifier of the node whose statistic has gaps.
        node_id: String,
        /// Period the statistic was computed for.
        #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
        period: PeriodId,
        /// Name of the statistic.
        function: String,
        /// Observations that had a value.
        observations: usize,
        /// Plan slots from the model start through the period.
        slots: usize,
    },
}

// The non-finite number form carried by evaluation warnings.
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(transparent)]
struct ResultNumber(
    #[serde(with = "finstack_quant_core::wire::non_finite_f64")]
    #[cfg_attr(feature = "json-schema", schemars(with = "serde_json::Value", extend("oneOf" = [{"type": "number"}, {"enum": ["nan", "inf", "-inf"]}])))]
    f64,
);
