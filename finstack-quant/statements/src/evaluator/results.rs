//! Results types for statement evaluation.

use crate::types::NodeValueType;
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::dates::{Date, PeriodId};
use finstack_quant_core::money::Money;
use finstack_quant_core::wire::SchemaVersion;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::types::FinancialModelSpec;

/// Results from evaluating a financial model.
///
/// Values are stored as an [`IndexMap`] keyed by node identifier so you can
/// preserve declaration order when presenting them. Helper methods make it easy
/// to access per-period values or export to Polars.
///
/// Results now support dual storage:
/// - `nodes`: f64 values for scalar results
/// - `monetary_nodes`: Money values for currency-aware monetary nodes
/// - `node_value_types`: Track which nodes are monetary vs scalar
///
/// # Example
///
/// ```rust
/// # use finstack_quant_statements::builder::ModelBuilder;
/// # use finstack_quant_statements::evaluator::Evaluator;
/// # use finstack_quant_core::dates::PeriodId;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let model = ModelBuilder::new("demo")
///     .periods("2025Q1..Q2", None)?
///     .value("revenue", &[
///         (PeriodId::quarter(2025, 1).expect("valid period fixture"), 100_000.0.into()),
///         (PeriodId::quarter(2025, 2).expect("valid period fixture"), 105_000.0.into()),
///     ])
///     .compute("gross_profit", "revenue * 0.6")?
///     .build()?;
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

    /// Map of node_id → (period_id → value) [f64 for scalar results]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "IndexMap<String, IndexMap<String, f64>>")
    )]
    pub nodes: IndexMap<String, IndexMap<PeriodId, f64>>,

    /// Map of node_id → (period_id → Money) for monetary nodes
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "IndexMap<String, IndexMap<String, Money>>")
    )]
    pub monetary_nodes: IndexMap<String, IndexMap<PeriodId, Money>>,

    /// Track value types for each node
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub node_value_types: IndexMap<String, NodeValueType>,

    /// Capital structure cashflows (populated when model has a capital_structure)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cs_cashflows: Option<crate::capital_structure::CapitalStructureCashflows>,

    /// Check report from inline validation (None if no checks configured)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_report: Option<crate::checks::CheckReport>,

    /// Metadata about the evaluation
    pub meta: EvalStats,
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
// Distinct from `finstack_quant_core::config::NumericMode`, which carries a
// different value set.
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
            nodes: IndexMap::new(),
            monetary_nodes: IndexMap::new(),
            node_value_types: IndexMap::new(),
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

    /// Get the value for a node at a specific period.
    ///
    /// # Arguments
    /// * `node_id` - Identifier of the node (e.g., `"revenue"`)
    /// * `period_id` - Period key returned by the evaluator or builder
    ///
    /// # Returns
    /// `Some(value)` if the datapoint exists, otherwise `None`.
    pub fn get(&self, node_id: &str, period_id: &PeriodId) -> Option<f64> {
        self.nodes
            .get(node_id)
            .and_then(|period_map| period_map.get(period_id).copied())
    }

    /// Get the Money value for a monetary node at a specific period.
    ///
    /// # Arguments
    /// * `node_id` - Identifier of the monetary node (e.g., `"revenue"`)
    /// * `period_id` - Period key
    ///
    /// # Returns
    /// `Some(Money)` if the node is monetary and has a value for this period, otherwise `None`.
    pub fn get_money(&self, node_id: &str, period_id: &PeriodId) -> Option<Money> {
        self.monetary_nodes
            .get(node_id)
            .and_then(|period_map| period_map.get(period_id).copied())
    }

    /// Get the scalar value for a non-monetary node at a specific period.
    ///
    /// # Arguments
    /// * `node_id` - Identifier of the scalar node (e.g., `"gross_margin_pct"`)
    /// * `period_id` - Period key
    ///
    /// # Returns
    /// `Some(f64)` if the node is scalar and has a value for this period, otherwise `None`.
    pub fn get_scalar(&self, node_id: &str, period_id: &PeriodId) -> Option<f64> {
        if let Some(NodeValueType::Scalar) = self.node_value_types.get(node_id) {
            self.get(node_id, period_id)
        } else {
            None
        }
    }

    /// Get all period values for a specific node.
    ///
    /// # Arguments
    /// * `node_id` - Identifier to look up
    pub fn get_node(&self, node_id: &str) -> Option<&IndexMap<PeriodId, f64>> {
        self.nodes.get(node_id)
    }

    /// Get an iterator over all periods for a node.
    ///
    /// # Arguments
    /// * `node_id` - Identifier to iterate over
    pub fn all_periods(&self, node_id: &str) -> impl Iterator<Item = (&PeriodId, f64)> + '_ {
        self.get_node(node_id)
            .into_iter()
            .flat_map(|map| map.iter().map(|(k, v)| (k, *v)))
    }

    /// Get value or default.
    ///
    /// # Arguments
    /// * `node_id` - Identifier to look up
    /// * `period` - Period identifier
    /// * `default` - Value to return when the datapoint is missing
    pub fn get_or(&self, node_id: &str, period: &PeriodId, default: f64) -> f64 {
        self.get(node_id, period).unwrap_or(default)
    }

    /// Infer and populate node value types and monetary node maps from a model.
    ///
    /// For each node, determines whether it is monetary or scalar based on:
    /// 1. Explicit `value_type` on the node spec (highest priority)
    /// 2. Inferred from the node's input values (currency homogeneity)
    /// 3. Default to scalar
    ///
    /// Populates `node_value_types` and `monetary_nodes` on this result.
    pub(crate) fn populate_value_types(&mut self, model: &FinancialModelSpec) -> Result<()> {
        for (node_id, node_spec) in &model.nodes {
            let node_id_str = node_id.as_str();

            if let Some(value_type) = &node_spec.value_type {
                self.node_value_types
                    .insert(node_id_str.to_string(), *value_type);

                if let NodeValueType::Monetary { currency } = value_type {
                    if let Some(period_map) = self.nodes.get(node_id_str) {
                        let (money_map, skipped) =
                            monetary_map_skipping_nonfinite(period_map, *currency, node_id_str);
                        self.monetary_nodes
                            .insert(node_id_str.to_string(), money_map);
                        self.meta.warnings.extend(skipped);
                    }
                }
            } else if let Some(values) = &node_spec.values {
                if let Some(NodeValueType::Monetary { currency }) =
                    crate::types::infer_series_value_type(values.values())?
                {
                    self.node_value_types.insert(
                        node_id_str.to_string(),
                        NodeValueType::Monetary { currency },
                    );

                    if let Some(period_map) = self.nodes.get(node_id_str) {
                        let (money_map, skipped) =
                            monetary_map_skipping_nonfinite(period_map, currency, node_id_str);
                        self.monetary_nodes
                            .insert(node_id_str.to_string(), money_map);
                        self.meta.warnings.extend(skipped);
                    }
                } else {
                    self.node_value_types
                        .insert(node_id_str.to_string(), NodeValueType::Scalar);
                }
            } else {
                self.node_value_types
                    .insert(node_id_str.to_string(), NodeValueType::Scalar);
            }
        }
        Ok(())
    }

    /// Export to a long-format table.
    ///
    /// Schema: `(node_id, period_id, value, value_money, currency, value_type)`.
    /// Rows preserve the result's node and period declaration order. Monetary
    /// nodes duplicate their numerical value in `value_money` and set
    /// `currency`; scalar nodes leave those two fields null.
    ///
    /// # Errors
    ///
    /// Returns a table-construction error if the result cannot be represented
    /// as a valid [`finstack_quant_core::table::TableEnvelope`]. Empty results
    /// are valid and produce an empty table with the full six-column schema.
    pub fn to_table_long(&self) -> Result<finstack_quant_core::table::TableEnvelope> {
        super::export::to_table_long(self)
    }

    /// Export to a wide-format table.
    ///
    /// Schema: `(period_id, <node1>, <node2>, ...)`. One row is emitted per
    /// unique period in ascending chronological order, and node columns follow
    /// result declaration order. Missing node-period observations are encoded
    /// as `NaN`, not zero, so downstream analytics can distinguish absence from
    /// an evaluated zero.
    ///
    /// # Errors
    ///
    /// Returns a table-construction error if a node identifier or result shape
    /// cannot be represented in a valid table envelope. Empty results are valid
    /// and produce a zero-row table containing only `period_id`.
    pub fn to_table_wide(&self) -> Result<finstack_quant_core::table::TableEnvelope> {
        super::export::to_table_wide(self)
    }
}

/// Build a `PeriodId -> Money` map for a monetary node, skipping any
/// non-finite (`NaN`/`±Inf`) cell.
///
/// The evaluator deliberately stores non-finite results (e.g. a division by
/// zero) and surfaces them as warnings rather than aborting. `Money::new`
/// asserts finiteness and would panic on those cells, so this uses
/// `Money::new` and returns a `NonFiniteValue` warning per skipped cell
/// instead. Returns the money map and the warnings for the skipped cells.
fn monetary_map_skipping_nonfinite(
    period_map: &IndexMap<PeriodId, f64>,
    currency: finstack_quant_core::currency::Currency,
    node_id: &str,
) -> (IndexMap<PeriodId, Money>, Vec<EvalWarning>) {
    let mut money_map = IndexMap::with_capacity(period_map.len());
    let mut skipped = Vec::new();
    for (period_id, &v) in period_map {
        match Money::new(v, currency) {
            Ok(money) => {
                money_map.insert(*period_id, money);
            }
            Err(_) => skipped.push(EvalWarning::NonFiniteValue {
                node_id: node_id.to_string(),
                period: *period_id,
                value: v,
            }),
        }
    }
    (money_map, skipped)
}

/// Cash claim category affected by a capital-structure warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    /// A schedule-to-model balance ratio exceeded the safety bound and was clamped.
    ScaleClamped {
        /// Unbounded ratio calculated from model and schedule balances.
        raw_ratio: f64,
        /// Ratio used after applying the safety bound.
        clamped_ratio: f64,
    },
    /// A contractual cashflow kind is not represented in statement debt service.
    CashflowIgnored {
        /// Canonical cashflow classification that was excluded.
        cashflow_kind: CFKind,
        /// Original contractual payment date.
        #[serde(with = "finstack_quant_core::wire::date")]
        #[cfg_attr(
            feature = "json-schema",
            schemars(with = "finstack_quant_core::wire::DateWire")
        )]
        cashflow_date: Date,
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
    /// Division by zero encountered
    DivisionByZero {
        /// Identifier of the node that triggered the warning.
        node_id: String,
        /// Period in which the warning occurred.
        #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
        period: PeriodId,
    },
    /// NaN value bubbled up to a node result
    #[serde(rename = "nan_propagated")]
    NaNPropagated {
        /// Identifier of the node that produced the NaN value.
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
        /// The actual non-finite value (NaN, Inf, or -Inf).
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
    /// One or more non-finite inputs were skipped by a skip-NaN aggregate
    /// (`sum`, `mean`, ...).
    ///
    /// The aggregate's skip-NaN policy is intentional, but silently dropping a
    /// broken line item can mask upstream problems (e.g. a division by zero in
    /// one argument), so the drop is surfaced here.
    NonFiniteSkipped {
        /// Identifier of the node whose aggregate dropped inputs.
        node_id: String,
        /// Period in which the drop occurred.
        #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
        period: PeriodId,
        /// Name of the aggregate function that dropped values.
        function: String,
        /// Number of non-finite inputs dropped.
        count: usize,
    },
}
