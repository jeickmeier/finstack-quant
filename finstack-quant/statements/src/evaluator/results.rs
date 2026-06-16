//! Results types for statement evaluation.

use crate::types::NodeValueType;
use finstack_quant_core::dates::PeriodId;
use finstack_quant_core::money::Money;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::types::FinancialModelSpec;

/// Wire-format schema version for [`StatementResult`].
///
/// Bump this when adding, removing, or renaming fields in a way that is NOT
/// handled by `#[serde(default)]` on the new field. Document every bump in
/// the workspace `CHANGELOG.md` and `docs/SERDE_STABILITY.md`.
pub const STATEMENT_RESULT_SCHEMA_VERSION: u32 = 1;

fn default_statement_result_schema_version() -> u32 {
    STATEMENT_RESULT_SCHEMA_VERSION
}

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
///         (PeriodId::quarter(2025, 1), 100_000.0.into()),
///         (PeriodId::quarter(2025, 2), 105_000.0.into()),
///     ])
///     .compute("gross_profit", "revenue * 0.6")?
///     .build()?;
///
/// let mut evaluator = Evaluator::new();
/// let result = evaluator.evaluate(&model)?;
/// assert!(result.get("gross_profit", &PeriodId::quarter(2025, 1)).is_some());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatementResult {
    /// Wire-format schema version. Current wire format: `STATEMENT_RESULT_SCHEMA_VERSION`.
    #[serde(default = "default_statement_result_schema_version")]
    pub schema_version: u32,

    /// Map of node_id → (period_id → value) [f64 for scalar results]
    pub nodes: IndexMap<String, IndexMap<PeriodId, f64>>,

    /// Map of node_id → (period_id → Money) for monetary nodes
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
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
    pub meta: ResultsMeta,
}

/// Metadata about evaluation results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultsMeta {
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

    /// Rounding context reserved for future fixed-point evaluation metadata.
    ///
    /// Kept in the wire format for forward compatibility; statement evaluation
    /// currently runs in [`NumericMode::Float64`] and leaves this as `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rounding_context: Option<finstack_quant_core::config::RoundingContext>,

    /// Whether parallel evaluation was used
    #[serde(default)]
    pub parallel: bool,

    /// Warnings encountered during evaluation (division by zero, NaN propagation, etc.)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<EvalWarning>,
}

impl Default for ResultsMeta {
    fn default() -> Self {
        Self {
            eval_time_ms: None,
            num_nodes: 0,
            num_periods: 0,
            numeric_mode: NumericMode::Float64,
            rounding_context: None,
            parallel: false,
            warnings: Vec::new(),
        }
    }
}

/// Numeric mode used for evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NumericMode {
    /// f64 floating-point mode (current default)
    #[default]
    Float64,
    /// Reserved fixed-point mode.
    ///
    /// The variant remains in the serde/public API so saved result metadata can
    /// evolve without renaming the enum. The evaluator currently emits
    /// [`NumericMode::Float64`] only.
    Decimal,
}

impl Default for StatementResult {
    fn default() -> Self {
        Self {
            schema_version: STATEMENT_RESULT_SCHEMA_VERSION,
            nodes: IndexMap::new(),
            monetary_nodes: IndexMap::new(),
            node_value_types: IndexMap::new(),
            cs_cashflows: None,
            check_report: None,
            meta: ResultsMeta::default(),
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
        // Check if this is a scalar node (not monetary)
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
                        let money_map = period_map
                            .iter()
                            .map(|(period_id, &v)| (*period_id, Money::new(v, *currency)))
                            .collect();
                        self.monetary_nodes
                            .insert(node_id_str.to_string(), money_map);
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
                        let money_map = period_map
                            .iter()
                            .map(|(period_id, &v)| (*period_id, Money::new(v, currency)))
                            .collect();
                        self.monetary_nodes
                            .insert(node_id_str.to_string(), money_map);
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
    pub fn to_table_long(&self) -> Result<finstack_quant_core::table::TableEnvelope> {
        super::export::to_table_long(self)
    }

    /// Export to a long-format table with node filtering.
    ///
    /// If `node_filter` is empty, all nodes are included.
    ///
    /// # Arguments
    /// * `node_filter` - Optional list of node identifiers to keep
    pub fn to_table_long_filtered(
        &self,
        node_filter: &[&str],
    ) -> Result<finstack_quant_core::table::TableEnvelope> {
        super::export::to_table_long_filtered(self, node_filter)
    }

    /// Export to a wide-format table.
    ///
    /// Schema: `(period_id, <node1>, <node2>, ...)`
    pub fn to_table_wide(&self) -> Result<finstack_quant_core::table::TableEnvelope> {
        super::export::to_table_wide(self)
    }
}

/// Warning emitted during evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvalWarning {
    /// Division by zero encountered
    DivisionByZero {
        /// Identifier of the node that triggered the warning.
        node_id: String,
        /// Period in which the warning occurred.
        period: PeriodId,
    },
    /// NaN value bubbled up to a node result
    NaNPropagated {
        /// Identifier of the node that produced the NaN value.
        node_id: String,
        /// Period in which the warning occurred.
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
        period: PeriodId,
        /// The actual non-finite value (NaN, Inf, or -Inf).
        value: f64,
    },
    /// Capital-structure cashflow classification was ignored during statement extraction.
    CapitalStructureCashflowIgnored {
        /// Period in which the ignored cashflow was encountered.
        period: PeriodId,
        /// Ignored cashflow kind.
        kind: String,
        /// Original cashflow date as a string for diagnostics.
        cashflow_date: String,
    },
}
