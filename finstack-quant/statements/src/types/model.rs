//! Financial model specification types.

use crate::error::{Error, Result};
use crate::types::{NodeId, NodeSpec, NodeType};
use finstack_quant_core::dates::Period;
use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize};

/// Current on-disk schema version for [`FinancialModelSpec`].
///
/// Bump on breaking wire-format changes. `validate_schema_version` rejects
/// versions outside `1..=CURRENT_SCHEMA_VERSION`; older payloads that are no
/// longer structurally compatible must be re-exported rather than migrated.
pub const CURRENT_SCHEMA_VERSION: u32 = 2;

/// Top-level financial model specification.
///
/// This is the wire format for a complete financial statement model.
/// It can be serialized to/from JSON for storage and interchange.
///
/// Period order in [`FinancialModelSpec::periods`] defines the evaluation timeline:
/// engines iterate periods in this sequence when resolving dependencies and rolling
/// windows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinancialModelSpec {
    /// Unique model identifier
    pub id: String,

    /// Ordered list of periods (quarters, months, etc.).
    ///
    /// Evaluation follows this order end-to-end (dependency resolution and time-series
    /// helpers assume a single coherent timeline).
    pub periods: Vec<Period>,

    /// Map of node_id → NodeSpec
    pub nodes: IndexMap<NodeId, NodeSpec>,

    /// Capital structure specification (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capital_structure: Option<CapitalStructureSpec>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub meta: IndexMap<String, serde_json::Value>,

    /// Schema version for forward compatibility.
    ///
    /// Validated on deserialize against `CURRENT_SCHEMA_VERSION`; unknown
    /// versions fail deserialization rather than silently accepting drift.
    #[serde(
        default = "default_schema_version",
        deserialize_with = "deserialize_schema_version"
    )]
    pub schema_version: u32,
}

impl FinancialModelSpec {
    /// Create a [`crate::builder::ModelBuilder`] for constructing a model specification.
    ///
    /// This is the preferred entry point for staged model creation. The
    /// returned builder uses typestate to require `.periods()` before node
    /// definitions can be added.
    #[must_use]
    pub fn builder(
        id: impl Into<String>,
    ) -> crate::builder::ModelBuilder<crate::builder::NeedPeriods> {
        crate::builder::ModelBuilder::new(id)
    }

    /// Create a new model specification directly from a period list.
    ///
    /// Prefer [`FinancialModelSpec::builder`] for user-facing model construction:
    /// the builder validates period ranges and catches stale references to
    /// undefined nodes. This direct constructor is retained for programmatic
    /// use (scenarios, template generators, tests) where callers already have
    /// a validated `Vec<Period>` and intend to add nodes by hand.
    ///
    /// # Arguments
    /// * `id` - Identifier used to reference the model
    /// * `periods` - Ordered list of [`Period`](finstack_quant_core::dates::Period) instances
    #[must_use]
    pub fn new(id: impl Into<String>, periods: Vec<Period>) -> Self {
        Self {
            id: id.into(),
            periods,
            nodes: IndexMap::new(),
            capital_structure: None,
            meta: IndexMap::new(),
            schema_version: CURRENT_SCHEMA_VERSION,
        }
    }

    /// Add a node to the model.
    ///
    /// # Arguments
    /// * `node` - Fully configured [`NodeSpec`](crate::types::NodeSpec)
    pub fn add_node(&mut self, node: NodeSpec) {
        self.nodes.insert(node.node_id.clone(), node);
    }

    /// Get a mutable reference to a node by ID.
    ///
    /// # Arguments
    /// * `node_id` - Identifier to search for
    pub fn get_node_mut(&mut self, node_id: &str) -> Option<&mut NodeSpec> {
        self.nodes.get_mut(node_id)
    }

    /// Get an immutable reference to a node by ID.
    ///
    /// # Arguments
    /// * `node_id` - Identifier to search for
    pub fn get_node(&self, node_id: &str) -> Option<&NodeSpec> {
        self.nodes.get(node_id)
    }

    /// Check if the model contains a node.
    ///
    /// # Arguments
    /// * `node_id` - Identifier to look up
    pub fn has_node(&self, node_id: &str) -> bool {
        self.nodes.contains_key(node_id)
    }

    /// Validate that periods are chronological and actuals form a prefix.
    ///
    /// Both rules exist to prevent look-ahead, and neither is enforced by the
    /// types: `periods_explicit` and raw JSON both accept an arbitrary `Vec`.
    ///
    /// Forecasting anchors on the **last** actual period. With actuals
    /// interleaved among forecast periods — say `[A 2024Q1, F 2024Q2,
    /// A 2024Q3, F 2024Q4]`, a "fill in the gap quarter" layout — the forecast
    /// covering 2024Q2 would be anchored on the 2024Q3 actual, a value from
    /// *after* the period being forecast, and the random-walk recurrence would
    /// then carry that future information forward. Positional seasonal and
    /// time-series indexing assumes contiguous forecast periods for the same
    /// reason.
    ///
    /// # Errors
    ///
    /// Returns an error if periods are not strictly increasing, or if an actual
    /// period appears after any forecast period.
    fn validate_period_timeline(periods: &[finstack_quant_core::dates::Period]) -> Result<()> {
        for window in periods.windows(2) {
            let (prev, next) = match window {
                [prev, next] => (prev, next),
                _ => continue,
            };
            if next.id <= prev.id {
                return Err(Error::build(format!(
                    "Model periods must be in strictly increasing chronological order, but \
                     {} appears after {}. Forecasts anchor on the last actual period, so an \
                     out-of-order timeline can silently anchor a forecast on a later value.",
                    next.id, prev.id
                )));
            }
        }

        if let Some(first_forecast) = periods.iter().position(|p| !p.is_actual) {
            if let Some(stray) = periods
                .iter()
                .skip(first_forecast)
                .find(|p| p.is_actual)
                .map(|p| p.id)
            {
                let first_forecast_id = periods
                    .get(first_forecast)
                    .map(|p| p.id.to_string())
                    .unwrap_or_default();
                return Err(Error::build(format!(
                    "Actual periods must form a prefix of the timeline, but actual period {stray} \
                     appears after forecast period {first_forecast_id}. Forecasts anchor on the \
                     last actual period, so an actual after a forecast would anchor that forecast \
                     on a value from a later period (look-ahead). Mark the intervening periods as \
                     actuals, or move the actual before the first forecast period."
                )));
            }
        }

        Ok(())
    }

    /// Validate model semantics that serde alone cannot enforce.
    ///
    /// This mirrors the terminal validation performed by the builder so JSON
    /// entry points reject structurally invalid models before evaluation. It
    /// infers omitted node value types from explicit values, validates formula
    /// syntax and known monetary/scalar dimensions, and validates the optional
    /// capital-structure waterfall. This method may populate `value_type` on
    /// nodes that have explicit values and no declared type.
    ///
    /// # Errors
    ///
    /// Returns a build error for an empty period set, reserved node IDs,
    /// incompatible node-type fields (such as a calculated node with values),
    /// mixed scalar/monetary values or currencies within a node, invalid
    /// formulas or known dimensions, or an invalid waterfall. Unknown formula
    /// references are warned about and deferred to evaluation to allow optional
    /// registry metrics; callers should treat that warning as a likely model
    /// authoring error and resolve it before production use.
    pub fn validate_semantics(&mut self) -> Result<()> {
        if self.periods.is_empty() {
            return Err(Error::build("Model must have at least one period"));
        }

        Self::validate_period_timeline(&self.periods)?;

        for node_id in self.nodes.keys() {
            crate::builder::validate_node_id(node_id.as_str())?;
        }

        for (node_id, node) in &self.nodes {
            match node.node_type {
                NodeType::Value => {
                    if node.formula_text.is_some() {
                        return Err(Error::build(format!(
                            "Value node '{}' cannot have a formula — use Mixed or Calculated type",
                            node_id
                        )));
                    }
                }
                NodeType::Calculated => {
                    if node.values.is_some() {
                        return Err(Error::build(format!(
                            "Calculated node '{}' cannot have explicit values — use Mixed or Value type",
                            node_id
                        )));
                    }
                    if node.forecast.is_some() {
                        return Err(Error::build(format!(
                            "Calculated node '{}' cannot have a forecast — use Mixed type (a \
                             Calculated node is formula-only; a forecast would override the \
                             formula in forecast periods)",
                            node_id
                        )));
                    }
                }
                NodeType::Mixed => {}
            }
        }

        for node in self.nodes.values_mut() {
            if let Some(values) = &node.values {
                let inferred = crate::types::infer_series_value_type(values.values())?;
                if node.value_type.is_none() {
                    node.value_type = inferred;
                }
            }
        }

        let node_value_types: IndexMap<NodeId, crate::types::NodeValueType> = self
            .nodes
            .iter()
            .filter_map(|(node_id, node)| {
                node.value_type
                    .map(|value_type| (node_id.clone(), value_type))
            })
            .collect();

        for (node_id, node) in &self.nodes {
            if let Some(formula) = &node.formula_text {
                let ast = crate::dsl::parse_formula(formula).map_err(|e| {
                    Error::build(format!("Invalid formula on node '{}': {}", node_id, e))
                })?;
                crate::dsl::compiler::validate_dimensions(&ast, &node_value_types).map_err(
                    |e| Error::build(format!("Invalid formula on node '{}': {}", node_id, e)),
                )?;
                crate::dsl::compile(&ast).map_err(|e| {
                    Error::build(format!("Invalid formula on node '{}': {}", node_id, e))
                })?;
            }

            if let Some(where_text) = &node.where_text {
                let ast = crate::dsl::parse_formula(where_text).map_err(|e| {
                    Error::build(format!("Invalid where clause on node '{}': {}", node_id, e))
                })?;
                crate::dsl::compiler::validate_dimensions(&ast, &node_value_types).map_err(
                    |e| Error::build(format!("Invalid where clause on node '{}': {}", node_id, e)),
                )?;
                crate::dsl::compile(&ast).map_err(|e| {
                    Error::build(format!("Invalid where clause on node '{}': {}", node_id, e))
                })?;
            }
        }

        if let Some(cs) = &self.capital_structure {
            if let Some(waterfall) = &cs.waterfall {
                waterfall.validate()?;
            }
        }

        match crate::evaluator::DependencyGraph::from_model(self) {
            Ok(graph) => graph.detect_cycles()?,
            Err(e) => {
                // The graph fails to build when a formula references an unknown
                // identifier (which also means cycle detection is skipped for
                // this model). This is tolerated rather than fatal because
                // `with_builtin_metrics` intentionally registers `fin.*` metrics
                // that reference user nodes which may not all be present. Surface
                // it at `warn` (not `debug`) so a genuine typo — and the skipped
                // cycle check — is visible rather than silent.
                tracing::warn!(
                    model_id = %self.id,
                    error = %e,
                    "Skipping cycle detection: dependency graph could not be built \
                     (a formula references an unknown identifier). Verify node references; \
                     cycles will only be caught later, at evaluation."
                );
            }
        }

        Ok(())
    }
}

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

fn deserialize_schema_version<'de, D>(deserializer: D) -> std::result::Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    let v = u32::deserialize(deserializer)?;
    validate_schema_version(v).map_err(serde::de::Error::custom)?;
    Ok(v)
}

fn validate_schema_version(v: u32) -> std::result::Result<(), String> {
    if v == 0 || v > CURRENT_SCHEMA_VERSION {
        return Err(format!(
            "unsupported FinancialModelSpec schema_version {v}; this build understands versions 1..={CURRENT_SCHEMA_VERSION}"
        ));
    }
    Ok(())
}

/// Capital structure specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapitalStructureSpec {
    /// Debt instruments (bonds, loans, swaps)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub debt_instruments: Vec<DebtInstrumentSpec>,

    /// Reserved equity instruments payloads.
    ///
    /// The field is currently not consumed by the waterfall engine. It is kept
    /// as a serde-compatible extension point for callers already persisting
    /// capital-structure JSON with equity-side metadata.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub equity_instruments: Vec<serde_json::Value>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub meta: IndexMap<String, serde_json::Value>,

    /// Optional reporting currency override for capital structure totals
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reporting_currency: Option<finstack_quant_core::currency::Currency>,

    /// Optional FX conversion policy override (defaults to CashflowDate)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fx_policy: Option<finstack_quant_core::money::fx::FxConversionPolicy>,

    /// Optional waterfall specification for dynamic cash flow allocation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waterfall: Option<crate::capital_structure::WaterfallSpec>,
}

/// Debt instrument specification.
///
/// An identifier paired with a canonical tagged instrument payload. With the
/// default `valuation-integration` feature, the `spec` value is resolved through
/// the valuations instrument registry from the tagged form
/// `{"type": "<tag>", "spec": {...}}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebtInstrumentSpec {
    /// Instrument identifier (key within the capital structure).
    pub id: String,
    /// Canonical tagged instrument payload: `{"type": "...", "spec": {...}}`.
    pub spec: serde_json::Value,
}

#[cfg(test)]
mod period_timeline_tests {
    use super::*;
    use crate::types::AmountOrScalar;
    use finstack_quant_core::dates::PeriodId;

    fn period(id: PeriodId, is_actual: bool) -> Period {
        let range = finstack_quant_core::dates::build_periods(&format!("{id}..{id}"), None)
            .expect("single-period range");
        let mut p = range.periods.into_iter().next().expect("one period");
        p.is_actual = is_actual;
        p
    }

    fn model_with_periods(periods: Vec<Period>) -> FinancialModelSpec {
        let mut model = FinancialModelSpec::new("timeline", periods);
        // A trivial node so the model is otherwise valid.
        let first = model.periods.first().expect("period").id;
        model.add_node(
            NodeSpec::new("revenue", NodeType::Value).with_values(
                [(first, AmountOrScalar::scalar(100.0))]
                    .into_iter()
                    .collect(),
            ),
        );
        model
    }

    /// The legitimate layout — actuals then forecasts, in order — must pass.
    #[test]
    fn contiguous_actuals_then_forecasts_is_accepted() {
        let mut model = model_with_periods(vec![
            period(PeriodId::quarter(2024, 1), true),
            period(PeriodId::quarter(2024, 2), true),
            period(PeriodId::quarter(2024, 3), false),
            period(PeriodId::quarter(2024, 4), false),
        ]);
        model
            .validate_semantics()
            .expect("actuals-then-forecasts in order is valid");
    }

    /// An actual after a forecast anchors that forecast on a later value.
    #[test]
    fn actual_after_forecast_is_rejected() {
        let mut model = model_with_periods(vec![
            period(PeriodId::quarter(2024, 1), true),
            period(PeriodId::quarter(2024, 2), false),
            period(PeriodId::quarter(2024, 3), true),
            period(PeriodId::quarter(2024, 4), false),
        ]);
        let err = model
            .validate_semantics()
            .expect_err("an actual after a forecast is look-ahead and must be rejected");
        assert!(
            err.to_string().contains("prefix"),
            "expected the actuals-prefix diagnostic: {err}"
        );
    }

    /// Out-of-order periods are rejected regardless of actual/forecast flags.
    #[test]
    fn out_of_order_periods_are_rejected() {
        let mut model = model_with_periods(vec![
            period(PeriodId::quarter(2024, 2), true),
            period(PeriodId::quarter(2024, 1), true),
        ]);
        let err = model
            .validate_semantics()
            .expect_err("descending periods must be rejected");
        assert!(
            err.to_string().contains("increasing"),
            "expected the ordering diagnostic: {err}"
        );
    }
}
