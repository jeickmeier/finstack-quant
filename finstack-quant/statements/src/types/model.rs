//! Financial model specification types.

use crate::error::{Error, Result};
use crate::types::{ForecastSpec, NodeId, NodeSpec, NodeType};
use finstack_quant_core::contract::{
    ContractDescriptor, ContractError, Diagnostic, LoadLimits, LoadPhase, Severity,
    ValidationReport,
};
use finstack_quant_core::dates::{PeriodId, PeriodPlan};
use finstack_quant_core::wire::SchemaVersion;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::fixed_income::convertible::ConvertibleBond;
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCredit;
use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;
use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
use finstack_quant_valuations::instruments::rates::cap_floor::CapFloor;
use finstack_quant_valuations::instruments::rates::irs::InterestRateSwap;
use finstack_quant_valuations::instruments::rates::swaption::Swaption;
use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};

/// Products of one semantic-validation pass; consumed by evaluation preparation.
pub(crate) struct SemanticAnalysis {
    pub(crate) compiled: IndexMap<NodeId, finstack_quant_core::expr::Expr>,
    pub(crate) graph: crate::evaluator::DependencyGraph,
}

/// Parse each source once within this validation run.
fn parse_cached<'a>(
    parsed: &'a mut IndexMap<String, crate::dsl::ast::StmtExpr>,
    source: &str,
) -> Result<&'a crate::dsl::ast::StmtExpr> {
    match parsed.entry(source.to_owned()) {
        indexmap::map::Entry::Occupied(entry) => Ok(entry.into_mut()),
        indexmap::map::Entry::Vacant(entry) => Ok(entry.insert(crate::dsl::parse_formula(source)?)),
    }
}

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

    /// Period plan: the ordered periods (quarters, months, etc.) with the
    /// calendar context they belong to, retained once (record
    /// CORE-CONVENTIONS-014 #4).
    ///
    /// Evaluation follows this order end-to-end (dependency resolution and time-series
    /// helpers assume a single coherent timeline).
    pub periods: PeriodPlan,

    /// Map of node_id → NodeSpec
    pub nodes: IndexMap<NodeId, NodeSpec>,

    /// Identity of the source snapshot the explicit observations were taken
    /// from, for example `"FY2025-10K-original"`.
    ///
    /// Required for an evaluation with an observation cutoff. Keep a revised
    /// set of observations in a model with a different snapshot identity, so
    /// a dated evaluation can replay the information set of its day.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_snapshot: Option<String>,

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
    /// Validate changed explicit inputs and infer their types without rebuilding
    /// formulas or the DAG. Only nonempty series contribute inferred types.
    pub(crate) fn validate_explicit_values(
        &self,
    ) -> Result<IndexMap<NodeId, crate::types::NodeValueType>> {
        let periods: IndexSet<_> = self.periods.iter().map(|period| period.id).collect();
        let mut inferred_types = IndexMap::new();
        for (node_id, node) in &self.nodes {
            if node.node_type == NodeType::Calculated && node.values.is_some() {
                return Err(Error::build(format!(
                    "Calculated node '{}' cannot have explicit values — use Mixed or Value type",
                    node_id
                )));
            }
            if let Some(values) = &node.values {
                for (period_id, value) in values {
                    if !value.value().is_finite() {
                        return Err(Error::build(format!(
                            "Node '{node_id}' has a non-finite explicit value for period \
                             {period_id}; leave a missing cell out of the series"
                        )));
                    }
                    if !periods.contains(period_id) {
                        return Err(Error::build(format!(
                            "Node '{node_id}' has an explicit value for period {period_id}, \
                             which is not present in the model timeline"
                        )));
                    }
                }
            }
            for period_id in node.availability_dates.keys() {
                if !periods.contains(period_id) {
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
            if let Some(inferred) = validated_explicit_value_type(node)? {
                inferred_types.insert(node_id.clone(), inferred);
            }
        }
        Ok(inferred_types)
    }

    /// Check that the model can be evaluated at an observation cutoff.
    ///
    /// # Errors
    ///
    /// Returns a build error if the model names no source snapshot, or an
    /// explicit observation in an actual period has no availability date.
    pub(crate) fn validate_point_in_time(&self) -> Result<()> {
        if self.source_snapshot.is_none() {
            return Err(Error::build(
                "An evaluation with an observation cutoff needs an identified source snapshot: \
                 set the model's source_snapshot",
            ));
        }
        for (node_id, node) in &self.nodes {
            let Some(values) = &node.values else { continue };
            for period in self.periods.iter().filter(|period| period.is_actual) {
                if values.contains_key(&period.id)
                    && !node.availability_dates.contains_key(&period.id)
                {
                    return Err(Error::build(format!(
                        "Node '{node_id}' has no availability date for its {} observation; an \
                         evaluation with an observation cutoff needs an explicit publication \
                         date for every observation in an actual period",
                        period.id
                    )));
                }
            }
        }
        Ok(())
    }

    /// Resolve explicit reporting currency, or infer one unambiguous monetary currency.
    pub(crate) fn capital_structure_currency(
        &self,
    ) -> Result<Option<finstack_quant_core::currency::Currency>> {
        if let Some(currency) = self
            .capital_structure
            .as_ref()
            .and_then(|cs| cs.reporting_currency)
        {
            return Ok(Some(currency));
        }
        if let Some(value) = self.meta.get("currency") {
            let value = value
                .as_str()
                .ok_or_else(|| Error::build("Model currency metadata must be a currency code"))?;
            return value
                .parse()
                .map(Some)
                .map_err(|_| Error::build(format!("Invalid model currency '{value}'")));
        }
        let mut currencies = std::collections::HashSet::new();
        for node in self.nodes.values() {
            if let Some(crate::types::NodeValueType::Monetary { currency }) = node.value_type {
                currencies.insert(currency);
            }
        }
        currencies.extend(
            self.capital_structure_instrument_currencies()?
                .into_values(),
        );
        Ok(if currencies.len() == 1 {
            currencies.into_iter().next()
        } else {
            None
        })
    }

    /// Native currencies of named instruments, independent of reporting FX.
    pub(crate) fn capital_structure_instrument_currencies(
        &self,
    ) -> Result<IndexMap<String, finstack_quant_core::currency::Currency>> {
        use finstack_quant_cashflows::CashflowScheduleSource;

        let mut currencies = IndexMap::new();
        if let Some(cs) = &self.capital_structure {
            for debt in &cs.debt_instruments {
                let source: &dyn CashflowScheduleSource = match &debt.spec {
                    FinancialStatementInstrument::Bond(value) => value,
                    FinancialStatementInstrument::ConvertibleBond(value) => value,
                    FinancialStatementInstrument::RevolvingCredit(value) => value,
                    FinancialStatementInstrument::TermLoan(value) => value,
                    FinancialStatementInstrument::InterestRateSwap(value) => value,
                    FinancialStatementInstrument::CapFloor(value) => value,
                    FinancialStatementInstrument::Swaption(value) => value,
                };
                if let Some(notional) = source.notional()? {
                    currencies.insert(debt.id.clone(), notional.currency());
                }
            }
        }
        Ok(currencies)
    }

    /// Create a new model specification directly from a period list.
    ///
    /// Prefer [`ModelBuilder::new`](crate::builder::ModelBuilder::new) for user-facing model construction:
    /// the builder validates period ranges and catches stale references to
    /// undefined nodes. This direct constructor is retained for programmatic
    /// use (scenarios, template generators, tests) where callers already have
    /// a validated [`PeriodPlan`] and intend to add nodes by hand.
    ///
    /// # Arguments
    /// * `id` - Identifier used to reference the model
    /// * `periods` - Period plan whose calendar context the model retains
    #[must_use]
    pub fn new(id: impl Into<String>, periods: PeriodPlan) -> Self {
        Self {
            id: id.into(),
            periods,
            nodes: IndexMap::new(),
            source_snapshot: None,
            capital_structure: None,
            meta: IndexMap::new(),
            schema_version: SchemaVersion::CURRENT,
        }
    }

    /// Load a model from JSON text with the default [`LoadLimits`].
    ///
    /// This is [`Self::from_slice_strict`] under the shared limits (64 MiB of
    /// input, JSON nesting depth 96, 256 retained diagnostics). Rust, Python
    /// and JavaScript all ingest a model here, so a document is accepted or
    /// rejected identically from every host.
    ///
    /// # Arguments
    ///
    /// * `json` - Complete JSON encoding of a financial model in its serde
    ///   wire form (`id`, ordered `periods`, `nodes`, optional
    ///   `capital_structure` and `meta`, and the integer `schema_version: 1`).
    ///
    /// # Errors
    ///
    /// The [`Self::from_slice_strict`] errors.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_statements::types::FinancialModelSpec;
    ///
    /// let err = FinancialModelSpec::from_json(
    ///     r#"{"id": "m", "periods": {"calendar": "gregorian", "periods": []}, "nodes": {}, "schema_version": 1}"#,
    /// )
    /// .expect_err("a model needs at least one period");
    /// assert!(err.to_string().contains("at least one period"));
    /// ```
    pub fn from_json(json: &str) -> std::result::Result<Self, ContractError> {
        Self::from_slice_strict(json.as_bytes(), &LoadLimits::default())
    }

    /// Load a model from its JSON bytes: the one admission path.
    ///
    /// Decoding is bounded and exact: the input may not exceed the byte and
    /// nesting limits, an object may not repeat a key, a closed object may
    /// not carry an unknown field, and `schema_version` must be the integer
    /// `1` (not `"1"`, `1.0` or another version). The decoded model then
    /// passes [`Self::validate_semantics`], which may fill in `value_type` on
    /// nodes whose explicit values determine it, before it is returned, so a
    /// loaded model is always runnable.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Complete UTF-8 JSON encoding of a financial model.
    /// * `limits` - Resource policy bounding input size, JSON depth, and
    ///   retained diagnostics.
    ///
    /// # Errors
    ///
    /// * [`ContractError::LimitExceeded`] when the input is longer or more
    ///   deeply nested than `limits` allows.
    /// * [`ContractError::Report`] for malformed JSON, a duplicate key, an
    ///   absent or different `schema_version`, or a document that does not
    ///   match the model shape. Its first diagnostic carries the failure
    ///   code and, where one exists, the JSON pointer of the offending field.
    /// * [`ContractError::Report`] in the semantic phase when the decoded
    ///   model fails [`Self::validate_semantics`]: its diagnostic code is
    ///   `statements/<failure>` (for example `statements/build`,
    ///   `statements/node_not_found` or `statements/circular_dependency`), its
    ///   pointer the node, period plan or capital-structure field concerned,
    ///   and its message the validation error.
    pub fn from_slice_strict(
        bytes: &[u8],
        limits: &LoadLimits,
    ) -> std::result::Result<Self, ContractError> {
        // The bounded pass checks the limits, key uniqueness and the version
        // on a JSON value, which does not keep key order. Node order is part
        // of a model, so the model itself is decoded from the bytes.
        FINANCIAL_MODEL_CONTRACT.decode_versioned::<serde::de::IgnoredAny>(bytes, limits)?;
        let mut model: Self = serde_json::from_slice(bytes).map_err(|error| {
            let mut report = ValidationReport::default();
            report.push_bounded(
                limits,
                Diagnostic::new(
                    "contract/structure-invalid",
                    LoadPhase::Structure,
                    Severity::Error,
                    error.to_string(),
                )
                .with_contract(FINANCIAL_MODEL_CONTRACT.id),
            );
            ContractError::Report(Box::new(report))
        })?;
        model.validate_located().map_err(|(pointer, error)| {
            let mut diagnostic = Diagnostic::new(
                format!("statements/{}", error.code()),
                LoadPhase::Semantic,
                Severity::Error,
                error.to_string(),
            )
            .with_contract(FINANCIAL_MODEL_CONTRACT.id);
            if !pointer.is_empty() {
                diagnostic = diagnostic.with_pointer(pointer);
            }
            let mut report = ValidationReport::default();
            report.push_bounded(limits, diagnostic);
            ContractError::Report(Box::new(report))
        })?;
        Ok(model)
    }

    /// Compute the content identity of this model after validating it.
    ///
    /// The model is validated first ([`Self::validate_semantics`] on a copy,
    /// which also fills in inferred `value_type` fields), and the identity is
    /// the versioned SHA-256 of that validated model's canonical JSON. A model
    /// that is not runnable therefore has no identity, and a model hashes the
    /// same before and after it has been validated.
    ///
    /// # Errors
    ///
    /// Returns the validation error (converted to the core error, keeping its
    /// kind) when the model is not runnable, and an error if it contains a
    /// non-finite number or cannot be serialized to canonical JSON.
    pub fn content_hash(&self) -> finstack_quant_core::Result<String> {
        let mut validated = self.clone();
        validated.validate_semantics()?;
        validated.canonical_hash()
    }

    /// Hash this model's canonical JSON without validating it.
    ///
    /// For a model already in its validated form: [`Self::content_hash`]
    /// after validating, and the evaluator after filling in the value types
    /// its `prepare` inferred.
    pub(crate) fn canonical_hash(&self) -> finstack_quant_core::Result<String> {
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

    /// Validate that the plan is canonical and actuals form a prefix.
    ///
    /// The plan must be nonempty, hold one period kind, give each period its
    /// calendar bounds and be contiguous
    /// ([`PeriodPlan::validate_canonical`]). All-actual and all-forecast
    /// plans are valid.
    ///
    /// Forecasting anchors on the **last** actual period. With actuals
    /// interleaved among forecast periods, a forecast would be anchored on a
    /// value from after the period being forecast.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty plan, mixed period kinds, bounds that
    /// differ from the identifier's calendar bounds, a gap or disorder between
    /// periods, or an actual period after any forecast period.
    fn validate_period_timeline(periods: &PeriodPlan) -> Result<()> {
        periods
            .validate_canonical()
            .map_err(|error| Error::build(format!("Model timeline is not canonical: {error}")))?;

        if let Some(first_forecast) = periods.iter().position(|p| !p.is_actual)
            && let Some(stray) = periods
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

        Ok(())
    }

    /// Validate model semantics that serde alone cannot enforce.
    ///
    /// This is the one validation path for a runnable model: the builder, every
    /// JSON loader and the evaluator all run it. It requires a canonical
    /// contiguous period plan with actuals as a prefix, checks each node kind
    /// against its permitted sources, infers omitted node value types from
    /// explicit values, validates formula syntax and known monetary/scalar
    /// dimensions, resolves every formula and `where` reference, rejects
    /// same-period dependency cycles and validates the optional
    /// capital-structure waterfall. This method may populate `value_type` on
    /// nodes that have explicit values and no declared type.
    ///
    /// # Errors
    ///
    /// Returns a build error for an empty, gapped, misordered or mixed-kind
    /// period plan or one whose bounds differ from its identifiers; a model
    /// with no node and no capital-structure instrument; an invalid or
    /// reserved node ID, or one that differs from its map key; a Value node
    /// with a formula or forecast, or a Calculated node without a formula or
    /// with values or a forecast; a non-finite observation, a value outside
    /// the timeline, or mixed scalar/monetary values or currencies within a
    /// node; invalid forecasts, formulas or known dimensions; or an invalid
    /// waterfall. Returns an evaluation error for a reference to an unknown
    /// node or a same-period dependency cycle.
    pub fn validate_semantics(&mut self) -> Result<()> {
        self.analyze_semantics().map(|_| ())
    }

    /// Validate the mutable model and retain expressions and the dependency graph.
    pub(crate) fn analyze_semantics(&mut self) -> Result<SemanticAnalysis> {
        self.analyze_semantics_at(&mut SemanticLocation::Field(""))
    }

    /// Validate the model and, when it is not runnable, report the JSON
    /// pointer of the field the failure concerns with its error.
    fn validate_located(&mut self) -> std::result::Result<(), (String, Error)> {
        let mut at = SemanticLocation::Field("");
        self.analyze_semantics_at(&mut at)
            .map(|_| ())
            .map_err(|error| (at.pointer(self), error))
    }

    /// [`Self::analyze_semantics`], keeping `at` on the field being checked
    /// so a failure can be located without an allocation on the success path.
    fn analyze_semantics_at(&mut self, at: &mut SemanticLocation) -> Result<SemanticAnalysis> {
        let mut parsed = IndexMap::new();
        let mut compiled = IndexMap::new();
        *at = SemanticLocation::Field("/periods");
        if self.periods.is_empty() {
            return Err(Error::build("Model must have at least one period"));
        }
        Self::validate_period_timeline(&self.periods)?;
        *at = SemanticLocation::Field("/source_snapshot");
        if self
            .source_snapshot
            .as_ref()
            .is_some_and(|id| id.trim().is_empty())
        {
            return Err(Error::build(
                "Model source_snapshot must be a nonempty identifier when set",
            ));
        }
        *at = SemanticLocation::Field("/nodes");
        let evaluable_capital_structure = self
            .capital_structure
            .as_ref()
            .is_some_and(|cs| !cs.debt_instruments.is_empty());
        if self.nodes.is_empty() && !evaluable_capital_structure {
            return Err(Error::build(
                "Model must have at least one node or a capital structure with an instrument",
            ));
        }
        let inferred_explicit_types = self.validate_explicit_values()?;

        for (index, node_id) in self.nodes.keys().enumerate() {
            *at = SemanticLocation::Node(index);
            crate::builder::validate_node_id(node_id.as_str())?;
        }

        for (index, (node_id, node)) in self.nodes.iter().enumerate() {
            *at = SemanticLocation::Node(index);
            if node.node_id != *node_id {
                return Err(Error::build(format!(
                    "Node map key '{node_id}' does not match its embedded node_id '{}'",
                    node.node_id
                )));
            }
            if let Some(forecast) = &node.forecast {
                self.validate_forecast(node_id, forecast)?;
            }
            match node.node_type {
                NodeType::Value => {
                    if node.formula_text.is_some() {
                        return Err(Error::build(format!(
                            "Value node '{}' cannot have a formula — use Mixed or Calculated type",
                            node_id
                        )));
                    }
                    if node.forecast.is_some() {
                        return Err(Error::build(format!(
                            "Value node '{node_id}' cannot have a forecast — use Mixed type (a \
                             Value node takes explicit observations only)"
                        )));
                    }
                }
                NodeType::Calculated => {
                    if node.formula_text.is_none() {
                        return Err(Error::build(format!(
                            "Calculated node '{node_id}' needs a formula"
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

        for (node_id, node) in &mut self.nodes {
            if node.value_type.is_none() {
                node.value_type = inferred_explicit_types.get(node_id).copied();
            }
        }

        // References and cycles first: an unknown node or a cycle would
        // otherwise surface as an unresolved unit.
        for (index, (node_id, node)) in self.nodes.iter().enumerate() {
            *at = SemanticLocation::Node(index);
            for (label, source) in [
                ("formula", &node.formula_text),
                ("where clause", &node.where_text),
            ] {
                if let Some(source) = source {
                    parse_cached(&mut parsed, source).map_err(|error| {
                        Error::build(format!("Invalid {label} on node '{node_id}': {error}"))
                    })?;
                }
            }
        }
        *at = SemanticLocation::Field("/nodes");
        let graph = crate::evaluator::DependencyGraph::from_parsed(self, &parsed, &mut |index| {
            *at = SemanticLocation::Node(index);
        })?;
        *at = SemanticLocation::Field("/nodes");
        // A cycle is located at the first node of the reported loop.
        graph.detect_cycles().inspect_err(|error| {
            if let Error::CircularDependency(path) = error
                && let Some(index) = path
                    .first()
                    .and_then(|id| self.nodes.get_index_of(id.as_str()))
            {
                *at = SemanticLocation::Node(index);
            }
        })?;

        *at = SemanticLocation::Field("/capital_structure");
        let capital_structure_currency = self.capital_structure_currency()?;
        let instrument_currencies = self.capital_structure_instrument_currencies()?;
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
                *at = self
                    .nodes
                    .get_index_of(&node_id)
                    .map_or(SemanticLocation::Field("/nodes"), SemanticLocation::Node);
                let inferred = {
                    let node = self.nodes.get(&node_id).ok_or_else(|| {
                        Error::build(format!("Formula node '{node_id}' disappeared"))
                    })?;
                    let formula = node.formula_text.as_deref().ok_or_else(|| {
                        Error::build(format!("Formula node '{node_id}' has no formula"))
                    })?;
                    let ast = parse_cached(&mut parsed, formula).map_err(|error| {
                        Error::build(format!("Invalid formula on node '{node_id}': {error}"))
                    })?;
                    crate::dsl::compiler::infer_value_type(
                        ast,
                        crate::dsl::compiler::UnitContext {
                            node_types: &node_value_types,
                            capital_structure_currency,
                            instrument_currencies: &instrument_currencies,
                            provisional: true,
                        },
                    )
                    .map_err(|error| {
                        Error::build(format!("Invalid formula on node '{node_id}': {error}"))
                    })?
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

        // Every unit is now resolved or provably unresolvable. Check each
        // formula against the resolved node types, with no provisional
        // unification.
        let units = crate::dsl::compiler::UnitContext {
            node_types: &node_value_types,
            capital_structure_currency,
            instrument_currencies: &instrument_currencies,
            provisional: false,
        };
        let has_capital_structure = self.capital_structure.is_some();
        *at = SemanticLocation::Field("/capital_structure");
        // Consolidating more than one native currency needs the conversion
        // policy chosen explicitly, and `total` is the consolidated scope.
        if let Some(cs) = &self.capital_structure {
            if let Some(reserved) = cs
                .debt_instruments
                .iter()
                .find(|debt| debt.id == crate::capital_structure::CsScope::TOTAL)
            {
                return Err(Error::build(format!(
                    "capital structure: instrument id '{}' is reserved for the consolidated \
                     total (`cs.<component>.total`)",
                    reserved.id
                )));
            }
            if let Some(reporting) = cs.reporting_currency
                && cs.fx_policy.is_none()
                && let Some((id, native)) = instrument_currencies
                    .iter()
                    .find(|(_, native)| **native != reporting)
            {
                return Err(Error::build(format!(
                    "capital structure: instrument '{id}' is in {native} and the reporting \
                     currency is {reporting}, but `fx_policy` is not set. Choose `period_end` \
                     explicitly to consolidate at the period-close rate."
                )));
            }
        }
        for (index, (node_id, node)) in self.nodes.iter().enumerate() {
            *at = SemanticLocation::Node(index);
            if node.formula_text.is_none() && node.value_type.is_none() {
                return Err(Error::build(format!(
                    "Cannot resolve the units of node '{node_id}': it has no observations; \
                     declare its value type (for example with value_scalar or value_money)"
                )));
            }
            if let Some(formula) = &node.formula_text {
                let ast = parse_cached(&mut parsed, formula).map_err(|e| {
                    Error::build(format!("Invalid formula on node '{}': {}", node_id, e))
                })?;
                crate::dsl::compiler::check_capital_references(ast, units, has_capital_structure)
                    .map_err(|error| {
                    Error::build(format!("Invalid formula on node '{node_id}': {error}"))
                })?;
                let inferred =
                    crate::dsl::compiler::infer_value_type(ast, units).map_err(|error| {
                        Error::build(format!("Invalid formula on node '{node_id}': {error}"))
                    })?;
                match inferred {
                    Some(inferred) if node.value_type != Some(inferred) => {
                        return Err(Error::build(format!(
                            "Formula node '{node_id}' declares {:?} but its expression returns \
                             {:?}",
                            node.value_type, inferred
                        )));
                    }
                    Some(_) => {}
                    None => {
                        return Err(Error::build(format!(
                            "Cannot resolve the units of formula node '{node_id}' ('{formula}'): \
                             declare the value type of the node, or of the inputs it reads"
                        )));
                    }
                }
                let expression = crate::dsl::compile(ast).map_err(|e| {
                    Error::build(format!("Invalid formula on node '{}': {}", node_id, e))
                })?;
                compiled.insert(node_id.clone(), expression);
            }

            if let Some(where_text) = &node.where_text {
                let ast = parse_cached(&mut parsed, where_text).map_err(|e| {
                    Error::build(format!("Invalid where clause on node '{}': {}", node_id, e))
                })?;
                crate::dsl::compiler::check_capital_references(ast, units, has_capital_structure)
                    .map_err(|error| {
                    Error::build(format!("Invalid where clause on node '{node_id}': {error}"))
                })?;
                let boolean = crate::dsl::compiler::is_boolean(ast, units).map_err(|error| {
                    Error::build(format!("Invalid where clause on node '{node_id}': {error}"))
                })?;
                if !boolean {
                    return Err(Error::build(format!(
                        "Where clause on node '{node_id}' must be a Boolean condition (a \
                         comparison, a logical operator or is_missing)"
                    )));
                }
                let expression = crate::dsl::compile(ast).map_err(|e| {
                    Error::build(format!("Invalid where clause on node '{}': {}", node_id, e))
                })?;
                compiled.insert(NodeId::new(format!("__where__{node_id}")), expression);
            }
        }

        *at = SemanticLocation::Field("/capital_structure");
        if let Some(cs) = &self.capital_structure {
            cs.validate()?;
        }

        Ok(SemanticAnalysis { compiled, graph })
    }
}

/// The model field semantic validation is checking.
#[derive(Clone, Copy)]
enum SemanticLocation {
    /// A fixed JSON pointer; empty for the whole document.
    Field(&'static str),
    /// The node at this position of the node map.
    Node(usize),
}

impl SemanticLocation {
    /// The JSON pointer (RFC 6901) of this location in `model`.
    fn pointer(self, model: &FinancialModelSpec) -> String {
        match self {
            Self::Field(pointer) => pointer.to_string(),
            Self::Node(index) => match model.nodes.get_index(index) {
                Some((node_id, _)) => format!(
                    "/nodes/{}",
                    node_id.as_str().replace('~', "~0").replace('/', "~1")
                ),
                None => "/nodes".to_string(),
            },
        }
    }
}

/// Validate explicit series units against their optional node declaration.
///
/// # Arguments
///
/// * `node` - Node whose explicit scalar or monetary series must use one type
///   and currency and agree with any declared units. Empty series infer no type.
///
/// # Errors
///
/// Returns a validation error for mixed series units or a declaration mismatch.
pub(crate) fn validated_explicit_value_type(
    node: &NodeSpec,
) -> Result<Option<crate::types::NodeValueType>> {
    let inferred = node
        .values
        .as_ref()
        .map(|values| crate::types::infer_series_value_type(values.values()))
        .transpose()?
        .flatten();
    if let (Some(declared), Some(actual)) = (node.value_type, inferred)
        && declared != actual
    {
        return Err(Error::build(format!(
            "Node '{}' declares {declared:?} but its explicit values have type {actual:?}",
            node.node_id
        )));
    }
    Ok(inferred)
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
    /// Required to consolidate instruments of more than one currency: there
    /// is no default. `period_end` converts each period's native-currency
    /// totals on the inclusive period-close date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fx_policy: Option<CapitalFxPolicy>,

    /// Optional waterfall specification for dynamic cash flow allocation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waterfall: Option<crate::capital_structure::WaterfallSpec>,
}

impl CapitalStructureSpec {
    /// Validate the capital structure on its own, without the enclosing model.
    ///
    /// Runs [`WaterfallSpec::validate`](crate::capital_structure::WaterfallSpec::validate)
    /// on the optional waterfall, rejects swap, cap/floor and swaption PIK
    /// targets, and rejects bond, convertible, swap, cap/floor and swaption
    /// instruments when the waterfall carries a prepayment rung (`Sweep`,
    /// `MandatoryPrepayment` or `VoluntaryPrepayment`). Interest-rate swaps of
    /// either side are accepted. It does not check that
    /// referenced model nodes exist; [`FinancialModelSpec::validate_semantics`]
    /// calls this method as part of whole-model validation.
    ///
    /// # Errors
    ///
    /// Returns a build error for an invalid waterfall, a PIK election naming
    /// anything but a term loan of the structure, or a bond or derivative
    /// instrument alongside a prepayment rung.
    pub fn validate(&self) -> Result<()> {
        if let Some(waterfall) = &self.waterfall {
            waterfall.validate()?;
            // An election is applied through a term loan's own coupon
            // terms, so only a term loan of this structure can be authorized.
            if let Some(pik) = &waterfall.pik_toggle {
                for election in &pik.elections {
                    let Some(debt) = self
                        .debt_instruments
                        .iter()
                        .find(|debt| debt.id == election.instrument_id)
                    else {
                        return Err(Error::build(format!(
                            "WaterfallSpec: `pik_toggle.elections` names '{}', which is not \
                             a debt instrument of the capital structure.",
                            election.instrument_id
                        )));
                    };
                    if !matches!(debt.spec, FinancialStatementInstrument::TermLoan(_)) {
                        return Err(Error::build(format!(
                            "WaterfallSpec: `pik_toggle.elections` names '{}', which is not \
                             a term loan. A PIK election is applied through a term loan's \
                             coupon terms; no other instrument can elect PIK.",
                            election.instrument_id
                        )));
                    }
                }
            }
            let has_prepay = waterfall.priority_of_payments.iter().any(|p| {
                matches!(
                    p,
                    crate::capital_structure::PaymentPriority::Sweep
                        | crate::capital_structure::PaymentPriority::MandatoryPrepayment
                        | crate::capital_structure::PaymentPriority::VoluntaryPrepayment
                )
            });
            if has_prepay {
                for debt in &self.debt_instruments {
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
            // Each prepayment names its eligible facilities. A named facility
            // must exist, and a facility whose agreement prices a payoff (a
            // call schedule with a premium or make-whole) cannot be repaid at
            // par here.
            for (_, name, facilities) in waterfall.prepayments() {
                if let crate::capital_structure::FacilitySelection::Named(ids) = facilities {
                    for id in ids {
                        if !self.debt_instruments.iter().any(|debt| &debt.id == id) {
                            return Err(Error::build(format!(
                                "WaterfallSpec: `{name}.facilities` names '{id}', which is \
                                 not a debt instrument of the capital structure."
                            )));
                        }
                    }
                }
                for debt in &self.debt_instruments {
                    if facilities.includes(&debt.id)
                        && matches!(
                            &debt.spec,
                            FinancialStatementInstrument::TermLoan(loan)
                                if loan.call_schedule.is_some()
                        )
                    {
                        return Err(Error::build(format!(
                            "WaterfallSpec: `{name}` would repay '{}' at par, but that loan \
                             has a call schedule with its own payoff terms, which the \
                             waterfall does not price. Exclude it from `{name}.facilities`.",
                            debt.id
                        )));
                    }
                }
            }
        }
        Ok(())
    }
}

/// How capital-structure totals are converted into the reporting currency.
///
/// Period-close conversion is the only supported policy. It must still be
/// chosen explicitly before instruments of more than one currency are
/// consolidated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CapitalFxPolicy {
    /// Convert each period's native-currency totals at the rate of the
    /// inclusive period-close date.
    PeriodEnd,
}

impl std::fmt::Display for CapitalFxPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("period_end")
    }
}

impl std::str::FromStr for CapitalFxPolicy {
    type Err = Error;

    fn from_str(label: &str) -> Result<Self> {
        match label {
            "period_end" => Ok(Self::PeriodEnd),
            other => Err(Error::build(format!(
                "unknown capital-structure FX policy '{other}': the only policy is 'period_end'"
            ))),
        }
    }
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

impl FinancialModelSpec {
    /// Static checks of one node's forecast: the method's own requirements,
    /// then the ones that need the model. They apply even when explicit
    /// observations cover every forecast period.
    fn validate_forecast(&self, node_id: &NodeId, forecast: &ForecastSpec) -> Result<()> {
        use crate::types::{DistributionParams, MeanRevertingParams};

        forecast
            .validate()
            .map_err(|error| Error::build(format!("Node '{node_id}': {error}")))?;
        let peer = match forecast {
            // A growth curve covers exactly the model's forecast periods.
            ForecastSpec::CurvePct(params) => {
                let horizon = self.periods.iter().filter(|p| !p.is_actual).count();
                if params.curve.len() != horizon {
                    return Err(Error::build(format!(
                        "Node '{node_id}': the growth curve has {} rates but the model has \
                         {horizon} forecast periods",
                        params.curve.len()
                    )));
                }
                None
            }
            // Override keys are periods of this model's plan.
            ForecastSpec::Override(params) => {
                for key in params.overrides.keys() {
                    let in_plan = key
                        .parse::<PeriodId>()
                        .is_ok_and(|id| self.periods.iter().any(|period| period.id == id));
                    if !in_plan {
                        return Err(Error::build(format!(
                            "Node '{node_id}': forecast override key '{key}' is not a period of \
                             the model"
                        )));
                    }
                }
                None
            }
            ForecastSpec::Normal(DistributionParams {
                correlation_with, ..
            })
            | ForecastSpec::LogNormal(DistributionParams {
                correlation_with, ..
            })
            | ForecastSpec::MeanReverting(MeanRevertingParams {
                correlation_with, ..
            }) => correlation_with.as_deref(),
            _ => None,
        };
        // A correlation peer is another stochastic forecast of the model.
        let Some(peer) = peer else {
            return Ok(());
        };
        let stochastic_peer = peer != node_id.as_str()
            && self
                .get_node(peer)
                .and_then(|node| node.forecast.as_ref())
                .is_some_and(|peer| {
                    matches!(
                        peer,
                        ForecastSpec::Normal(_)
                            | ForecastSpec::LogNormal(_)
                            | ForecastSpec::MeanReverting(_)
                    )
                });
        if !stochastic_peer {
            return Err(Error::build(format!(
                "Node '{node_id}': correlation peer '{peer}' is not another node with a normal, \
                 log_normal or mean_reverting forecast"
            )));
        }
        // An active (non-zero) correlation mixes standardized shocks, so
        // both nodes need positive volatility and the peer chain must end.
        fn active(forecast: &ForecastSpec) -> Option<(&str, f64)> {
            let (peer, rho, std_dev) = match forecast {
                ForecastSpec::Normal(p) | ForecastSpec::LogNormal(p) => {
                    (p.correlation_with.as_deref(), p.correlation, p.std_dev)
                }
                ForecastSpec::MeanReverting(p) => {
                    (p.correlation_with.as_deref(), p.correlation, p.std_dev)
                }
                _ => return None,
            };
            peer.filter(|_| rho.is_some_and(|rho| rho != 0.0))
                .map(|peer| (peer, std_dev))
        }
        let Some((mut next, std_dev)) = active(forecast) else {
            return Ok(());
        };
        if std_dev == 0.0 {
            return Err(Error::build(format!(
                "Node '{node_id}': a non-zero correlation needs a positive 'std_dev'"
            )));
        }
        for _ in 0..self.nodes.len() {
            if next == node_id.as_str() {
                return Err(Error::build(format!(
                    "Node '{node_id}': its correlation peers form a cycle"
                )));
            }
            let peer_forecast = self.get_node(next).and_then(|node| node.forecast.as_ref());
            let std_dev = match peer_forecast {
                Some(ForecastSpec::Normal(p) | ForecastSpec::LogNormal(p)) => p.std_dev,
                Some(ForecastSpec::MeanReverting(p)) => p.std_dev,
                _ => 0.0,
            };
            if std_dev == 0.0 {
                return Err(Error::build(format!(
                    "Node '{node_id}': correlation peer '{next}' needs a positive 'std_dev' to \
                     supply shocks"
                )));
            }
            match peer_forecast.and_then(active) {
                Some((peer, _)) => next = peer,
                None => return Ok(()),
            }
        }
        // A chain longer than the node count has a cycle that does not pass
        // through this node; the nodes on it report it.
        Ok(())
    }
}

#[cfg(test)]
mod period_timeline_tests {
    use super::*;
    use crate::types::AmountOrScalar;
    use finstack_quant_core::dates::{Date, Period, PeriodId};
    use time::Month;

    fn period(id: PeriodId, is_actual: bool) -> Period {
        let range = finstack_quant_core::dates::build_periods(&format!("{id}..{id}"), None)
            .expect("single-period range");
        let mut p = range.into_periods().into_iter().next().expect("one period");
        p.is_actual = is_actual;
        p
    }

    fn model_with_periods(periods: Vec<Period>) -> FinancialModelSpec {
        let mut model = FinancialModelSpec::new(
            "timeline",
            finstack_quant_core::dates::PeriodPlan::try_new(
                finstack_quant_core::dates::PeriodCalendar::Gregorian,
                periods,
            )
            .expect("valid period plan"),
        );
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
        let rows = vec![
            period(PeriodId::quarter(2024, 2).unwrap(), true),
            period(PeriodId::quarter(2024, 1).unwrap(), true),
        ];
        let err = finstack_quant_core::dates::PeriodPlan::try_new(
            finstack_quant_core::dates::PeriodCalendar::Gregorian,
            rows,
        )
        .expect_err("descending periods must be rejected at construction");
        assert!(err.to_string().contains("ascending"), "{err}");
    }

    /// Both swap sides are supported; signed cashflows and the declared IRS
    /// kind determine receipts and payments independently of coupon signs.
    #[test]
    fn receive_swap_in_capital_structure_is_accepted() {
        use finstack_quant_valuations::instruments::PayReceive;

        let mut swap = InterestRateSwap::example().expect("example swap");
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
        model
            .validate_semantics()
            .expect("Receive swaps are supported");
    }

    /// A bond can use the default waterfall. Explicit prepayment requires an
    /// engine that can rebuild the instrument's schedule after each prepayment.
    #[test]
    fn bond_accepts_default_waterfall_and_rejects_explicit_sweep() {
        use crate::capital_structure::{EcfSweepSpec, PaymentPriority, WaterfallSpec};

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
        let default_waterfall: WaterfallSpec =
            serde_json::from_value(serde_json::json!({ "available_cash_node": "cash" }))
                .expect("waterfall with omitted payment order");
        assert_eq!(default_waterfall, WaterfallSpec::default());
        let mut capital = CapitalStructureSpec {
            debt_instruments: vec![DebtInstrumentSpec {
                id: "BOND-SWEEP".to_string(),
                spec: FinancialStatementInstrument::Bond(bond),
            }],
            meta: IndexMap::new(),
            reporting_currency: None,
            fx_policy: None,
            waterfall: Some(default_waterfall.clone()),
        };
        capital
            .validate()
            .expect("a bond can use the default scheduled-payment waterfall");
        capital.waterfall = Some(WaterfallSpec {
            priority_of_payments: vec![
                PaymentPriority::Fees,
                PaymentPriority::Interest,
                PaymentPriority::Amortization,
                PaymentPriority::Sweep,
                PaymentPriority::Equity,
            ],
            ecf_sweep: Some(EcfSweepSpec {
                cash_measure: "ebitda".into(),
                deductions: vec![
                    crate::capital_structure::EcfTerm::PaidCashInterest,
                    crate::capital_structure::EcfTerm::PaidFees,
                    crate::capital_structure::EcfTerm::PaidScheduledPrincipal,
                ],
                credits: Vec::new(),
                sweep_percentage: 0.5,
                facilities: crate::capital_structure::FacilitySelection::AllEligible,
                payment_lag_days: None,
            }),
            ..default_waterfall
        });
        model.capital_structure = Some(capital);
        let err = model
            .validate_semantics()
            .expect_err("Bond + Sweep must be a build error");
        assert!(
            err.to_string().contains("bond"),
            "expected the Bond+sweep diagnostic: {err}"
        );
    }

    /// A prepayment names facilities of the capital structure, and cannot
    /// repay at par a loan whose agreement prices its own payoff.
    #[test]
    fn prepayment_facilities_must_exist_and_be_repayable_at_par() {
        use crate::capital_structure::{
            FacilitySelection, PaymentPriority, PrepaymentSpec, WaterfallSpec,
        };

        let loan = |id: &str, callable: bool| {
            let mut loan = if callable {
                TermLoan::example_callable().expect("callable loan")
            } else {
                TermLoan::example().expect("loan")
            };
            loan.id = finstack_quant_core::types::InstrumentId::new(id);
            DebtInstrumentSpec {
                id: id.to_string(),
                spec: FinancialStatementInstrument::TermLoan(loan),
            }
        };
        let capital = |facilities: FacilitySelection| CapitalStructureSpec {
            debt_instruments: vec![loan("TL-PAR", false), loan("TL-CALL", true)],
            meta: IndexMap::new(),
            reporting_currency: None,
            fx_policy: None,
            waterfall: Some(WaterfallSpec {
                priority_of_payments: vec![
                    PaymentPriority::Fees,
                    PaymentPriority::Interest,
                    PaymentPriority::Amortization,
                    PaymentPriority::VoluntaryPrepayment,
                    PaymentPriority::Equity,
                ],
                voluntary_prepayment: Some(PrepaymentSpec {
                    amount: "prepay".into(),
                    facilities,
                }),
                ..WaterfallSpec::default()
            }),
        };

        capital(FacilitySelection::Named(vec!["TL-PAR".into()]))
            .validate()
            .expect("a loan without payoff terms is repaid at par");

        let error = capital(FacilitySelection::Named(vec!["TL-GHOST".into()]))
            .validate()
            .expect_err("unknown facility")
            .to_string();
        assert!(error.contains("'TL-GHOST'"), "{error}");
        assert!(error.contains("voluntary_prepayment"), "{error}");

        for facilities in [
            FacilitySelection::AllEligible,
            FacilitySelection::Named(vec!["TL-CALL".into()]),
        ] {
            let error = capital(facilities)
                .validate()
                .expect_err("callable loan repaid at par")
                .to_string();
            assert!(error.contains("'TL-CALL'"), "{error}");
            assert!(error.contains("call schedule"), "{error}");
        }
    }

    /// A PIK election is applied through a term loan's coupon terms, so it
    /// may only name a term loan of the structure.
    #[test]
    fn pik_elections_may_only_name_term_loans() {
        use crate::capital_structure::{PikElectionSpec, PikToggleSpec, WaterfallSpec};

        let bond = Bond::fixed(
            finstack_quant_core::types::InstrumentId::new("BOND-PIK"),
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
        let mut loan = TermLoan::example().expect("loan");
        loan.id = finstack_quant_core::types::InstrumentId::new("TL-PIK");
        let capital = |target: &str| CapitalStructureSpec {
            debt_instruments: vec![
                DebtInstrumentSpec {
                    id: "BOND-PIK".to_string(),
                    spec: FinancialStatementInstrument::Bond(bond.clone()),
                },
                DebtInstrumentSpec {
                    id: "TL-PIK".to_string(),
                    spec: FinancialStatementInstrument::TermLoan(loan.clone()),
                },
            ],
            meta: IndexMap::new(),
            reporting_currency: None,
            fx_policy: None,
            waterfall: Some(WaterfallSpec {
                pik_toggle: Some(PikToggleSpec {
                    liquidity_metric: "cash".into(),
                    threshold: 1.0,
                    elections: vec![PikElectionSpec {
                        instrument_id: target.into(),
                        pik_fraction: rust_decimal::Decimal::ONE,
                        min_coupon_periods: 0,
                        notice_days: 0,
                    }],
                }),
                ..WaterfallSpec::default()
            }),
        };
        capital("TL-PIK").validate().expect("a term loan target");
        let error = capital("TL-GHOST")
            .validate()
            .expect_err("an unknown loan")
            .to_string();
        assert!(error.contains("'TL-GHOST'"), "{error}");
        let error = capital("BOND-PIK")
            .validate()
            .expect_err("a bond target")
            .to_string();
        assert!(error.contains("'BOND-PIK'"), "{error}");
        assert!(error.contains("pik_toggle"), "{error}");
    }

    /// The supported `Pay` side must continue to pass validation.
    #[test]
    fn pay_swap_in_capital_structure_is_accepted() {
        let swap = InterestRateSwap::example().expect("example swap");
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

    /// The capital-structure checks run on the spec alone, so a host can
    /// validate a `CapitalStructureSpec` without building a model around it.
    #[test]
    fn capital_structure_spec_validates_standalone() {
        let spec: CapitalStructureSpec = serde_json::from_value(serde_json::json!({
            "waterfall": {
                "priority_of_payments": ["fees", "interest", "amortization", "equity", "sweep"],
                "available_cash_node": "cash",
            }
        }))
        .expect("shape parses");
        let err = spec.validate().expect_err("equity must rank last");
        assert!(err.to_string().contains("last entry"), "{err}");

        let mut swap = InterestRateSwap::example().expect("example swap");
        swap.side = finstack_quant_valuations::instruments::PayReceive::Receive;
        let spec = CapitalStructureSpec {
            debt_instruments: vec![DebtInstrumentSpec {
                id: "IRS-RCV".to_string(),
                spec: FinancialStatementInstrument::InterestRateSwap(swap),
            }],
            meta: IndexMap::new(),
            reporting_currency: None,
            fx_policy: None,
            waterfall: None,
        };
        spec.validate().expect("Receive swaps are supported");
    }

    /// `from_json` decodes and then validates: a document that decodes but
    /// is not runnable fails with its validation kind, and a valid one loads.
    #[test]
    fn from_json_runs_semantic_validation() {
        let err = FinancialModelSpec::from_json(
            r#"{"id": "m", "periods": {"calendar": "gregorian", "periods": []}, "nodes": {}, "schema_version": 1}"#,
        )
        .expect_err("empty timeline is rejected");
        let ContractError::Report(report) = &err else {
            panic!("expected a located diagnostic, got {err:?}");
        };
        assert_eq!(report.diagnostics[0].code, "statements/build");
        assert_eq!(report.diagnostics[0].pointer.as_deref(), Some("/periods"));
        assert!(err.to_string().contains("at least one period"), "{err}");

        let err =
            FinancialModelSpec::from_json("{\"id\": ").expect_err("malformed JSON is rejected");
        assert!(
            matches!(&err, ContractError::Report(report) if report.diagnostics[0].code == "contract/parse-error"),
            "{err:?}"
        );

        let model = model_with_periods(vec![period(
            PeriodId::quarter(2024, 1).expect("valid period fixture"),
            true,
        )]);
        let json = serde_json::to_string(&model).expect("serialize");
        let parsed = FinancialModelSpec::from_json(&json).expect("valid model parses");
        assert_eq!(parsed.nodes.len(), 1);
    }

    #[test]
    fn financial_statement_instrument_conversion_rejects_unsupported_types() {
        let unsupported = InstrumentJson::Equity(
            finstack_quant_valuations::instruments::equity::spot::Equity::example()
                .expect("example"),
        );
        let error = FinancialStatementInstrument::try_from(unsupported)
            .expect_err("equity is not a capital-structure debt instrument");
        assert!(error.to_string().contains("equity"));
    }
}
