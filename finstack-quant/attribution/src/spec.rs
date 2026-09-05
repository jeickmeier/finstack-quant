//! JSON specification and execution framework for attribution.
//!
//! Provides serializable specs for defining complete attribution runs in JSON,
//! with stable schemas and deterministic round-trip serialization.

use super::{AttributionMethod, CreditFactorDetailOptions, ExecutionPolicy, PnlAttribution};
use finstack_quant_core::{
    config::{FinstackConfig, ResultsMeta},
    currency::Currency,
    dates::Date,
    market_data::context::MarketContextState,
    Result,
};
use finstack_quant_models::factor::credit::hierarchy::CreditFactorModel;
use finstack_quant_valuations::instruments::model_params::ModelParamsSnapshot;
use finstack_quant_valuations::instruments::{InstrumentEnvelope, InstrumentJson};
use finstack_quant_valuations::metrics::MetricId;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Schema marker for attribution serialization.
pub const ATTRIBUTION_SCHEMA: &str = "finstack_quant.attribution/1";

/// Exact schema marker accepted by attribution envelopes.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum AttributionSchema {
    /// The sole supported attribution contract.
    #[serde(rename = "finstack_quant.attribution/1")]
    Attribution,
}

impl AttributionSchema {
    /// The exact marker required by every persisted attribution envelope.
    pub const CURRENT: Self = Self::Attribution;
}

/// Top-level envelope for attribution specifications.
///
/// Mirrors the calibration and instrument envelope patterns with schema versioning
/// and strict field validation for long-term JSON stability.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AttributionEnvelope {
    /// Schema version identifier (currently "finstack_quant.attribution/1")
    pub schema: AttributionSchema,
    /// The attribution specification
    pub attribution: AttributionSpec,
}

impl AttributionEnvelope {
    /// Create a new attribution envelope with the current schema version.
    ///
    /// # Arguments
    ///
    /// * `attribution` - Complete single-run attribution specification to wrap
    ///   in the current persistence envelope.
    pub fn new(attribution: AttributionSpec) -> Self {
        Self {
            schema: AttributionSchema::CURRENT,
            attribution,
        }
    }

    /// Execute the attribution and return the result envelope.
    ///
    /// # Errors
    ///
    /// Propagates all instrument, market-data, pricing, and method-specific
    /// errors from [`AttributionSpec::execute`]. Unsupported schema markers
    /// are rejected during deserialization.
    pub fn execute(&self) -> Result<AttributionResultEnvelope> {
        let result = self.attribution.execute()?;
        Ok(AttributionResultEnvelope::new(result))
    }

    /// Execute with panic containment; see [`AttributionSpec::execute_contained`].
    ///
    /// # Errors
    ///
    /// Everything [`Self::execute`] returns, plus
    /// [`finstack_quant_core::Error::Internal`] for a contained panic.
    pub fn execute_contained(&self) -> Result<AttributionResultEnvelope> {
        let result = self.attribution.execute_contained()?;
        Ok(AttributionResultEnvelope::new(result))
    }
}

/// Attribution specification for a single P&L attribution run.
///
/// Contains all data needed to perform attribution: instrument, market snapshots,
/// dates, and methodology.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AttributionSpec {
    /// Instrument to attribute (as JSON envelope)
    pub instrument: InstrumentJson,
    /// Market context at T₀
    pub market_t0: MarketContextState,
    /// Market context at T₁
    pub market_t1: MarketContextState,
    /// Valuation date at T₀
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub as_of_t0: Date,
    /// Valuation date at T₁
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub as_of_t1: Date,
    /// Attribution methodology
    pub method: AttributionMethod,
    /// Optional model parameters at T₀ (for attributing parameter changes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_params_t0: Option<ModelParamsSnapshot>,
    /// Optional configuration overrides (defaults to FinstackConfig::default())
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<AttributionConfig>,
    /// Optional calibrated credit factor model. When present (and the
    /// instrument has a recognizable issuer + credit-curve exposure), the
    /// returned `PnlAttribution` carries a `credit_factor_detail` field with
    /// generic / per-level / adder P&L additively decomposing
    /// `credit_curves_pnl`. Parallel and waterfall populate the detail via
    /// the reprice cascade; metrics-based and Taylor back-solve it after
    /// the linear decomposition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credit_factor_model: Option<Box<CreditFactorModel>>,
    /// Detail/payload options for `credit_factor_detail`. Inert when
    /// `credit_factor_model` is `None`.
    #[serde(default)]
    pub credit_factor_detail_options: CreditFactorDetailOptions,
    /// Option to compute all 36 cross-factor pairs when enabled
    #[serde(default)]
    pub full_cross_attribution: bool,
}

/// Default for [`AttributionConfig::strict_validation`] on the spec/execution
/// path used for official reports: factor pricing errors propagate instead of
/// being zeroed into residual.
pub(crate) const DEFAULT_STRICT_VALIDATION: bool = true;

/// Optional configuration for attribution runs.
///
/// Allows overriding default tolerances and metrics for attribution calculations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AttributionConfig {
    /// Absolute tolerance for residual validation (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tolerance_abs: Option<f64>,
    /// Percentage tolerance for residual validation (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tolerance_pct: Option<f64>,
    /// Metrics to compute for metrics-based attribution (optional)
    /// If not provided, a default set will be used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<Vec<String>>,
    /// Strict validation mode. When omitted, official spec/execution reports
    /// default to `true` so factor pricing errors fail closed instead of
    /// being zeroed into residual. Set to `false` only for diagnostic runs
    /// that must complete with a soft-failed factor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict_validation: Option<bool>,
    /// Rounding scale override (number of decimal places)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rounding_scale: Option<u32>,
    /// Rate bump size in basis points for sensitivities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_bump_bp: Option<f64>,
    /// Optional reporting currency for the attribution output.
    ///
    /// When supplied and different from the instrument's native pricing
    /// currency, the per-instrument attribution is computed in native
    /// currency and then translated to `target_currency` via
    /// [`crate::translate_to_target_currency`]. The translation:
    ///
    /// - converts every aggregate factor amount at `market_t1`'s FX,
    /// - emits a new `fx_translation_pnl` field that captures the FX move
    ///   applied to the opening position (`val_t0 × ΔFX`),
    /// - stamps `meta.fx_policy.target_currency` so downstream consumers know the
    ///   report is in a non-native currency.
    ///
    /// When `None` (the default), the attribution stays in
    /// `val_t1.currency()`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_currency: Option<Currency>,
    /// Controls whether attribution's per-factor repricings run in parallel.
    ///
    /// Defaults to [`ExecutionPolicy::Serial`] when omitted. Opt into
    /// [`ExecutionPolicy::Parallel`] only when the caller is not already
    /// parallelizing an outer portfolio or batch loop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_policy: Option<ExecutionPolicy>,
}

/// JSON fragments used by host bindings to construct an [`AttributionSpec`].
///
/// Every field is a serialized payload or an ISO date string. Optional
/// fragments that are `None` are omitted from the spec (no model-parameter
/// snapshot, no credit-factor model).
#[derive(Debug, Clone, Copy)]
pub struct AttributionJsonInputs<'a> {
    /// Canonical v1 instrument envelope JSON.
    pub instrument_json: &'a str,
    /// Canonical market-context JSON at T₀.
    pub market_t0_json: &'a str,
    /// Canonical market-context JSON at T₁.
    pub market_t1_json: &'a str,
    /// ISO-8601 valuation date for `market_t0_json`.
    pub as_of_t0: &'a str,
    /// ISO-8601 valuation date for `market_t1_json`.
    pub as_of_t1: &'a str,
    /// Snake-case serialized [`AttributionMethod`].
    pub method_json: &'a str,
    /// Optional complete serialized [`AttributionConfig`].
    pub config_json: Option<&'a str>,
    /// Optional serialized T₀ [`finstack_quant_valuations::instruments::model_params::ModelParamsSnapshot`].
    pub model_params_t0_json: Option<&'a str>,
    /// Optional serialized [`CreditFactorModel`].
    pub credit_factor_model_json: Option<&'a str>,
    /// When true, parallel attribution evaluates every pairwise cross term.
    pub full_cross_attribution: bool,
}

impl AttributionSpec {
    /// Build an attribution spec from the JSON-friendly inputs used by bindings.
    ///
    /// `inputs.as_of_t0` and `inputs.as_of_t1` must use ISO-8601 calendar-date
    /// syntax. When present, `inputs.config_json` supplies the complete
    /// serialized attribution configuration; it is not merged with caller state.
    ///
    /// # Arguments
    ///
    /// * `inputs` - Instrument, market, date, method, and optional config,
    ///   model-parameter, credit-factor-model, and full-cross fragments.
    ///   Dates must be ISO-8601 calendar dates. JSON fragments must match
    ///   the crate's serde schemas (`deny_unknown_fields`).
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when any JSON payload
    /// has the wrong schema or either as-of date cannot be parsed.
    pub fn from_json_inputs(inputs: AttributionJsonInputs<'_>) -> Result<Self> {
        let instrument_envelope: InstrumentEnvelope =
            parse_input_json("instrument envelope", inputs.instrument_json)?;
        Ok(Self {
            instrument: instrument_envelope.instrument,
            market_t0: parse_input_json("market_t0", inputs.market_t0_json)?,
            market_t1: parse_input_json("market_t1", inputs.market_t1_json)?,
            as_of_t0: parse_iso_date("as_of_t0", inputs.as_of_t0)?,
            as_of_t1: parse_iso_date("as_of_t1", inputs.as_of_t1)?,
            method: parse_input_json("method", inputs.method_json)?,
            model_params_t0: inputs
                .model_params_t0_json
                .map(|json| parse_input_json("model_params_t0", json))
                .transpose()?,
            config: inputs
                .config_json
                .map(|json| parse_input_json("config", json))
                .transpose()?,
            credit_factor_model: inputs
                .credit_factor_model_json
                .map(|json| parse_input_json("credit_factor_model", json))
                .transpose()?
                .map(Box::new),
            credit_factor_detail_options: CreditFactorDetailOptions::default(),
            full_cross_attribution: inputs.full_cross_attribution,
        })
    }
}

fn parse_input_json<T: DeserializeOwned>(label: &str, json: &str) -> Result<T> {
    serde_json::from_str(json).map_err(|e| {
        finstack_quant_core::Error::Validation(format!("invalid attribution {label} JSON: {e}"))
    })
}

fn parse_iso_date(label: &str, value: &str) -> Result<Date> {
    let format = time::format_description::well_known::Iso8601::DEFAULT;
    Date::parse(value, &format).map_err(|e| {
        finstack_quant_core::Error::Validation(format!(
            "invalid attribution {label} date '{value}': {e}"
        ))
    })
}

impl AttributionSpec {
    pub(crate) fn build_finstack_config(
        &self,
        instrument_currency: Option<Currency>,
    ) -> Result<FinstackConfig> {
        let mut config = FinstackConfig::default();

        if let Some(ref cfg) = self.config {
            if let Some(scale) = cfg.rounding_scale {
                if let Some(ccy) = instrument_currency {
                    config.rounding.output_scale.overrides.insert(ccy, scale);
                    config.rounding.ingest_scale.overrides.insert(ccy, scale);
                }
            }
            if let Some(rate_bump_bp) = cfg.rate_bump_bp {
                config.extensions.insert(
                    "valuations.sensitivities.v1",
                    json!({ "rate_bump_bp": rate_bump_bp }),
                )?;
            }
        }

        Ok(config)
    }
}

/// Run one attribution specification against many instruments.
///
/// Every instrument is attributed with the same markets, dates, method and
/// configuration carried by `template` (its own `instrument` field is
/// ignored). Results come back in input order, one per instrument; the
/// loop is serial and deterministic, and the first failing instrument aborts
/// the batch with its error. Panics are contained per instrument exactly as
/// in [`AttributionSpec::execute_contained`].
///
/// # Arguments
///
/// * `template` - Attribution run whose markets, valuation dates, method,
///   config, model-parameter snapshot and credit-factor model are shared by
///   every instrument.
/// * `instruments` - Instrument payloads to attribute, in output order.
///
/// # Errors
///
/// Returns the first instrument's error — see [`AttributionSpec::execute`].
///
/// # Returns
///
/// One [`PnlAttribution`] per entry of `instruments`, in the same order.
pub fn attribute_pnl_many(
    template: &AttributionSpec,
    instruments: Vec<InstrumentJson>,
) -> Result<Vec<PnlAttribution>> {
    instruments
        .into_iter()
        .map(|instrument| {
            let spec = AttributionSpec {
                instrument,
                ..template.clone()
            };
            spec.execute_contained().map(|result| result.attribution)
        })
        .collect()
}

/// Default set of metrics for metrics-based attribution.
///
/// Delegates to [`AttributionMethod::required_metrics`] on the `MetricsBased` variant.
pub fn default_attribution_metrics() -> Vec<MetricId> {
    AttributionMethod::MetricsBased.required_metrics()
}

/// Validate an attribution specification JSON payload and return it canonicalized.
///
/// This is the canonical validation entry point shared by the Python and WASM
/// bindings. It deserializes the input against the strict
/// [`AttributionEnvelope`] schema (unknown fields denied), applies the same
/// schema-version gate that [`AttributionEnvelope::execute`] relies on — so a
/// payload that validates here cannot later be rejected at execution for a
/// version mismatch — and re-serializes the envelope to compact canonical JSON.
///
/// # Arguments
///
/// * `json` - JSON-serialized [`AttributionEnvelope`] to validate.
///
/// # Returns
///
/// The canonical compact JSON re-serialization of the validated envelope.
///
/// # Errors
///
/// Returns [`finstack_quant_core::Error::Validation`] when `json` is malformed,
/// violates the exact envelope schema, or carries an unsupported schema
/// version marker, and [`finstack_quant_core::Error::Internal`] if the
/// validated envelope cannot be re-serialized.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_attribution::validate_attribution_json;
///
/// // An empty object is missing every required envelope field.
/// assert!(validate_attribution_json("{}").is_err());
/// ```
pub fn validate_attribution_json(json: &str) -> Result<String> {
    let envelope: AttributionEnvelope = serde_json::from_str(json).map_err(|e| {
        finstack_quant_core::Error::Validation(format!("invalid attribution JSON: {e}"))
    })?;
    // Explicit schema-version gate. `AttributionSchema` deserialization already
    // rejects unknown markers, but the gate is asserted here so the contract is
    // visible at the validation boundary and stays correct if the enum ever
    // grows a second variant.
    if envelope.schema != AttributionSchema::CURRENT {
        return Err(finstack_quant_core::Error::Validation(format!(
            "unsupported attribution schema version; expected {ATTRIBUTION_SCHEMA:?}"
        )));
    }
    serde_json::to_string(&envelope).map_err(|e| {
        finstack_quant_core::Error::Internal(format!(
            "failed to re-serialize validated attribution envelope: {e}"
        ))
    })
}

/// Complete attribution result with P&L attribution and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AttributionResult {
    /// P&L attribution with factor decomposition
    pub attribution: PnlAttribution,
    /// Results metadata (timestamp, version, rounding context, etc.)
    pub results_meta: ResultsMeta,
}

/// Top-level envelope for attribution results.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct AttributionResultEnvelope {
    /// Schema version identifier.
    ///
    /// Deserialization rejects unknown versions instead of silently
    /// re-interpreting them as the current result contract.
    pub schema: AttributionSchema,
    /// The attribution result
    pub result: AttributionResult,
}

impl AttributionResultEnvelope {
    /// Create a new result envelope with the current schema version.
    ///
    /// # Arguments
    ///
    /// * `result` - Completed attribution result and execution metadata to
    ///   wrap in the current persistence envelope.
    pub fn new(result: AttributionResult) -> Self {
        Self {
            schema: AttributionSchema::Attribution,
            result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::create_date;
    use finstack_quant_core::money::Money;
    use time::Month;

    #[test]
    fn test_attribution_envelope_roundtrip() {
        use finstack_quant_valuations::instruments::Bond;

        let bond = Bond::fixed(
            "TEST-BOND",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            create_date(2024, Month::January, 1).expect("Valid test date"),
            create_date(2034, Month::January, 1).expect("Valid test date"),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .unwrap();

        let spec = AttributionSpec {
            instrument: InstrumentJson::Bond(bond),
            market_t0: MarketContextState {
                schema_version: finstack_quant_core::wire::SchemaVersion::CURRENT,
                curves: vec![],
                fx: None,
                surfaces: vec![],
                prices: std::collections::BTreeMap::new(),
                series: vec![],
                inflation_indices: vec![],
                dividends: vec![],
                credit_indices: vec![],
                collateral: std::collections::BTreeMap::new(),
                fx_delta_vol_surfaces: vec![],
                vol_cubes: vec![],
                hierarchy: None,
            },
            market_t1: MarketContextState {
                schema_version: finstack_quant_core::wire::SchemaVersion::CURRENT,
                curves: vec![],
                fx: None,
                surfaces: vec![],
                prices: std::collections::BTreeMap::new(),
                series: vec![],
                inflation_indices: vec![],
                dividends: vec![],
                credit_indices: vec![],
                collateral: std::collections::BTreeMap::new(),
                fx_delta_vol_surfaces: vec![],
                vol_cubes: vec![],
                hierarchy: None,
            },
            as_of_t0: create_date(2025, Month::January, 1).expect("Valid test date"),
            as_of_t1: create_date(2025, Month::January, 2).expect("Valid test date"),
            method: AttributionMethod::Parallel,
            model_params_t0: None,
            config: None,
            credit_factor_model: None,
            credit_factor_detail_options: CreditFactorDetailOptions::default(),
            full_cross_attribution: false,
        };

        let envelope = AttributionEnvelope::new(spec);
        let json = serde_json::to_string_pretty(&envelope)
            .expect("JSON serialization should succeed in test");
        let parsed: AttributionEnvelope =
            serde_json::from_str(&json).expect("JSON deserialization should succeed in test");

        assert_eq!(parsed.schema, AttributionSchema::Attribution);
        assert_eq!(parsed.attribution.as_of_t0, envelope.attribution.as_of_t0);
        assert_eq!(parsed.attribution.as_of_t1, envelope.attribution.as_of_t1);
    }

    #[test]
    fn test_attribution_config_optional_fields() {
        let config = AttributionConfig {
            tolerance_abs: Some(0.01),
            tolerance_pct: Some(0.001),
            metrics: None,
            strict_validation: Some(true),
            rounding_scale: None,
            rate_bump_bp: None,
            target_currency: None,
            execution_policy: None,
        };

        let json =
            serde_json::to_value(&config).expect("JSON value conversion should succeed in test");
        assert!(json.get("tolerance_abs").is_some());
        assert!(json.get("tolerance_pct").is_some());
        assert!(json.get("strict_validation").is_some());
        // metrics should not be present when None
        assert!(json.get("metrics").is_none());
    }

    #[test]
    fn test_attribution_spec_from_json_inputs() {
        use finstack_quant_valuations::instruments::Bond;

        let bond = Bond::fixed(
            "TEST-BOND",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            create_date(2024, Month::January, 1).expect("Valid test date"),
            create_date(2034, Month::January, 1).expect("Valid test date"),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("Bond::fixed should succeed with valid parameters");

        let market_state = MarketContextState {
            schema_version: finstack_quant_core::wire::SchemaVersion::CURRENT,
            curves: vec![],
            fx: None,
            surfaces: vec![],
            prices: std::collections::BTreeMap::new(),
            series: vec![],
            inflation_indices: vec![],
            dividends: vec![],
            credit_indices: vec![],
            collateral: std::collections::BTreeMap::new(),
            fx_delta_vol_surfaces: vec![],
            vol_cubes: vec![],
            hierarchy: None,
        };
        let config = AttributionConfig {
            tolerance_abs: Some(0.01),
            tolerance_pct: None,
            metrics: None,
            strict_validation: Some(true),
            rounding_scale: Some(6),
            rate_bump_bp: None,
            target_currency: None,
            execution_policy: None,
        };

        let instrument_json =
            serde_json::to_string(&InstrumentEnvelope::new(InstrumentJson::Bond(bond)))
                .expect("instrument JSON should serialize");
        let market_json =
            serde_json::to_string(&market_state).expect("market JSON should serialize");
        let method_json = serde_json::to_string(&AttributionMethod::Parallel)
            .expect("method JSON should serialize");
        let config_json = serde_json::to_string(&config).expect("config JSON should serialize");
        let spec = AttributionSpec::from_json_inputs(AttributionJsonInputs {
            instrument_json: &instrument_json,
            market_t0_json: &market_json,
            market_t1_json: &market_json,
            as_of_t0: "2025-01-01",
            as_of_t1: "2025-01-02",
            method_json: &method_json,
            config_json: Some(&config_json),
            model_params_t0_json: None,
            credit_factor_model_json: None,
            full_cross_attribution: false,
        })
        .expect("binding-friendly spec constructor should succeed");

        assert!(matches!(spec.method, AttributionMethod::Parallel));
        assert_eq!(
            spec.as_of_t0,
            create_date(2025, Month::January, 1).expect("Valid test date")
        );
        assert_eq!(
            spec.as_of_t1,
            create_date(2025, Month::January, 2).expect("Valid test date")
        );
        assert!(spec
            .config
            .as_ref()
            .and_then(|cfg| cfg.strict_validation)
            .expect("strict_validation should be preserved"));
        assert!(!spec.full_cross_attribution);
        assert!(spec.credit_factor_model.is_none());
        assert!(spec.model_params_t0.is_none());
    }

    #[test]
    fn attribution_json_inputs_reject_bare_instruments() {
        use finstack_quant_valuations::instruments::Bond;

        let bond = Bond::example().expect("bond example should build");
        // schema-rejection-test: standalone attribution requires an envelope.
        let raw = serde_json::to_string(&InstrumentJson::Bond(bond))
            .expect("instrument JSON should serialize");

        let error = AttributionSpec::from_json_inputs(AttributionJsonInputs {
            instrument_json: &raw,
            market_t0_json: "{}",
            market_t1_json: "{}",
            as_of_t0: "2025-01-01",
            as_of_t1: "2025-01-02",
            method_json: "\"parallel\"",
            config_json: None,
            model_params_t0_json: None,
            credit_factor_model_json: None,
            full_cross_attribution: false,
        })
        .expect_err("bare instrument JSON must be rejected before market parsing");

        assert!(error.to_string().contains("instrument envelope"));
    }

    #[test]
    fn test_attribution_envelope_json_envelope_trait() {
        use finstack_quant_valuations::instruments::Bond;

        let bond = Bond::fixed(
            "TEST-BOND",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            create_date(2024, Month::January, 1).expect("Valid test date"),
            create_date(2034, Month::January, 1).expect("Valid test date"),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("Bond::fixed should succeed with valid parameters");

        let spec = AttributionSpec {
            instrument: InstrumentJson::Bond(bond),
            market_t0: MarketContextState {
                schema_version: finstack_quant_core::wire::SchemaVersion::CURRENT,
                curves: vec![],
                fx: None,
                surfaces: vec![],
                prices: std::collections::BTreeMap::new(),
                series: vec![],
                inflation_indices: vec![],
                dividends: vec![],
                credit_indices: vec![],
                collateral: std::collections::BTreeMap::new(),
                fx_delta_vol_surfaces: vec![],
                vol_cubes: vec![],
                hierarchy: None,
            },
            market_t1: MarketContextState {
                schema_version: finstack_quant_core::wire::SchemaVersion::CURRENT,
                curves: vec![],
                fx: None,
                surfaces: vec![],
                prices: std::collections::BTreeMap::new(),
                series: vec![],
                inflation_indices: vec![],
                dividends: vec![],
                credit_indices: vec![],
                collateral: std::collections::BTreeMap::new(),
                fx_delta_vol_surfaces: vec![],
                vol_cubes: vec![],
                hierarchy: None,
            },
            as_of_t0: create_date(2025, Month::January, 1).expect("Valid test date"),
            as_of_t1: create_date(2025, Month::January, 2).expect("Valid test date"),
            method: AttributionMethod::Parallel,
            model_params_t0: None,
            config: None,
            credit_factor_model: None,
            credit_factor_detail_options: CreditFactorDetailOptions::default(),
            full_cross_attribution: false,
        };

        let envelope = AttributionEnvelope::new(spec);

        // Test serde round-trip
        let json = serde_json::to_string_pretty(&envelope).expect("to_json should succeed");
        assert!(json.contains("finstack_quant.attribution/1"));

        let parsed =
            serde_json::from_str::<AttributionEnvelope>(&json).expect("from_json should succeed");
        assert_eq!(parsed.schema, AttributionSchema::Attribution);
        assert_eq!(parsed.attribution.as_of_t0, envelope.attribution.as_of_t0);

        let reader = std::io::Cursor::new(json.as_bytes());
        let parsed_from_reader = serde_json::from_reader::<_, AttributionEnvelope>(reader)
            .expect("from_reader should succeed");
        assert_eq!(parsed_from_reader.schema, AttributionSchema::Attribution);
    }

    #[test]
    fn validate_attribution_json_roundtrips_and_gates_schema() {
        use finstack_quant_valuations::instruments::Bond;

        let bond = Bond::fixed(
            "TEST-BOND",
            Money::from((1_000_000_i64, Currency::USD)),
            finstack_quant_core::types::Rate::from_decimal(0.05).expect("valid rate fixture"),
            create_date(2024, Month::January, 1).expect("Valid test date"),
            create_date(2034, Month::January, 1).expect("Valid test date"),
            finstack_quant_core::dates::StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("Bond::fixed should succeed with valid parameters");

        let market = MarketContextState::from(
            &finstack_quant_core::market_data::context::MarketContext::new(),
        );
        let spec = AttributionSpec {
            instrument: InstrumentJson::Bond(bond),
            market_t0: market.clone(),
            market_t1: market,
            as_of_t0: create_date(2025, Month::January, 1).expect("Valid test date"),
            as_of_t1: create_date(2025, Month::January, 2).expect("Valid test date"),
            method: AttributionMethod::Parallel,
            model_params_t0: None,
            config: None,
            credit_factor_model: None,
            credit_factor_detail_options: CreditFactorDetailOptions::default(),
            full_cross_attribution: false,
        };
        let envelope = AttributionEnvelope::new(spec);
        let pretty =
            serde_json::to_string_pretty(&envelope).expect("envelope should serialize in test");

        // Valid envelope: canonical compact JSON comes back and re-parses.
        let canonical = validate_attribution_json(&pretty).expect("valid envelope must validate");
        assert!(canonical.contains("finstack_quant.attribution/1"));
        assert!(!canonical.contains('\n'), "canonical form is compact");
        let reparsed: AttributionEnvelope =
            serde_json::from_str(&canonical).expect("canonical output must re-parse");
        assert_eq!(reparsed.schema, AttributionSchema::CURRENT);

        // Malformed and empty payloads are rejected as validation errors.
        for bad in ["not json", "{}"] {
            let err = validate_attribution_json(bad).expect_err("must reject invalid JSON");
            assert!(matches!(err, finstack_quant_core::Error::Validation(_)));
        }

        // Wrong schema version marker is rejected (gate is applied on ingest).
        let wrong_version = pretty.replace(
            "finstack_quant.attribution/1",
            "finstack_quant.attribution/999",
        );
        let err = validate_attribution_json(&wrong_version)
            .expect_err("unsupported schema versions must be rejected");
        assert!(matches!(err, finstack_quant_core::Error::Validation(_)));
    }

    #[test]
    fn test_attribution_result_envelope_json_envelope_trait() {
        use finstack_quant_core::config::ResultsMeta;

        let total = Money::from((1000_i64, Currency::USD));
        let attribution = PnlAttribution::new(
            total,
            "TEST-BOND",
            create_date(2025, Month::January, 1).expect("Valid test date"),
            create_date(2025, Month::January, 2).expect("Valid test date"),
            AttributionMethod::Parallel,
        );

        let result = AttributionResult {
            attribution,
            results_meta: ResultsMeta::default(),
        };

        let envelope = AttributionResultEnvelope::new(result);

        // Test serde round-trip
        let json = serde_json::to_string_pretty(&envelope).expect("to_json should succeed");
        assert!(json.contains("finstack_quant.attribution/1"));

        let parsed = serde_json::from_str::<AttributionResultEnvelope>(&json)
            .expect("from_json should succeed");
        assert_eq!(parsed.schema, AttributionSchema::Attribution);
        assert_eq!(
            parsed.result.attribution.total_pnl,
            envelope.result.attribution.total_pnl
        );

        let reader = std::io::Cursor::new(json.as_bytes());
        let parsed_from_reader = serde_json::from_reader::<_, AttributionResultEnvelope>(reader)
            .expect("from_reader should succeed");
        assert_eq!(parsed_from_reader.schema, AttributionSchema::Attribution);
    }
}
