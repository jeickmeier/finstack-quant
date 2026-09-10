//! Calibration Schema.
//!
//! Defines the JSON contract for plan-driven calibration.

// `EnvelopeError` is intentionally large (carries available-IDs lists and
// strict-load diagnostics) because the cross-binding consumers want all the
// diagnostic context in one shot. Boxing the error would harm ergonomics on a
// cold error path. Mirrors the same allowance in `api::engine` and
// `api::validate`.
#![allow(clippy::result_large_err)]

use crate::api::market_datum::MarketDatum;
use crate::api::prior_market::PriorMarketObject;
use crate::config::{CalibrationConfig, CalibrationMethod, RatesStepConventions};
use crate::hull_white::SwapFrequency;
use crate::quotes::ids::QuoteId;
use crate::CalibrationReport;
use finstack_quant_core::config::ResultsMeta;
use finstack_quant_core::contract::{
    deserialize_json_value, parse_json_value, ContractDescriptor, ContractError, Diagnostic,
    LoadLimits, LoadPhase, Severity, ValidationReport as ContractValidationReport,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::BusinessDayConvention;
use finstack_quant_core::dates::{Date, DayCount, Tenor};
use finstack_quant_core::market_data::context::{MarketContext, MarketContextState};
use finstack_quant_core::market_data::term_structures::{
    NelsonSiegelModel, NsVariant, ParInterp, Seniority,
};
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use finstack_quant_core::types::{CurveId, IndexId};
use finstack_quant_valuations::instruments::credit_derivatives::cds::CdsValuationConvention;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts_export")]
use ts_rs::TS;

/// Current schema version identifier for the calibration API.
pub const CALIBRATION_SCHEMA: &str = "finstack_quant.calibration/1";
/// Persistence contract for calibration request and result envelopes.
pub const CALIBRATION_CONTRACT: ContractDescriptor =
    ContractDescriptor::new("finstack_quant.calibration");
const FINAL_MARKET_POINTER: &str = "/result/final_market";

/// Exact schema marker accepted by calibration envelopes.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
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

fn validate_schema_marker(
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

fn append_prefixed_report(
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

fn prefix_json_pointer(prefix: &str, child: Option<&str>) -> String {
    match child {
        None | Some("") => prefix.to_string(),
        Some(pointer) if pointer.starts_with('/') => format!("{prefix}{pointer}"),
        Some(pointer) => format!("{prefix}/{}", pointer.replace('~', "~0").replace('/', "~1")),
    }
}

#[cfg(test)]
mod pointer_tests {
    use super::{prefix_json_pointer, FINAL_MARKET_POINTER};

    #[test]
    fn nested_diagnostic_pointer_join_is_rfc_6901_safe() {
        assert_eq!(
            prefix_json_pointer(FINAL_MARKET_POINTER, Some("/schema_version")),
            "/result/final_market/schema_version"
        );
        assert_eq!(
            prefix_json_pointer(FINAL_MARKET_POINTER, Some("/")),
            "/result/final_market/"
        );
        assert_eq!(
            prefix_json_pointer(FINAL_MARKET_POINTER, Some("desk/USD~CSA")),
            "/result/final_market/desk~1USD~0CSA"
        );
        assert_eq!(
            prefix_json_pointer(FINAL_MARKET_POINTER, None),
            FINAL_MARKET_POINTER
        );
    }
}

fn validate_final_market(
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

fn parse_result_value(
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
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CalibrationResult {
    /// Final calibrated market context (all curves, surfaces, scalars, etc.)
    // MarketContextState is from finstack-quant-core which does not carry ts_export yet.
    // Using `unknown` as a placeholder until finstack-quant-core adds TS support.
    #[cfg_attr(feature = "ts_export", ts(type = "unknown"))]
    pub final_market: MarketContextState,
    /// Merged plan-level calibration report.
    #[cfg_attr(feature = "ts_export", ts(type = "unknown"))]
    pub report: CalibrationReport,
    /// Per-step calibration reports keyed by step id.
    #[cfg_attr(feature = "ts_export", ts(type = "Record<string, unknown>"))]
    pub step_reports: std::collections::BTreeMap<String, CalibrationReport>,
    /// Results metadata (timestamp, version, rounding context, etc.).
    #[cfg_attr(feature = "ts_export", ts(type = "unknown"))]
    pub results_meta: ResultsMeta,
}

/// Top-level envelope for calibration results.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
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
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CalibrationEnvelope {
    /// Optional `$schema` URL/path for editor-side JSON Schema discovery.
    /// Ignored at runtime; serialized when present.
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts_export", ts(type = "string | null"))]
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
    /// Returns [`super::errors::EnvelopeError::JsonSerialize`] if
    /// serialization fails.
    pub fn to_json_pretty(&self) -> Result<String, super::errors::EnvelopeError> {
        super::validate::serialize_pretty_json(self, "CalibrationEnvelope")
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
    /// in the bounded validation report so `dry_run` can report all findings;
    /// execution rejects that report before constructing market state.
    pub fn from_slice_strict(
        bytes: &[u8],
        limits: &LoadLimits,
    ) -> Result<(Self, ContractValidationReport), ContractError> {
        let value = parse_json_value(bytes, limits)?;
        validate_schema_marker(&value, limits)?;
        let mut report = ContractValidationReport::default();
        let envelope: Self = deserialize_json_value(value, limits)?;
        super::validate::append_contract_diagnostics(
            &mut report,
            super::validate::validate(&envelope).errors,
            limits,
        );
        Ok((envelope, report))
    }
}

/// A calibration plan containing quote sets and execution steps.
///
/// A plan organizes market data into named sets and defines a sequence of
/// [`CalibrationStep`] to be executed.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
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
    #[cfg_attr(feature = "ts_export", ts(type = "Record<string, Array<string>>"))]
    pub quote_sets: IndexMap<String, Vec<QuoteId>>,
    /// Sequence of calibration steps to execute.
    pub steps: Vec<CalibrationStep>,
    /// Global settings for the calibration process.
    #[serde(default)]
    pub settings: CalibrationConfig,
}

/// A single step in the calibration process.
///
/// Each step targets the construction or update of a specific market object
/// (e.g., a yield curve) using a specified set of quotes.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
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

/// Polymorphic parameters for different calibration step types.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[cfg_attr(feature = "ts_export", ts(rename_all = "snake_case"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StepParams {
    /// Discount curve calibration.
    Discount(DiscountCurveParams),

    /// Forward curve calibration.
    Forward(ForwardCurveParams),

    /// Hazard curve calibration.
    Hazard(HazardCurveParams),

    /// Inflation curve calibration.
    Inflation(InflationCurveParams),

    /// Volatility surface calibration.
    VolSurface(VolSurfaceParams),

    /// Swaption volatility surface calibration.
    SwaptionVol(SwaptionVolParams),

    /// Base correlation calibration.
    BaseCorrelation(BaseCorrelationParams),

    /// Student-t copula degrees of freedom calibration.
    StudentT(StudentTParams),

    /// Hull-White 1-factor model calibration.
    HullWhite(HullWhiteStepParams),

    /// Hull-White 1-factor calibration to cap/floor volatility quotes.
    CapFloorHullWhite(CapFloorHullWhiteStepParams),

    /// SVI volatility surface calibration.
    SviSurface(SviSurfaceParams),

    /// Cross-currency basis curve calibration.
    XccyBasis(XccyBasisParams),

    /// Parametric (Nelson-Siegel / NSS) curve calibration.
    Parametric(ParametricCurveParams),
}

/// Primary output class used by runtime batching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StepPrimaryOutput {
    /// Curve-like object in the market context.
    Curve(CurveId),
    /// Volatility surface/cube-like object in the market context.
    Surface(CurveId),
    /// Scalar value in the market context.
    Scalar(String),
}

/// Static input/output metadata for a calibration step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StepIo {
    /// Step kind as serialized in envelope JSON.
    pub(crate) kind: &'static str,
    /// Curve / surface / scalar IDs the step reads.
    pub(crate) reads: Vec<String>,
    /// Curve / surface / scalar IDs the step writes.
    pub(crate) writes: Vec<String>,
    /// Primary output used to detect parallel-batch conflicts.
    pub(crate) primary_output: StepPrimaryOutput,
}

impl StepParams {
    /// Return the static dependency metadata for this step.
    pub(crate) fn io(&self) -> StepIo {
        match self {
            StepParams::Discount(p) => StepIo {
                kind: "discount",
                reads: Vec::new(),
                writes: vec![p.curve_id.to_string()],
                primary_output: StepPrimaryOutput::Curve(p.curve_id.clone()),
            },
            StepParams::Forward(p) => StepIo {
                kind: "forward",
                reads: vec![p.discount_curve_id.to_string()],
                writes: vec![p.curve_id.to_string()],
                primary_output: StepPrimaryOutput::Curve(p.curve_id.clone()),
            },
            StepParams::Hazard(p) => StepIo {
                kind: "hazard",
                reads: vec![p.discount_curve_id.to_string()],
                writes: vec![p.curve_id.to_string()],
                primary_output: StepPrimaryOutput::Curve(p.curve_id.clone()),
            },
            StepParams::Inflation(p) => StepIo {
                kind: "inflation",
                reads: vec![p.discount_curve_id.to_string()],
                writes: vec![p.curve_id.to_string()],
                primary_output: StepPrimaryOutput::Curve(p.curve_id.clone()),
            },
            StepParams::VolSurface(p) => StepIo {
                kind: "vol_surface",
                reads: p
                    .discount_curve_id
                    .as_ref()
                    .map(|id| vec![id.to_string()])
                    .unwrap_or_default(),
                writes: vec![p.vol_surface_id.clone()],
                primary_output: StepPrimaryOutput::Surface(CurveId::from(
                    p.vol_surface_id.as_str(),
                )),
            },
            StepParams::SwaptionVol(p) => StepIo {
                kind: "swaption_vol",
                reads: vec![p.discount_curve_id.to_string()],
                writes: vec![p.vol_surface_id.clone()],
                primary_output: StepPrimaryOutput::Surface(CurveId::from(
                    p.vol_surface_id.as_str(),
                )),
            },
            StepParams::BaseCorrelation(p) => {
                let curve_id = CurveId::from(format!("{}_CORR", p.index_id));
                StepIo {
                    kind: "base_correlation",
                    reads: vec![p.discount_curve_id.to_string()],
                    writes: vec![curve_id.to_string()],
                    primary_output: StepPrimaryOutput::Curve(curve_id),
                }
            }
            StepParams::StudentT(p) => {
                let mut reads = vec![p.base_correlation_curve_id.clone()];
                if let Some(discount_curve_id) = &p.discount_curve_id {
                    reads.push(discount_curve_id.to_string());
                }
                let scalar_key = format!("{}_STUDENT_T_DF", p.tranche_instrument_id);
                StepIo {
                    kind: "student_t",
                    reads,
                    writes: vec![scalar_key.clone()],
                    primary_output: StepPrimaryOutput::Scalar(scalar_key),
                }
            }
            StepParams::HullWhite(p) => {
                let scalar_key = format!("{}_HW1F", p.curve_id.as_str());
                StepIo {
                    kind: "hull_white",
                    reads: vec![p.curve_id.to_string()],
                    writes: vec![scalar_key.clone()],
                    primary_output: StepPrimaryOutput::Scalar(scalar_key),
                }
            }
            StepParams::CapFloorHullWhite(p) => {
                let mut reads = vec![p.discount_curve_id.to_string()];
                if p.forward_curve_id != p.discount_curve_id {
                    reads.push(p.forward_curve_id.to_string());
                }
                let scalar_key = format!("{}_CAPFLOOR_HW1F", p.discount_curve_id.as_str());
                StepIo {
                    kind: "cap_floor_hull_white",
                    reads,
                    writes: vec![scalar_key.clone()],
                    primary_output: StepPrimaryOutput::Scalar(scalar_key),
                }
            }
            StepParams::SviSurface(p) => StepIo {
                kind: "svi_surface",
                reads: p
                    .discount_curve_id
                    .as_ref()
                    .map(|id| vec![id.to_string()])
                    .unwrap_or_default(),
                writes: vec![p.vol_surface_id.clone()],
                primary_output: StepPrimaryOutput::Surface(CurveId::from(
                    p.vol_surface_id.as_str(),
                )),
            },
            StepParams::XccyBasis(p) => {
                let mut writes = vec![p.curve_id.to_string()];
                if let Some(basis_id) = &p.basis_spread_curve_id {
                    writes.push(basis_id.to_string());
                }
                StepIo {
                    kind: "xccy_basis",
                    reads: vec![p.domestic_discount_id.to_string()],
                    writes,
                    primary_output: StepPrimaryOutput::Curve(p.curve_id.clone()),
                }
            }
            StepParams::Parametric(p) => StepIo {
                kind: "parametric",
                reads: Vec::new(),
                writes: vec![p.curve_id.to_string()],
                primary_output: StepPrimaryOutput::Curve(p.curve_id.clone()),
            },
        }
    }
}

// Step Parameter Structs

/// Parameters for discount curve calibration step.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DiscountCurveParams {
    /// Identifier for the discount curve being built.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub curve_id: CurveId,
    /// Currency of the curve.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub currency: Currency,
    /// Base date for the curve.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub base_date: Date,
    /// Calibration method to use.
    #[serde(default)]
    pub method: CalibrationMethod,
    /// Interpolation style for the constructed discount curve.
    ///
    /// Defaults to log-linear discount factors, preserving positive discount
    /// factors and piecewise-constant continuously compounded forwards.
    /// `Linear` remains available only when explicitly requested.
    #[serde(default = "default_interp_log_linear")]
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub interpolation: InterpStyle,
    /// Extrapolation policy for the curve.
    #[serde(default = "default_extrap_flat")]
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub extrapolation: ExtrapolationPolicy,
    /// Optional separate ID for pricing logic (defaults to curve_id).
    #[serde(default)]
    #[cfg_attr(feature = "ts_export", ts(type = "string | null"))]
    pub pricing_discount_id: Option<CurveId>,
    /// Optional forward curve ID for pricing (if needed).
    #[serde(default)]
    #[cfg_attr(feature = "ts_export", ts(type = "string | null"))]
    pub pricing_forward_id: Option<CurveId>,

    /// Step-level conventions for pricing and curve time axis.
    #[serde(default)]
    pub conventions: RatesStepConventions,
}

/// Parameters for forward curve calibration step.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ForwardCurveParams {
    /// Identifier for the forward curve being built.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub curve_id: CurveId,
    /// Currency of the curve.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub currency: Currency,
    /// Base date for the curve.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub base_date: Date,
    /// Tenor in years for the forward curve.
    pub tenor_years: f64,
    /// Identifier for the discount curve to use.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub discount_curve_id: CurveId,
    /// Calibration method to use.
    ///
    /// Forward curves require [`CalibrationMethod::GlobalSolve`]. Sequential
    /// bootstrap is rejected because contractual off-grid reset intervals
    /// couple adjacent rates through projection discount factors.
    #[serde(default)]
    pub method: CalibrationMethod,
    /// Interpolation style for the constructed forward curve.
    ///
    /// Defaults to Hagan-West monotone-convex interpolation for a smooth,
    /// shape-preserving forward term structure. `Linear` remains explicitly
    /// opt-in.
    #[serde(default = "default_interp_monotone_convex")]
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub interpolation: InterpStyle,

    /// Step-level conventions for pricing and curve time axis.
    #[serde(default)]
    pub conventions: RatesStepConventions,
}

/// Parameters for hazard curve calibration step.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HazardCurveParams {
    /// Identifier for the hazard curve being built.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub curve_id: CurveId,
    /// Entity name.
    pub entity: String,
    /// Seniority of the debt.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub seniority: Seniority,
    /// Currency of the curve.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub currency: Currency,
    /// Base date for the curve.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub base_date: Date,
    /// Identifier for the discount curve to use.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub discount_curve_id: CurveId,
    /// Required recovery-rate assumption as a decimal fraction in `[0, 1]`.
    pub recovery_rate: f64,
    /// Notional used to price synthetic CDS instruments during calibration.
    ///
    /// Calibration normalizes residuals by notional, so this is typically left as
    /// the unit-notional default unless you have a specific reason to change it.
    #[serde(default = "default_unit_notional")]
    pub notional: f64,
    /// Calibration method to use.
    #[serde(default)]
    pub method: CalibrationMethod,
    /// Interpolation style for survival probabilities between pillars.
    ///
    /// Only log-linear survival interpolation is supported, preserving the
    /// piecewise-constant hazard representation. Other styles are rejected
    /// before calibration starts.
    #[serde(default = "default_interp_log_linear")]
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub interpolation: InterpStyle,

    /// Interpolation method for par spreads reported by the calibrated curve.
    ///
    /// Note: this is used for *quoting/interpolation of stored par spreads* and does not affect
    /// survival no-arbitrage, which is enforced via non-negative hazards together with the
    /// required log-linear survival interpolation.
    #[serde(default = "default_par_interp_linear")]
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub par_interp: ParInterp,

    /// Optional CDS documentation-clause assertion.
    ///
    /// Hazard schedule conventions come from the quote `CdsConventionKey`. When
    /// this field is set, it must be consistent with those quote-derived
    /// conventions (same clause or the matching regional family).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc_clause: Option<String>,
    /// Optional CDS valuation convention used by synthetic CDS instruments
    /// during hazard calibration and rebootstrap.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts_export", ts(type = "string | null"))]
    pub cds_valuation_convention: Option<CdsValuationConvention>,
}

/// Parameters for inflation curve calibration step.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct InflationCurveParams {
    /// Identifier for the inflation curve being built.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub curve_id: CurveId,
    /// Currency of the curve.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub currency: Currency,
    /// Valuation and calibration-instrument start date. The output curve's
    /// zero-time reference CPI date is this date minus `observation_lag`.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub base_date: Date,
    /// Identifier for the discount curve to use.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub discount_curve_id: CurveId,
    /// Reference index (e.g. "USA-CPI-U").
    pub index: String,
    /// Observation lag (e.g. "3M").
    ///
    /// Overrides the quote convention's lag and must match any supplied index
    /// lag. The same lag determines the output curve's reference-date origin and the dates
    /// of the CPI observations consumed by calibration instruments.
    pub observation_lag: String,
    /// Base CPI level used as the curve's reference CPI at t=0.
    ///
    /// This is the contractual reference CPI at the start date after applying
    /// observation lag and monthly interpolation. Its curve date is
    /// `base_date - observation_lag`. Supplied index observations must reproduce
    /// this value, including seasonality; a mismatch is rejected.
    pub base_cpi: f64,
    /// Notional used to price synthetic inflation swaps during calibration.
    ///
    /// Calibration normalizes residuals by notional, so this is typically left as
    /// the unit-notional default unless you have a specific reason to change it.
    #[serde(default = "default_unit_notional")]
    pub notional: f64,
    /// Calibration method to use.
    #[serde(default)]
    pub method: CalibrationMethod,
    /// Interpolation style for the curve.
    #[serde(default = "default_interp_linear")]
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub interpolation: InterpStyle,

    /// Optional seasonal adjustment factors for deseasonalizing CPI observations.
    ///
    /// When provided, the calibrator will:
    /// 1. Deseasonalize input CPI levels using the monthly factors
    /// 2. Fit the smooth zero-coupon curve to deseasonalized levels
    /// 3. Reseasonalize the output CPI path
    ///
    /// Monthly adjustments are additive to log CPI level. They should approximately
    /// sum to zero over 12 months.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seasonal_factors: Option<SeasonalFactors>,
}

/// Monthly seasonal adjustment factors for inflation curves.
///
/// Used to deseasonalize CPI observations before fitting a smooth
/// zero-coupon inflation curve, then reseasonalize the output.
/// Monthly adjustments should approximately sum to zero.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SeasonalFactors {
    /// Monthly adjustment factors (Jan=index 0 through Dec=index 11).
    /// These are additive adjustments to the log CPI level.
    pub monthly_adjustments: [f64; 12],
}

/// Parameters for volatility surface calibration step.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum VolSurfaceModel {
    /// Stochastic alpha-beta-rho model.
    Sabr,
}

/// Parameters for volatility surface calibration step.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VolSurfaceParams {
    /// Identifier for the volatility surface being built.
    pub vol_surface_id: String,
    /// Base date for the surface.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub base_date: Date,
    /// Identifier for the underlying instrument.
    pub underlying_ticker: String,
    /// Volatility model used for calibration.
    pub model: VolSurfaceModel,
    /// Discount curve ID.
    #[serde(default)]
    #[cfg_attr(feature = "ts_export", ts(type = "string | null"))]
    pub discount_curve_id: Option<CurveId>,
    /// SABR Beta parameter.
    #[serde(default = "default_sabr_beta")]
    pub beta: f64,
    /// Target expiries for calibration.
    #[serde(default)]
    pub target_expiries: Vec<f64>,
    /// Target strikes for calibration.
    #[serde(default)]
    pub target_strikes: Vec<f64>,
    /// Optional spot price override.
    #[serde(default)]
    pub spot_override: Option<f64>,
    /// Optional dividend yield override.
    #[serde(default)]
    pub dividend_yield_override: Option<f64>,
    /// Extrapolation policy for total-variance fill across expiries.
    ///
    /// After each quoted expiry is calibrated, the published expiry×strike
    /// grid is filled by interpolating total variance `w = σ²T` in expiry
    /// (not by interpolating SABR α/ν/ρ and evaluating Hagan at the target
    /// `T`). This policy controls targets that fall outside the calibrated
    /// expiry range: `Error` rejects them; `Clamp` holds the nearest slice's
    /// total variance flat.
    #[serde(default)]
    pub expiry_extrapolation: SurfaceExtrapolationPolicy,
}

/// Parameters for calibrating swaption volatility surfaces.
///
/// Defines the structure and conventions for building a volatility surface
/// from swaption quotes using the SABR model.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SwaptionVolParams {
    /// Identifier for the volatility surface.
    pub vol_surface_id: String,
    /// Base date for the calibration.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub base_date: Date,
    /// Discount curve identifier for pricing.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub discount_curve_id: CurveId,
    /// Optional forward curve identifier (if different from discount curve).
    #[serde(default)]
    pub forward_id: Option<String>,
    /// Currency for the swaption surface.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub currency: Currency,
    /// Volatility quoting convention (normal or lognormal).
    #[serde(default)]
    pub vol_convention: SwaptionVolConvention,
    /// SABR beta parameter (typically 0.0 for normal, 1.0 for lognormal).
    #[serde(default = "default_sabr_beta")]
    pub sabr_beta: f64,
    /// Target expiry times (in years) for the surface grid.
    #[serde(default)]
    pub target_expiries: Vec<f64>,
    /// Target tenor times (in years) for the surface grid.
    #[serde(default)]
    pub target_tenors: Vec<f64>,
    /// SABR parameter interpolation method between expiries/tenors.
    #[serde(default)]
    pub sabr_interpolation: SabrInterpolationMethod,
    /// Optional calendar identifier for date adjustments.
    #[serde(default)]
    pub calendar_id: Option<String>,
    /// Optional day count convention for fixed leg calculations.
    #[serde(default)]
    #[cfg_attr(feature = "ts_export", ts(type = "string | null"))]
    pub fixed_day_count: Option<DayCount>,

    /// Optional floating index identifier used to resolve market swap conventions.
    ///
    /// Swaption forward/par rate calculations require swap schedule conventions (fixed frequency,
    /// day count, calendar, BDC) that are now indexed off a rate index conventions registry.
    ///
    /// If omitted, individual swaption quotes must provide `float_leg_conventions.index`.
    #[serde(default)]
    #[cfg_attr(feature = "ts_export", ts(type = "string | null"))]
    pub swap_index: Option<IndexId>,
    /// Maximum absolute error of any fitted volatility quote; defaults to 0.0015.
    ///
    /// This is distinct from `plan.settings.tolerance` (solver tolerance). For swaption-vol
    /// calibration, success should reflect whether the fitted smile residuals are within a
    /// market-appropriate tolerance (e.g., 10–20 normal vol bp), not machine epsilon.
    /// Normal quotes use decimal rate per square-root year; Black quotes use
    /// dimensionless annual volatility. Errors are never scaled by vega.
    #[serde(default)]
    pub vol_tolerance: Option<f64>,

    /// Extrapolation policy used when interpolating SABR parameters across the
    /// expiry–tenor grid for target points that do not have a directly calibrated bucket.
    #[serde(default)]
    pub sabr_extrapolation: SurfaceExtrapolationPolicy,

    /// Allow deterministic fallbacks when SABR grid corners are missing during interpolation.
    ///
    /// If `false` (default), missing corner buckets are treated as an error instead of
    /// silently substituting a nearby bucket.
    #[serde(default)]
    pub allow_sabr_missing_bucket_fallback: bool,
}

/// Extrapolation policy for volatility surface construction.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[cfg_attr(feature = "ts_export", ts(rename_all = "snake_case"))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SurfaceExtrapolationPolicy {
    /// Reject out-of-bounds targets with an explicit error (vendor-matching).
    #[default]
    Error,
    /// Clamp targets to the nearest boundary (flat extrapolation).
    Clamp,
}

/// Parameters for calibrating base correlation curves.
///
/// Defines the structure for building a base correlation curve from
/// CDS tranche quotes with different detachment points.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct BaseCorrelationParams {
    /// Credit index identifier (e.g., CDX, iTraxx).
    pub index_id: String,
    /// Series number of the credit index.
    pub series: u16,
    /// Maturity of the tranches in years.
    pub maturity_years: f64,
    /// Base date for the calibration.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub base_date: Date,
    /// Discount curve identifier for pricing.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub discount_curve_id: CurveId,
    /// Currency used for synthetic tranche pricing.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub currency: Currency,
    /// Notional used to price synthetic tranches during calibration.
    ///
    /// Calibration can be expressed in upfront % terms, so this is typically left
    /// as unit-notional unless you have a specific reason to change it.
    #[serde(default = "default_unit_notional")]
    pub notional: f64,
    /// Payment frequency for synthetic tranches (e.g., quarterly).
    #[serde(default)]
    #[cfg_attr(feature = "ts_export", ts(type = "string | null"))]
    pub frequency: Option<Tenor>,
    /// Day count convention for synthetic tranche premium accrual.
    #[serde(default)]
    #[cfg_attr(feature = "ts_export", ts(type = "string | null"))]
    pub day_count: Option<DayCount>,
    /// Business day convention for synthetic tranche schedule adjustments.
    #[serde(default)]
    #[cfg_attr(feature = "ts_export", ts(type = "string | null"))]
    pub business_day_convention: Option<BusinessDayConvention>,
    /// Optional calendar identifier for schedule generation and date adjustments.
    #[serde(default)]
    pub calendar_id: Option<String>,
    /// Detachment points (as percentages) for the tranches.
    #[serde(default)]
    pub detachment_points: Vec<f64>,
    /// Whether to use IMM dates for coupon schedules.
    #[serde(default)]
    pub use_imm_dates: bool,
}

/// Parameters for Student-t copula degrees of freedom calibration.
///
/// Calibrates the `df` (degrees of freedom) parameter of a Student-t copula
/// by repricing a market tranche upfront quote and minimizing the residual
/// using Brent root-finding.
///
/// # Fields
///
/// - `tranche_instrument_id`: Identifier for the reference tranche instrument
///   used as the calibration target.
/// - `base_correlation_curve_id`: Identifier for the pre-calibrated base
///   correlation curve (must already exist in the market context).
/// - `initial_df`: Starting guess for the degrees of freedom (e.g., 5.0).
/// - `df_bounds`: Feasible domain for `df` as `(lo, hi)`, e.g., `(2.1, 50.0)`.
/// - `correlation`: Market-implied flat correlation for the tranche.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StudentTParams {
    /// Identifier for the reference tranche instrument.
    pub tranche_instrument_id: String,
    /// Identifier for the pre-calibrated base correlation curve.
    pub base_correlation_curve_id: String,
    /// Discount curve identifier used to price the tranche.
    ///
    /// When omitted, calibration falls back to the only discount curve present
    /// in the market context as a convenience default.
    #[serde(default)]
    #[cfg_attr(feature = "ts_export", ts(type = "string | null"))]
    pub discount_curve_id: Option<CurveId>,
    /// Starting guess for degrees of freedom (typically 4-10).
    #[serde(default = "default_student_t_initial_df")]
    pub initial_df: f64,
    /// Feasible domain for `df` as `(lower_bound, upper_bound)`.
    ///
    /// `df` must be > 2 for finite variance. Typical range: `(2.1, 50.0)`.
    #[serde(default = "default_student_t_df_bounds")]
    pub df_bounds: (f64, f64),
    /// Market-implied flat correlation for the tranche.
    #[serde(default = "default_student_t_correlation")]
    pub correlation: f64,
}

fn default_student_t_initial_df() -> f64 {
    5.0
}

fn default_student_t_df_bounds() -> (f64, f64) {
    (2.1, 50.0)
}

fn default_student_t_correlation() -> f64 {
    0.3
}

/// Parameters for Hull-White 1-factor model calibration step.
///
/// Calibrates κ (mean reversion) and σ (short rate volatility) by fitting
/// ATM European swaption market prices using Jamshidian decomposition.
/// Supplied strikes must match the contractual forward swap rate within
/// `1e-8` in decimal rate units (0.0001 bp); off-ATM quotes are rejected.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HullWhiteStepParams {
    /// Required positive maximum implied-quote error in quoted volatility units.
    /// Normal quotes use decimal rate volatility; Black quotes use relative volatility.
    /// This acceptance budget is independent of the numerical solver tolerance.
    pub fit_tolerance: f64,
    /// Discount curve ID (must already exist in market context).
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub curve_id: CurveId,
    /// Currency for conventions.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub currency: Currency,
    /// Base date for the calibration.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub base_date: Date,
    /// Optional initial guess for mean reversion κ.
    #[serde(default)]
    pub initial_kappa: Option<f64>,
    /// Optional initial guess for short rate vol σ.
    #[serde(default)]
    pub initial_sigma: Option<f64>,
}

/// Parameters for Hull-White 1-factor calibration to cap/floor volatility quotes.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum HullWhiteVolatilityMode {
    /// Calibrate and persist one scalar short-rate volatility.
    #[default]
    Scalar,
    /// Bootstrap a piecewise-constant short-rate volatility schedule at quote expiries.
    Piecewise,
}

/// Parameters for Hull-White 1-factor calibration to cap/floor volatility quotes.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CapFloorHullWhiteStepParams {
    /// Required positive maximum implied-quote error in quoted volatility units.
    /// Normal quotes use decimal rate volatility; Black quotes use relative volatility.
    /// This acceptance budget is independent of the numerical solver tolerance.
    pub fit_tolerance: f64,
    /// Discount curve ID (must already exist in market context).
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub discount_curve_id: CurveId,
    /// Forward/projection curve ID. If equal to `discount_curve_id`, the
    /// discount curve is used as the single-curve projection proxy.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub forward_curve_id: CurveId,
    /// Currency for conventions.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub currency: Currency,
    /// Base date for the calibration.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub base_date: Date,
    /// Optional source mean reversion κ. Required for one-quote calibration.
    #[serde(default)]
    pub fixed_kappa: Option<f64>,
    /// Optional initial guess for mean reversion κ when solving both κ and σ.
    #[serde(default)]
    pub initial_kappa: Option<f64>,
    /// Optional initial guess for short-rate volatility σ when solving both κ and σ.
    #[serde(default)]
    pub initial_sigma: Option<f64>,
    /// Payment frequency used to decompose quoted caps/floors into caplets.
    #[serde(default)]
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub payment_frequency: SwapFrequency,
    /// Scalar or expiry-bootstraped piecewise short-rate volatility calibration.
    #[serde(default)]
    pub volatility_mode: HullWhiteVolatilityMode,
}

/// Parameters for SVI volatility surface calibration step.
///
/// Fits a Stochastic Volatility Inspired (SVI) parameterization per-expiry
/// to market-implied volatilities.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct SviSurfaceParams {
    /// Identifier for the volatility surface being built.
    pub vol_surface_id: String,
    /// Base date for the surface.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub base_date: Date,
    /// Underlying instrument ticker.
    pub underlying_ticker: String,
    /// Discount curve ID (optional).
    #[serde(default)]
    #[cfg_attr(feature = "ts_export", ts(type = "string | null"))]
    pub discount_curve_id: Option<CurveId>,
    /// Target expiries for calibration.
    #[serde(default)]
    pub target_expiries: Vec<f64>,
    /// Target strikes for calibration.
    #[serde(default)]
    pub target_strikes: Vec<f64>,
    /// Optional spot price override.
    #[serde(default)]
    pub spot_override: Option<f64>,
    /// Optional continuous dividend yield; defaults to the market scalar
    /// `"<underlying_ticker>-DIVYIELD"` or zero.
    #[serde(default)]
    pub dividend_yield_override: Option<f64>,
}

/// Volatility quoting convention for swaptions.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[cfg_attr(feature = "ts_export", ts(rename_all = "snake_case"))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SwaptionVolConvention {
    /// Normal (Bachelier) absolute volatility in decimal rate units.
    ///
    /// Example: `0.0050` means 50 basis points of absolute volatility.
    Normal,
    /// Lognormal (Black) volatility in decimal units.
    ///
    /// Example: `0.20` means 20% Black volatility.
    #[default]
    Lognormal,
    /// Shifted lognormal (Black) volatility in decimal units, with an explicit shift.
    ///
    /// Example: `0.20` means 20% Black volatility.
    ShiftedLognormal {
        /// Shift amount for negative rate handling
        shift: f64,
    },
}

/// Interpolation method for SABR parameters across the expiry–tenor grid.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[cfg_attr(feature = "ts_export", ts(rename_all = "snake_case"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum SabrInterpolationMethod {
    /// Bilinear interpolation in (expiry, tenor) over SABR parameters.
    #[default]
    Bilinear,
}

fn default_interp_linear() -> InterpStyle {
    InterpStyle::Linear
}

fn default_interp_log_linear() -> InterpStyle {
    InterpStyle::LogLinear
}

fn default_interp_monotone_convex() -> InterpStyle {
    InterpStyle::MonotoneConvex
}

fn default_par_interp_linear() -> ParInterp {
    ParInterp::Linear
}

fn default_extrap_flat() -> ExtrapolationPolicy {
    ExtrapolationPolicy::FlatForward
}

fn default_unit_notional() -> f64 {
    1.0
}

fn default_sabr_beta() -> f64 {
    0.5
}

// XCCY Basis Curve

/// Parameters for cross-currency basis curve calibration step.
///
/// Derives a foreign-currency discount curve from a domestic OIS curve,
/// FX spot rate, and cross-currency basis swap or FX forward quotes.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct XccyBasisParams {
    /// Identifier for the foreign discount curve being built.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub curve_id: CurveId,
    /// Foreign currency being calibrated.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub currency: Currency,
    /// Base date for the curve.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub base_date: Date,
    /// T+0 cash FX rate (domestic per foreign), used when a quote omits `spot_fx`.
    ///
    /// Covered-interest parity from today needs the cash FX, not the screen
    /// "spot". Market spot is T+2 for most G10 pairs and T+1 for USD/CAD;
    /// convert to T+0 using ON/TN points before passing the rate here.
    /// Mixing T+2 screen spot with T+0 discounting biases long-tenor basis
    /// by roughly 1–2 bp.
    pub fx_spot: f64,
    /// Identifier for the pre-calibrated domestic discount curve.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub domestic_discount_id: CurveId,
    /// Calibration method to use.
    #[serde(default)]
    pub method: CalibrationMethod,
    /// Interpolation style for the constructed foreign discount curve.
    ///
    /// Defaults to log-linear discount factors. `Linear` remains available
    /// only when explicitly requested.
    #[serde(default = "default_interp_log_linear")]
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub interpolation: InterpStyle,
    /// Extrapolation policy for the foreign curve.
    #[serde(default = "default_extrap_flat")]
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub extrapolation: ExtrapolationPolicy,
    /// Step-level conventions for pricing and curve time axis.
    #[serde(default)]
    pub conventions: RatesStepConventions,
    /// Optional ID for the byproduct basis spread curve.
    #[serde(default)]
    #[cfg_attr(feature = "ts_export", ts(type = "string | null"))]
    pub basis_spread_curve_id: Option<CurveId>,
}

// Parametric (NS / NSS) Curve

/// Parameters for parametric curve calibration step.
///
/// Fits a Nelson-Siegel or Nelson-Siegel-Svensson yield curve model to
/// rate instrument quotes using global (Levenberg-Marquardt) optimization.
#[cfg_attr(feature = "ts_export", derive(TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ParametricCurveParams {
    /// Identifier for the single discount curve being fitted. All calibration
    /// instruments use this curve for discounting and implied projection.
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub curve_id: CurveId,
    /// Base date for the curve.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub base_date: Date,
    /// Nelson-Siegel variant (NS or NSS).
    #[cfg_attr(feature = "ts_export", ts(type = "string"))]
    pub model: NsVariant,
    /// Optional initial parameter guesses.
    #[serde(default)]
    #[cfg_attr(feature = "ts_export", ts(type = "unknown | null"))]
    pub initial_params: Option<NelsonSiegelModel>,
}

#[cfg(test)]
mod interpolation_default_tests {
    use super::*;

    #[test]
    fn production_curve_interpolation_defaults_are_shape_safe() {
        let discount: DiscountCurveParams = serde_json::from_value(serde_json::json!({
            "curve_id": "USD-OIS",
            "currency": "USD",
            "base_date": "2025-01-01"
        }))
        .expect("discount params");
        assert_eq!(discount.interpolation, InterpStyle::LogLinear);

        let forward: ForwardCurveParams = serde_json::from_value(serde_json::json!({
            "curve_id": "USD-SOFR-3M",
            "currency": "USD",
            "base_date": "2025-01-01",
            "tenor_years": 0.25,
            "discount_curve_id": "USD-OIS"
        }))
        .expect("forward params");
        assert_eq!(forward.interpolation, InterpStyle::MonotoneConvex);

        let xccy: XccyBasisParams = serde_json::from_value(serde_json::json!({
            "curve_id": "EUR-OIS",
            "currency": "EUR",
            "base_date": "2025-01-01",
            "fx_spot": 1.1,
            "domestic_discount_id": "USD-OIS"
        }))
        .expect("xccy params");
        assert_eq!(xccy.interpolation, InterpStyle::LogLinear);
    }
}
