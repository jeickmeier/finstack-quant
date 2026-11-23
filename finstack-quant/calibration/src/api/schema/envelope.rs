//! Calibration plan, envelope, and result contract types.

use super::steps::StepParams;
use crate::CalibrationReport;
use crate::api::market_datum::MarketDatum;
use crate::api::prior_market::PriorMarketObject;
use crate::config::CalibrationConfig;
use crate::quotes::QuoteId;
use finstack_quant_core::config::ResultsMeta;
use finstack_quant_core::contract::{
    ContractDescriptor, ContractError, Diagnostic, LoadLimits, LoadPhase, Severity,
    ValidationReport as ContractValidationReport, deserialize_json_value, parse_json_value,
};
use finstack_quant_core::market_data::context::{MarketContext, MarketContextState};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Current schema version identifier for the calibration API.
pub const CALIBRATION_SCHEMA: &str = "finstack_quant.calibration/1";
/// Persistence contract for calibration request and result envelopes.
pub const CALIBRATION_CONTRACT: ContractDescriptor =
    ContractDescriptor::new("finstack_quant.calibration");
pub(super) const FINAL_MARKET_POINTER: &str = "/result/final_market";

/// Exact schema marker accepted by calibration envelopes.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum CalibrationSchema {
    /// The sole supported calibration contract.
    #[serde(rename = "finstack_quant.calibration/1")]
    Calibration,
}

impl CalibrationSchema {
    /// The exact marker required by every persisted calibration envelope.
    pub const CURRENT: Self = Self::Calibration;
}

pub(super) fn validate_schema_marker(
    value: &serde_json::Value,
    limits: &LoadLimits,
) -> Result<(), ContractError> {
    match value.get("schema") {
        Some(serde_json::Value::String(schema)) => {
            CALIBRATION_CONTRACT.parse_schema_strict(Some(schema), "/schema", limits)?;
            Ok(())
        }
        Some(found) => {
            let mut report = ContractValidationReport::default();
            report.push_bounded(
                limits,
                Diagnostic::new(
                    "contract/schema-malformed",
                    LoadPhase::Version,
                    Severity::Error,
                    format!("malformed schema marker {found}; expected {CALIBRATION_SCHEMA:?}"),
                )
                .with_pointer("/schema")
                .with_contract(CALIBRATION_CONTRACT.id)
                .with_expected_version(ContractDescriptor::VERSION),
            );
            Err(ContractError::Report(Box::new(report)))
        }
        None => {
            CALIBRATION_CONTRACT.parse_schema_strict(None, "/schema", limits)?;
            Ok(())
        }
    }
}

pub(super) fn append_prefixed_report(
    target: &mut ContractValidationReport,
    source: ContractValidationReport,
    limits: &LoadLimits,
    prefix: &str,
) {
    for mut diagnostic in source.diagnostics {
        diagnostic.pointer = Some(prefix_json_pointer(prefix, diagnostic.pointer.as_deref()));
        target.push_bounded(limits, diagnostic);
    }
    target.truncated |= source.truncated;
}

pub(super) fn prefix_json_pointer(prefix: &str, child: Option<&str>) -> String {
    match child {
        None | Some("") => prefix.to_string(),
        Some(pointer) if pointer.starts_with('/') => format!("{prefix}{pointer}"),
        Some(pointer) => format!(
            "{prefix}/{}",
            super::super::validate::escape_json_pointer(pointer)
        ),
    }
}

pub(super) fn validate_final_market(
    value: &serde_json::Value,
    report: &mut ContractValidationReport,
    limits: &LoadLimits,
) {
    let Some(final_market) = value
        .get("result")
        .and_then(|result| result.get("final_market"))
    else {
        return;
    };
    let bytes = match serde_json::to_vec(final_market) {
        Ok(bytes) => bytes,
        Err(error) => {
            report.push_bounded(
                limits,
                Diagnostic::new(
                    "market-context/structure-invalid",
                    LoadPhase::Structure,
                    Severity::Error,
                    error.to_string(),
                )
                .with_pointer(FINAL_MARKET_POINTER),
            );
            return;
        }
    };
    match MarketContext::from_state_slice(&bytes, limits) {
        Ok((_market, nested_report)) => {
            append_prefixed_report(report, nested_report, limits, FINAL_MARKET_POINTER);
        }
        Err(ContractError::Report(nested_report)) => {
            append_prefixed_report(report, *nested_report, limits, FINAL_MARKET_POINTER);
        }
        Err(ContractError::LimitExceeded {
            what: "JSON depth",
            found,
            limit,
        }) => {
            report.push_bounded(
                limits,
                Diagnostic::new(
                    "contract/depth-limit",
                    LoadPhase::Parse,
                    Severity::Error,
                    format!("nested final_market JSON depth {found} exceeds limit {limit}"),
                )
                .with_pointer(FINAL_MARKET_POINTER),
            );
        }
        Err(ContractError::Core(error)) => {
            report.push_bounded(
                limits,
                Diagnostic::new(
                    "market-context/restore-failed",
                    LoadPhase::Build,
                    Severity::Error,
                    error.to_string(),
                )
                .with_pointer(FINAL_MARKET_POINTER),
            );
        }
        Err(error) => {
            report.push_bounded(
                limits,
                Diagnostic::new(
                    "market-context/load-failed",
                    LoadPhase::Build,
                    Severity::Error,
                    error.to_string(),
                )
                .with_pointer(FINAL_MARKET_POINTER),
            );
        }
    }
}

pub(super) fn parse_result_value(
    bytes: &[u8],
    limits: &LoadLimits,
) -> Result<serde_json::Value, ContractError> {
    let relaxed_depth = (*limits).with_max_depth(usize::MAX);
    let value = parse_json_value(bytes, &relaxed_depth)?;

    let mut outer = value.clone();
    if let Some(final_market) = outer
        .get_mut("result")
        .and_then(|result| result.get_mut("final_market"))
    {
        *final_market = serde_json::Value::Null;
    }
    let outer_bytes = serde_json::to_vec(&outer).map_err(|error| {
        finstack_quant_core::Error::Validation(format!(
            "failed to inspect calibration result depth: {error}"
        ))
    })?;
    parse_json_value(&outer_bytes, limits)?;
    Ok(value)
}

/// Complete calibration result with market snapshot and diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CalibrationResult {
    /// Final calibrated market context (all curves, surfaces, scalars, etc.)
    pub final_market: MarketContextState,
    /// Merged plan-level calibration report.
    pub report: CalibrationReport,
    /// Per-step calibration reports keyed by step id.
    pub step_reports: std::collections::BTreeMap<String, CalibrationReport>,
    /// Results metadata (timestamp, version, rounding context, etc.).
    pub results_meta: ResultsMeta,
}

impl CalibrationResult {
    /// Report of one calibration step.
    ///
    /// # Arguments
    ///
    /// * `step_id` - Identifier of the step, as given in the plan
    ///   (`CalibrationStep::id`); matched exactly against the keys of
    ///   [`Self::step_reports`].
    ///
    /// # Errors
    ///
    /// Returns a not-found input error naming the available step ids when no
    /// step with `step_id` was executed.
    pub fn step_report(&self, step_id: &str) -> finstack_quant_core::Result<&CalibrationReport> {
        self.step_reports.get(step_id).ok_or_else(|| {
            finstack_quant_core::InputError::NotFound {
                id: format!(
                    "calibration step '{step_id}'; available steps: {:?}",
                    self.step_reports.keys().collect::<Vec<_>>()
                ),
            }
            .into()
        })
    }
}

/// Top-level envelope for calibration results.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CalibrationResultEnvelope {
    /// Schema marker; current writers emit [`CALIBRATION_SCHEMA`].
    pub schema: CalibrationSchema,
    /// The calibration result.
    pub result: CalibrationResult,
}

impl CalibrationResultEnvelope {
    /// Create a new result envelope.
    ///
    /// # Arguments
    ///
    /// * `result` - Completed calibration output, including the final market
    ///   snapshot, reports, and result metadata.
    pub fn new(result: CalibrationResult) -> Self {
        Self {
            schema: CalibrationSchema::Calibration,
            result,
        }
    }

    /// Compute the versioned SHA-256 hash of this result envelope's canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if the result contains a non-finite number or cannot
    /// be serialized to the core canonical JSON representation.
    pub fn content_hash(&self) -> finstack_quant_core::Result<String> {
        finstack_quant_core::canonical::content_hash(self)
    }

    /// Strictly load a persisted calibration result envelope.
    ///
    /// This persistence entry point requires the exact v1 schema marker,
    /// enforces the supplied resource limits, and strictly restores the nested
    /// final market so its version and references cannot bypass validation.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Complete UTF-8 JSON encoding of a calibration result
    ///   envelope.
    /// * `limits` - Resource policy bounding input size, JSON depth, and
    ///   retained diagnostics.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError`] for malformed JSON, resource-limit failures,
    /// a missing or non-string schema marker, an unsupported schema version,
    /// an invalid result-envelope shape, or a nested final market with an
    /// invalid version, excessive depth, or unresolved restore references.
    pub fn from_slice_strict(
        bytes: &[u8],
        limits: &LoadLimits,
    ) -> Result<(Self, ContractValidationReport), ContractError> {
        let value = parse_result_value(bytes, limits)?;
        validate_schema_marker(&value, limits)?;
        let mut report = ContractValidationReport::default();
        validate_final_market(&value, &mut report, limits);
        if report.has_errors() {
            return Err(ContractError::Report(Box::new(report)));
        }
        let envelope: Self = deserialize_json_value(value, limits)?;
        Ok((envelope, report))
    }
}

/// Top-level envelope for calibration requests.
///
/// This is the outer-most structure for a calibration request. It includes
/// the schema version, the plan to execute, and the flat market-data inputs
/// (plus optional pre-built prior calibrated objects) to build upon.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CalibrationEnvelope {
    /// Optional `$schema` URL/path for editor-side JSON Schema discovery.
    /// Ignored at runtime; serialized when present.
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema_url: Option<String>,
    /// Schema marker; current writers emit [`CALIBRATION_SCHEMA`].
    pub schema: CalibrationSchema,
    /// The calibration plan containing steps and quote-set references.
    pub plan: CalibrationPlan,

    /// Flat, id-addressable market data inputs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub market_data: Vec<MarketDatum>,

    /// Pre-built calibrated objects from a prior run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prior_market: Vec<PriorMarketObject>,
}

/// A calibration step together with the observations for its named quote set.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StepWithQuotes {
    /// Typed step containing its parameters and quote-set identifier.
    pub step: CalibrationStep,
    /// Inputs to register in the named quote set, in calibration order.
    /// A later step can pass an empty list to reuse a previously registered set.
    pub quotes: Vec<MarketDatum>,
}

impl CalibrationEnvelope {
    /// Create a current-version calibration request envelope.
    ///
    /// # Arguments
    ///
    /// * `plan` - Ordered calibration plan, quote-set references, and solver
    ///   settings to execute.
    /// * `market_data` - Flat market-data inputs addressable by the plan.
    /// * `prior_market` - Previously calibrated objects available as initial
    ///   dependencies.
    #[must_use]
    pub fn new(
        plan: CalibrationPlan,
        market_data: Vec<MarketDatum>,
        prior_market: Vec<PriorMarketObject>,
    ) -> Self {
        Self {
            schema_url: None,
            schema: CalibrationSchema::Calibration,
            plan,
            market_data,
            prior_market,
        }
    }

    /// Serialize this request envelope as canonical pretty-printed JSON.
    ///
    /// # Errors
    ///
    /// Returns [`super::super::errors::EnvelopeError::JsonSerialize`] if
    /// serialization fails.
    pub fn to_json_pretty(&self) -> Result<String, super::super::errors::EnvelopeError> {
        super::super::validate::serialize_pretty_json(self, "CalibrationEnvelope")
    }

    /// Compute the versioned SHA-256 hash of this request envelope's canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if the request contains a non-finite number or cannot
    /// be serialized to the core canonical JSON representation.
    pub fn content_hash(&self) -> finstack_quant_core::Result<String> {
        finstack_quant_core::canonical::content_hash(self)
    }

    /// Strictly load a persisted calibration request envelope.
    ///
    /// The loader enforces resource limits and requires the exact v1 calibration
    /// schema marker. Typed requests also run the complete solver-free semantic
    /// validation pass before they are returned.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Complete UTF-8 JSON encoding of a calibration request
    ///   envelope in the canonical flat-market shape.
    /// * `limits` - Resource policy bounding input bytes, JSON nesting depth,
    ///   and retained diagnostics.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError`] for malformed JSON, resource-limit failures,
    /// a missing or malformed schema marker, an unsupported schema version, or
    /// an invalid calibration-envelope shape. Semantic findings are returned
    /// as contract diagnostics in the bounded validation report rather than as
    /// an error. ([`super::super::validate::dry_run`] reports them as `EnvelopeError`s.)
    pub fn from_slice_strict(
        bytes: &[u8],
        limits: &LoadLimits,
    ) -> Result<(Self, ContractValidationReport), ContractError> {
        let envelope = Self::parse_shape(bytes, limits)?;
        let mut report = ContractValidationReport::default();
        super::super::validate::append_contract_diagnostics(
            &mut report,
            super::super::validate::validate(&envelope).errors,
            limits,
        );
        Ok((envelope, report))
    }
    pub(crate) fn parse_shape(bytes: &[u8], limits: &LoadLimits) -> Result<Self, ContractError> {
        let value = parse_json_value(bytes, limits)?;
        validate_schema_marker(&value, limits)?;
        deserialize_json_value(value, limits)
    }
}

/// A calibration plan containing quote sets and execution steps.
///
/// A plan organizes market data into named sets and defines a sequence of
/// [`CalibrationStep`] to be executed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CalibrationPlan {
    /// Unique identifier for the calibration plan.
    pub id: String,
    /// Optional human-readable description of the plan's purpose.
    #[serde(default)]
    pub description: Option<String>,
    /// Named ID lists; each ID must resolve to a quote in `market_data`.
    #[serde(default)]
    pub quote_sets: IndexMap<String, Vec<QuoteId>>,
    /// Sequence of calibration steps to execute.
    pub steps: Vec<CalibrationStep>,
    /// Global settings for the calibration process.
    #[serde(default)]
    pub settings: CalibrationConfig,
}

impl CalibrationPlan {
    /// Plan identifier host builders use when the caller does not name one.
    pub const DEFAULT_ID: &'static str = "plan";
}

/// A single step in the calibration process.
///
/// Each step targets the construction or update of a specific market object
/// (e.g., a yield curve) using a specified set of quotes.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "json-schema", schemars(deny_unknown_fields))]
pub struct CalibrationStep {
    /// Unique identifier for the object being calibrated in this step.
    pub id: String,
    /// Reference to a named quote set in the parent plan.
    pub quote_set: String,
    /// Step-specific parameters and configuration.
    #[serde(flatten)]
    pub params: StepParams,
}

impl<'de> Deserialize<'de> for CalibrationStep {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct CalibrationStepWire {
            id: String,
            quote_set: String,
            #[serde(flatten)]
            params: serde_json::Map<String, serde_json::Value>,
        }

        let wire = CalibrationStepWire::deserialize(deserializer)?;
        let params = StepParams::deserialize(serde_json::Value::Object(wire.params))
            .map_err(serde::de::Error::custom)?;
        Ok(Self {
            id: wire.id,
            quote_set: wire.quote_set,
            params,
        })
    }
}
