//! Financial model specification types.

use crate::error::{Error, Result};
use crate::types::{NodeId, NodeSpec, NodeType};
use finstack_quant_core::contract::{
    deserialize_json_value, parse_json_value, ContractDescriptor, ContractError, Diagnostic,
    LoadLimits, LoadPhase, Severity, ValidationReport,
};
use finstack_quant_core::dates::Period;
use finstack_quant_core::wire::SchemaVersion;
use finstack_quant_valuations::instruments::{
    Bond, CapFloor, ConvertibleBond, InstrumentJson, InterestRateSwap, RevolvingCredit, Swaption,
    TermLoan,
};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Persistence contract for [`FinancialModelSpec`].
pub const FINANCIAL_MODEL_CONTRACT: ContractDescriptor =
    ContractDescriptor::new("finstack_quant.financial_model");

/// Top-level financial model specification.
///
/// This is the wire format for a complete financial statement model.
/// It can be serialized to/from JSON for storage and interchange.
///
/// Period order in [`FinancialModelSpec::periods`] defines the evaluation timeline:
/// engines iterate periods in this sequence when resolving dependencies and rolling
/// windows.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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

    /// Required schema version. Only version `1` is accepted.
    pub schema_version: SchemaVersion,
}

impl FinancialModelSpec {
    /// Create a [`crate::builder::ModelBuilder`] for constructing a model specification.
    ///
    /// This is the preferred entry point for staged model creation. The
    /// returned builder uses typestate to require `.periods()` before node
    /// definitions can be added.
    ///
    /// # Arguments
    ///
    /// * `id` - Stable string identifier used for lookup and serialization of this object
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
            schema_version: SchemaVersion::CURRENT,
        }
    }

    /// Load and validate a persisted financial model.
    ///
    /// This strict entry point requires `schema_version: 1` and runs
    /// [`Self::validate_semantics`] before returning the model.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Complete UTF-8 JSON encoding of a financial model.
    /// * `limits` - Resource policy bounding input size, JSON depth, and
    ///   retained diagnostics.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError`] for malformed JSON, resource-limit failures,
    /// missing or unsupported versions, invalid model shape, or semantic
    /// validation failures.
    pub fn from_slice_strict(
        bytes: &[u8],
        limits: &LoadLimits,
    ) -> std::result::Result<(Self, ValidationReport), ContractError> {
        let value = parse_json_value(bytes, limits)?;
        let version = match value.get("schema_version") {
            Some(version) => Some(deserialize_json_value::<u32>(version.clone(), limits)?),
            None => None,
        };
        FINANCIAL_MODEL_CONTRACT.resolve_strict(version, "/schema_version", limits)?;
        let mut model: Self = deserialize_json_value(value, limits)?;
        model
            .validate_semantics()
            .map_err(|error| validation_report_error(error, limits))?;
        Ok((model, ValidationReport::default()))
    }

    /// Compute the versioned SHA-256 hash of this model's canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if the model contains a non-finite number or cannot be
    /// serialized to the core canonical JSON representation.
    pub fn content_hash(&self) -> finstack_quant_core::Result<String> {
        finstack_quant_core::canonical::content_hash(self)
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
            let [prev, next] = window else { continue };
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

            for period_id in node.availability_dates.keys() {
                if !self.periods.iter().any(|period| period.id == *period_id) {
                    return Err(Error::build(format!(
                        "Node '{node_id}' has an availability date for period {period_id}, \
                         which is not present in the model timeline"
                    )));
                }
                if !node
                    .values
                    .as_ref()
                    .is_some_and(|values| values.contains_key(period_id))
                {
                    return Err(Error::build(format!(
                        "Node '{node_id}' has an availability date for period {period_id}, \
                         but no explicit value for that period"
                    )));
                }
            }
        }

        for node in self.nodes.values_mut() {
            if let Some(values) = &node.values {
                let inferred = crate::types::infer_series_value_type(values.values())?;
                match (node.value_type, inferred) {
                    (Some(declared), Some(actual)) if declared != actual => {
                        return Err(Error::build(format!(
                            "Node '{}' declares {declared:?} but its explicit values have type {actual:?}",
                            node.node_id
                        )));
                    }
                    (None, inferred) => node.value_type = inferred,
                    _ => {}
                }
            }
        }

        let capital_structure_currency = self
            .capital_structure
            .as_ref()
            .and_then(|capital_structure| capital_structure.reporting_currency)
            .or_else(|| {
                self.meta
                    .get("currency")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|currency| currency.parse().ok())
            });
        let mut node_value_types: IndexMap<NodeId, crate::types::NodeValueType> = self
            .nodes
            .iter()
            .filter_map(|(node_id, node)| {
                node.value_type
                    .map(|value_type| (node_id.clone(), value_type))
            })
            .collect();
        let mut unresolved: Vec<NodeId> = self
            .nodes
            .iter()
            .filter(|(_, node)| node.formula_text.is_some() && node.value_type.is_none())
            .map(|(node_id, _)| node_id.clone())
            .collect();
        loop {
            let mut next = Vec::new();
            let mut progress = false;
            for node_id in unresolved {
                let inferred = {
                    let node = self.nodes.get(&node_id).ok_or_else(|| {
                        Error::build(format!("Formula node '{node_id}' disappeared"))
                    })?;
                    let formula = node.formula_text.as_deref().ok_or_else(|| {
                        Error::build(format!("Formula node '{node_id}' has no formula"))
                    })?;
                    let ast = crate::dsl::parse_formula(formula).map_err(|error| {
                        Error::build(format!("Invalid formula on node '{node_id}': {error}"))
                    })?;
                    crate::dsl::compiler::infer_value_type(
                        &ast,
                        &node_value_types,
                        capital_structure_currency,
                    )?
                };
                if let Some(value_type) = inferred {
                    self.nodes
                        .get_mut(&node_id)
                        .ok_or_else(|| {
                            Error::build(format!("Formula node '{node_id}' disappeared"))
                        })?
                        .value_type = Some(value_type);
                    node_value_types.insert(node_id, value_type);
                    progress = true;
                } else {
                    next.push(node_id);
                }
            }
            if next.is_empty() {
                break;
            }
            if !progress {
                // Unknown registry references, same-period cycles, and
                // dimension families not represented by NodeValueType remain
                // unresolved here. The dependency graph diagnoses references
                // and cycles later; callers may use `node_value_type` for
                // intentionally unrepresentable outputs such as variance.
                break;
            }
            unresolved = next;
        }

        for (node_id, node) in &self.nodes {
            if let Some(formula) = &node.formula_text {
                let ast = crate::dsl::parse_formula(formula).map_err(|e| {
                    Error::build(format!("Invalid formula on node '{}': {}", node_id, e))
                })?;
                if let Some(inferred) = crate::dsl::compiler::infer_value_type(
                    &ast,
                    &node_value_types,
                    capital_structure_currency,
                )
                .map_err(|error| {
                    Error::build(format!("Invalid formula on node '{node_id}': {error}"))
                })? {
                    if node.value_type != Some(inferred) {
                        return Err(Error::build(format!(
                            "Formula node '{node_id}' declares {:?} but its expression returns \
                             {:?}",
                            node.value_type, inferred
                        )));
                    }
                }
                crate::dsl::compile(&ast).map_err(|e| {
                    Error::build(format!("Invalid formula on node '{}': {}", node_id, e))
                })?;
            }

            if let Some(where_text) = &node.where_text {
                let ast = crate::dsl::parse_formula(where_text).map_err(|e| {
                    Error::build(format!("Invalid where clause on node '{}': {}", node_id, e))
                })?;
                let where_type = crate::dsl::compiler::infer_value_type(
                    &ast,
                    &node_value_types,
                    capital_structure_currency,
                )
                .map_err(|error| {
                    Error::build(format!("Invalid where clause on node '{node_id}': {error}"))
                })?;
                if where_type != Some(crate::types::NodeValueType::Scalar) {
                    return Err(Error::build(format!(
                        "Where clause on node '{node_id}' must return a scalar condition"
                    )));
                }
                crate::dsl::compile(&ast).map_err(|e| {
                    Error::build(format!("Invalid where clause on node '{}': {}", node_id, e))
                })?;
            }
        }

        if let Some(cs) = &self.capital_structure {
            if let Some(waterfall) = &cs.waterfall {
                waterfall.validate()?;
                let has_prepay = waterfall.priority_of_payments.iter().any(|p| {
                    matches!(
                        p,
                        crate::capital_structure::PaymentPriority::Sweep
                            | crate::capital_structure::PaymentPriority::MandatoryPrepayment
                            | crate::capital_structure::PaymentPriority::VoluntaryPrepayment
                    )
                });
                if has_prepay {
                    for debt in &cs.debt_instruments {
                        match &debt.spec {
                            FinancialStatementInstrument::Bond(_)
                            | FinancialStatementInstrument::ConvertibleBond(_) => {
                                return Err(Error::build(format!(
                                    "WaterfallSpec: instrument '{}' is a bond; this waterfall \
                                     is a loan/revolver engine and rejects Bond or \
                                     ConvertibleBond targets when a prepayment rung \
                                     (`Sweep`, `MandatoryPrepayment`, or \
                                     `VoluntaryPrepayment`) is present. Bond coupons stay on \
                                     original face.",
                                    debt.id
                                )));
                            }
                            FinancialStatementInstrument::InterestRateSwap(_)
                            | FinancialStatementInstrument::CapFloor(_)
                            | FinancialStatementInstrument::Swaption(_) => {
                                return Err(Error::build(format!(
                                    "WaterfallSpec: instrument '{}' is not a sweep target; \
                                     swaps and options cannot appear with a prepayment rung.",
                                    debt.id
                                )));
                            }
                            FinancialStatementInstrument::TermLoan(_)
                            | FinancialStatementInstrument::RevolvingCredit(_) => {}
                        }
                    }
                }
            }
            // Period-flow classification infers expense vs income for two-leg
            // instruments from the sign of the net flow, which assumes the
            // issuer pays fixed (`PayReceive::Pay`). A `Receive` swap would
            // silently invert that classification, so reject it loudly until
            // the sign convention is threaded through (INVARIANTS.md §3).
            for debt in &cs.debt_instruments {
                if let FinancialStatementInstrument::InterestRateSwap(swap) = &debt.spec {
                    if swap.side == finstack_quant_valuations::instruments::PayReceive::Receive {
                        return Err(Error::build(format!(
                            "Interest rate swap '{}' has side `Receive`, which the \
                             capital-structure flow classification does not support: \
                             two-leg expense/income signs assume the issuer pays fixed \
                             (`Pay`). Model the position as a `Pay` swap with inverted \
                             legs instead.",
                            debt.id
                        )));
                    }
                }
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

fn validation_report_error(error: Error, limits: &LoadLimits) -> ContractError {
    let mut report = ValidationReport::default();
    report.push_bounded(
        limits,
        Diagnostic::new(
            "contract/semantic-invalid",
            LoadPhase::Semantic,
            Severity::Error,
            error.to_string(),
        )
        .with_contract(FINANCIAL_MODEL_CONTRACT.id),
    );
    ContractError::Report(Box::new(report))
}

/// Capital structure specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CapitalStructureSpec {
    /// Debt instruments (bonds, loans, swaps)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub debt_instruments: Vec<DebtInstrumentSpec>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub meta: IndexMap<String, serde_json::Value>,

    /// Optional reporting currency override for capital structure totals
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reporting_currency: Option<finstack_quant_core::currency::Currency>,

    /// Optional FX conversion policy override.
    ///
    /// When omitted, `cs.*` cash items and balances convert on the inclusive
    /// period-end date (`FxConversionPolicy::PeriodEnd`). Conversion applies to
    /// the already-aggregated period bucket, not per contractual cashflow date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fx_policy: Option<finstack_quant_core::money::fx::FxConversionPolicy>,

    /// Optional waterfall specification for dynamic cash flow allocation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waterfall: Option<crate::capital_structure::WaterfallSpec>,
}

/// Instruments supported by company financial statement capital structures.
///
/// This intentionally smaller union keeps the financial statement schema
/// focused on debt and its common interest-rate hedges. The payloads are
/// converted to the canonical valuations registry only when a model is
/// evaluated.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(
    rename_all = "snake_case",
    tag = "type",
    content = "spec",
    deny_unknown_fields
)]
#[non_exhaustive]
pub enum FinancialStatementInstrument {
    /// Fixed- or floating-rate corporate or government bond.
    Bond(Bond),
    /// Convertible bond with an equity conversion feature.
    ConvertibleBond(ConvertibleBond),
    /// Revolving credit facility.
    RevolvingCredit(RevolvingCredit),
    /// Bilateral or institutional term loan.
    TermLoan(TermLoan),
    /// Plain-vanilla interest-rate swap used to hedge financing exposure.
    InterestRateSwap(InterestRateSwap),
    /// Interest-rate cap, floor, or collar.
    CapFloor(CapFloor),
    /// Option on an interest-rate swap.
    Swaption(Swaption),
}

impl From<FinancialStatementInstrument> for InstrumentJson {
    fn from(instrument: FinancialStatementInstrument) -> Self {
        match instrument {
            FinancialStatementInstrument::Bond(value) => Self::Bond(value),
            FinancialStatementInstrument::ConvertibleBond(value) => Self::ConvertibleBond(value),
            FinancialStatementInstrument::RevolvingCredit(value) => Self::RevolvingCredit(value),
            FinancialStatementInstrument::TermLoan(value) => Self::TermLoan(value),
            FinancialStatementInstrument::InterestRateSwap(value) => Self::InterestRateSwap(value),
            FinancialStatementInstrument::CapFloor(value) => Self::CapFloor(value),
            FinancialStatementInstrument::Swaption(value) => Self::Swaption(value),
        }
    }
}

impl TryFrom<InstrumentJson> for FinancialStatementInstrument {
    type Error = crate::Error;

    fn try_from(instrument: InstrumentJson) -> std::result::Result<Self, Self::Error> {
        match instrument {
            InstrumentJson::Bond(value) => Ok(Self::Bond(value)),
            InstrumentJson::ConvertibleBond(value) => Ok(Self::ConvertibleBond(value)),
            InstrumentJson::RevolvingCredit(value) => Ok(Self::RevolvingCredit(value)),
            InstrumentJson::TermLoan(value) => Ok(Self::TermLoan(value)),
            InstrumentJson::InterestRateSwap(value) => Ok(Self::InterestRateSwap(value)),
            InstrumentJson::CapFloor(value) => Ok(Self::CapFloor(value)),
            InstrumentJson::Swaption(value) => Ok(Self::Swaption(value)),
            unsupported => Err(crate::Error::invalid_input(format!(
                "instrument type '{}' is not supported in a financial statement capital structure",
                unsupported.type_tag()
            ))),
        }
    }
}

/// Debt instrument specification.
///
/// An identifier paired with a supported financial-statement instrument.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DebtInstrumentSpec {
    /// Instrument identifier (key within the capital structure).
    pub id: String,
    /// Tagged instrument payload: `{"type": "...", "spec": {...}}`.
    pub spec: FinancialStatementInstrument,
}

#[cfg(test)]
mod period_timeline_tests {
    use super::*;
    use crate::types::AmountOrScalar;
    use finstack_quant_core::dates::{Date, PeriodId};
    use time::Month;

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
            period(
                PeriodId::quarter(2024, 1).expect("valid period fixture"),
                true,
            ),
            period(
                PeriodId::quarter(2024, 2).expect("valid period fixture"),
                true,
            ),
            period(
                PeriodId::quarter(2024, 3).expect("valid period fixture"),
                false,
            ),
            period(
                PeriodId::quarter(2024, 4).expect("valid period fixture"),
                false,
            ),
        ]);
        model
            .validate_semantics()
            .expect("actuals-then-forecasts in order is valid");
    }

    /// An actual after a forecast anchors that forecast on a later value.
    #[test]
    fn actual_after_forecast_is_rejected() {
        let mut model = model_with_periods(vec![
            period(
                PeriodId::quarter(2024, 1).expect("valid period fixture"),
                true,
            ),
            period(
                PeriodId::quarter(2024, 2).expect("valid period fixture"),
                false,
            ),
            period(
                PeriodId::quarter(2024, 3).expect("valid period fixture"),
                true,
            ),
            period(
                PeriodId::quarter(2024, 4).expect("valid period fixture"),
                false,
            ),
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
            period(
                PeriodId::quarter(2024, 2).expect("valid period fixture"),
                true,
            ),
            period(
                PeriodId::quarter(2024, 1).expect("valid period fixture"),
                true,
            ),
        ]);
        let err = model
            .validate_semantics()
            .expect_err("descending periods must be rejected");
        assert!(
            err.to_string().contains("increasing"),
            "expected the ordering diagnostic: {err}"
        );
    }

    /// A `Receive` swap inverts the expense/income classification the flow
    /// engine infers from net-flow signs, so it must be rejected at build.
    #[test]
    fn receive_swap_in_capital_structure_is_rejected() {
        use finstack_quant_valuations::instruments::PayReceive;

        let mut swap = InterestRateSwap::example_standard().expect("example swap");
        swap.side = PayReceive::Receive;
        let mut model = model_with_periods(vec![period(
            PeriodId::quarter(2024, 1).expect("valid period fixture"),
            true,
        )]);
        model.capital_structure = Some(CapitalStructureSpec {
            debt_instruments: vec![DebtInstrumentSpec {
                id: "IRS-RCV".to_string(),
                spec: FinancialStatementInstrument::InterestRateSwap(swap),
            }],
            meta: IndexMap::new(),
            reporting_currency: None,
            fx_policy: None,
            waterfall: None,
        });
        let err = model
            .validate_semantics()
            .expect_err("a Receive swap must be rejected");
        assert!(
            err.to_string().contains("Receive"),
            "expected the Receive-side diagnostic: {err}"
        );
    }

    /// A bond plus a prepayment rung is rejected: this engine rebuilds loan
    /// schedules, and bond coupons stay on original face.
    #[test]
    fn bond_plus_sweep_in_priority_of_payments_is_rejected() {
        let bond = Bond::fixed(
            finstack_quant_core::types::InstrumentId::new("BOND-SWEEP"),
            finstack_quant_core::money::Money::from((
                1_000_000_i64,
                finstack_quant_core::currency::Currency::USD,
            )),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            Date::from_calendar_date(2025, Month::January, 1).expect("valid date"),
            Date::from_calendar_date(2030, Month::January, 1).expect("valid date"),
            finstack_quant_core::dates::StubKind::ShortFront,
            finstack_quant_core::types::CurveId::new("USD-OIS"),
        )
        .expect("bond");
        let mut model = model_with_periods(vec![period(
            PeriodId::quarter(2025, 1).expect("valid period fixture"),
            true,
        )]);
        model.capital_structure = Some(CapitalStructureSpec {
            debt_instruments: vec![DebtInstrumentSpec {
                id: "BOND-SWEEP".to_string(),
                spec: FinancialStatementInstrument::Bond(bond),
            }],
            meta: IndexMap::new(),
            reporting_currency: None,
            fx_policy: None,
            waterfall: Some(crate::capital_structure::WaterfallSpec {
                priority_of_payments: crate::capital_structure::default_priority_of_payments(),
                available_cash_node: "cash".into(),
                ecf_sweep: Some(crate::capital_structure::EcfSweepSpec {
                    ebitda_node: "ebitda".into(),
                    taxes_node: None,
                    capex_node: None,
                    working_capital_node: None,
                    cash_interest_node: None,
                    sweep_percentage: 0.5,
                    target_instrument_id: None,
                }),
                pik_toggle: None,
                ..Default::default()
            }),
        });
        let err = model
            .validate_semantics()
            .expect_err("Bond + Sweep must be a build error");
        assert!(
            err.to_string().contains("bond"),
            "expected the Bond+sweep diagnostic: {err}"
        );
    }

    /// The supported `Pay` side must continue to pass validation.
    #[test]
    fn pay_swap_in_capital_structure_is_accepted() {
        let swap = InterestRateSwap::example_standard().expect("example swap");
        let mut model = model_with_periods(vec![period(
            PeriodId::quarter(2024, 1).expect("valid period fixture"),
            true,
        )]);
        model.capital_structure = Some(CapitalStructureSpec {
            debt_instruments: vec![DebtInstrumentSpec {
                id: "IRS-PAY".to_string(),
                spec: FinancialStatementInstrument::InterestRateSwap(swap),
            }],
            meta: IndexMap::new(),
            reporting_currency: None,
            fx_policy: None,
            waterfall: None,
        });
        model
            .validate_semantics()
            .expect("a Pay swap is supported and must pass");
    }

    #[test]
    fn financial_statement_instrument_conversion_rejects_unsupported_types() {
        let unsupported =
            InstrumentJson::Equity(finstack_quant_valuations::instruments::Equity::example());
        let error = FinancialStatementInstrument::try_from(unsupported)
            .expect_err("equity is not a capital-structure debt instrument");
        assert!(error.to_string().contains("equity"));
    }
}
