//! JSON specification and execution framework for attribution.
//!
//! Provides serializable specs for defining complete attribution runs in JSON,
//! with stable schemas and deterministic round-trip serialization.

use super::{
    AttributionMethod, CreditFactorDetailOptions, CreditFactorModelRef, ModelParamsSnapshot,
    PnlAttribution,
};
use crate::instruments::InstrumentJson;
use crate::metrics::MetricId;
use finstack_core::{
    config::{FinstackConfig, ResultsMeta},
    currency::Currency,
    dates::Date,
    market_data::context::MarketContextState,
    Result,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Schema version for attribution serialization.
pub const ATTRIBUTION_SCHEMA_V1: &str = "finstack.attribution/1";

/// Top-level envelope for attribution specifications.
///
/// Mirrors the calibration and instrument envelope patterns with schema versioning
/// and strict field validation for long-term JSON stability.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttributionEnvelope {
    /// Schema version identifier (currently "finstack.attribution/1")
    pub schema: String,
    /// The attribution specification
    pub attribution: AttributionSpec,
}

impl AttributionEnvelope {
    /// Create a new attribution envelope with the current schema version.
    pub fn new(attribution: AttributionSpec) -> Self {
        Self {
            schema: ATTRIBUTION_SCHEMA_V1.to_string(),
            attribution,
        }
    }

    /// Execute the attribution and return the result envelope.
    pub fn execute(&self) -> Result<AttributionResultEnvelope> {
        if self.schema != ATTRIBUTION_SCHEMA_V1 {
            return Err(finstack_core::Error::Validation(format!(
                "Unsupported attribution schema '{}'; supported schemas: {}",
                self.schema, ATTRIBUTION_SCHEMA_V1
            )));
        }
        let result = self.attribution.execute()?;
        Ok(AttributionResultEnvelope::new(result))
    }
}

/// Attribution specification for a single P&L attribution run.
///
/// Contains all data needed to perform attribution: instrument, market snapshots,
/// dates, and methodology.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttributionSpec {
    /// Instrument to attribute (as JSON envelope)
    pub instrument: InstrumentJson,
    /// Market context at T₀
    #[schemars(with = "serde_json::Value")]
    pub market_t0: MarketContextState,
    /// Market context at T₁
    #[schemars(with = "serde_json::Value")]
    pub market_t1: MarketContextState,
    /// Valuation date at T₀
    #[schemars(with = "String")]
    pub as_of_t0: Date,
    /// Valuation date at T₁
    #[schemars(with = "String")]
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
    /// `credit_curves_pnl`. PR-7 wires metrics-based and Taylor methods.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub credit_factor_model: Option<CreditFactorModelRef>,
    /// Detail/payload options for `credit_factor_detail`. Inert when
    /// `credit_factor_model` is `None`.
    #[serde(default)]
    pub credit_factor_detail_options: CreditFactorDetailOptions,
    /// Option to compute all 36 cross-factor pairs when enabled
    #[serde(default)]
    pub full_cross_attribution: bool,
}

/// Optional configuration for attribution runs.
///
/// Allows overriding default tolerances and metrics for attribution calculations.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
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
    /// Strict validation mode (if true, errors during attribution will propagate instead of being logged)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strict_validation: Option<bool>,
    /// Rounding scale override (number of decimal places)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rounding_scale: Option<u32>,
    /// Rate bump size in basis points for sensitivities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_bump_bp: Option<f64>,
}

impl AttributionSpec {
    /// Build an attribution spec from the JSON-friendly inputs used by bindings.
    pub fn from_json_inputs(
        instrument_json: &str,
        market_t0_json: &str,
        market_t1_json: &str,
        as_of_t0: &str,
        as_of_t1: &str,
        method_json: &str,
        config_json: Option<&str>,
    ) -> Result<Self> {
        Ok(Self {
            instrument: parse_input_json("instrument", instrument_json)?,
            market_t0: parse_input_json("market_t0", market_t0_json)?,
            market_t1: parse_input_json("market_t1", market_t1_json)?,
            as_of_t0: parse_iso_date("as_of_t0", as_of_t0)?,
            as_of_t1: parse_iso_date("as_of_t1", as_of_t1)?,
            method: parse_input_json("method", method_json)?,
            model_params_t0: None,
            config: config_json
                .map(|json| parse_input_json("config", json))
                .transpose()?,
            credit_factor_model: None,
            credit_factor_detail_options: CreditFactorDetailOptions::default(),
            full_cross_attribution: false,
        })
    }
}

fn parse_input_json<T: DeserializeOwned>(label: &str, json: &str) -> Result<T> {
    serde_json::from_str(json).map_err(|e| {
        finstack_core::Error::Validation(format!("invalid attribution {label} JSON: {e}"))
    })
}

fn parse_iso_date(label: &str, value: &str) -> Result<Date> {
    let format = time::format_description::well_known::Iso8601::DEFAULT;
    Date::parse(value, &format).map_err(|e| {
        finstack_core::Error::Validation(format!("invalid attribution {label} date '{value}': {e}"))
    })
}

impl AttributionSpec {
    pub(crate) fn build_finstack_config(&self, instrument_ccy: Option<Currency>) -> FinstackConfig {
        let mut config = FinstackConfig::default();

        if let Some(ref cfg) = self.config {
            if let Some(scale) = cfg.rounding_scale {
                if let Some(ccy) = instrument_ccy {
                    config.rounding.output_scale.overrides.insert(ccy, scale);
                    config.rounding.ingest_scale.overrides.insert(ccy, scale);
                }
            }
            if let Some(rate_bump_bp) = cfg.rate_bump_bp {
                config.extensions.insert(
                    "valuations.sensitivities.v1",
                    json!({ "rate_bump_bp": rate_bump_bp }),
                );
            }
        }

        config
    }
}

/// Default set of metrics for metrics-based attribution.
///
/// Delegates to [`AttributionMethod::required_metrics`] on the `MetricsBased` variant.
pub fn default_attribution_metrics() -> Vec<MetricId> {
    AttributionMethod::MetricsBased.required_metrics()
}

/// Complete attribution result with P&L attribution and metadata.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttributionResult {
    /// P&L attribution with factor decomposition
    pub attribution: PnlAttribution,
    /// Results metadata (timestamp, version, rounding context, etc.)
    pub results_meta: ResultsMeta,
}

/// Top-level envelope for attribution results.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttributionResultEnvelope {
    /// Schema version identifier
    pub schema: String,
    /// The attribution result
    pub result: AttributionResult,
}

impl AttributionResultEnvelope {
    /// Create a new result envelope with the current schema version.
    pub fn new(result: AttributionResult) -> Self {
        Self {
            schema: ATTRIBUTION_SCHEMA_V1.to_string(),
            result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_core::currency::Currency;
    use finstack_core::dates::create_date;
    use finstack_core::money::Money;
    use time::Month;

    #[test]
    #[allow(clippy::unwrap_used)] // Test code
    fn test_attribution_envelope_roundtrip() {
        use crate::instruments::Bond;

        let bond = Bond::fixed(
            "TEST-BOND",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            create_date(2024, Month::January, 1).expect("Valid test date"),
            create_date(2034, Month::January, 1).expect("Valid test date"),
            "USD-OIS",
        )
        .unwrap();

        let spec = AttributionSpec {
            instrument: InstrumentJson::Bond(bond),
            market_t0: MarketContextState {
                version: finstack_core::market_data::context::MARKET_CONTEXT_STATE_VERSION,
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
                version: finstack_core::market_data::context::MARKET_CONTEXT_STATE_VERSION,
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

        assert_eq!(parsed.schema, ATTRIBUTION_SCHEMA_V1);
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
        use crate::instruments::Bond;

        let bond = Bond::fixed(
            "TEST-BOND",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            create_date(2024, Month::January, 1).expect("Valid test date"),
            create_date(2034, Month::January, 1).expect("Valid test date"),
            "USD-OIS",
        )
        .expect("Bond::fixed should succeed with valid parameters");

        let market_state = MarketContextState {
            version: finstack_core::market_data::context::MARKET_CONTEXT_STATE_VERSION,
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
        };

        let spec = AttributionSpec::from_json_inputs(
            &serde_json::to_string(&InstrumentJson::Bond(bond))
                .expect("instrument JSON should serialize"),
            &serde_json::to_string(&market_state).expect("market_t0 JSON should serialize"),
            &serde_json::to_string(&market_state).expect("market_t1 JSON should serialize"),
            "2025-01-01",
            "2025-01-02",
            &serde_json::to_string(&AttributionMethod::Parallel)
                .expect("method JSON should serialize"),
            Some(&serde_json::to_string(&config).expect("config JSON should serialize")),
        )
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
    }

    #[test]
    fn test_attribution_envelope_json_envelope_trait() {
        use crate::instruments::Bond;

        let bond = Bond::fixed(
            "TEST-BOND",
            Money::new(1_000_000.0, Currency::USD),
            0.05,
            create_date(2024, Month::January, 1).expect("Valid test date"),
            create_date(2034, Month::January, 1).expect("Valid test date"),
            "USD-OIS",
        )
        .expect("Bond::fixed should succeed with valid parameters");

        let spec = AttributionSpec {
            instrument: InstrumentJson::Bond(bond),
            market_t0: MarketContextState {
                version: finstack_core::market_data::context::MARKET_CONTEXT_STATE_VERSION,
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
                version: finstack_core::market_data::context::MARKET_CONTEXT_STATE_VERSION,
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
        assert!(json.contains("finstack.attribution/1"));

        let parsed =
            serde_json::from_str::<AttributionEnvelope>(&json).expect("from_json should succeed");
        assert_eq!(parsed.schema, ATTRIBUTION_SCHEMA_V1);
        assert_eq!(parsed.attribution.as_of_t0, envelope.attribution.as_of_t0);

        let reader = std::io::Cursor::new(json.as_bytes());
        let parsed_from_reader = serde_json::from_reader::<_, AttributionEnvelope>(reader)
            .expect("from_reader should succeed");
        assert_eq!(parsed_from_reader.schema, ATTRIBUTION_SCHEMA_V1);
    }

    #[test]
    fn test_attribution_result_envelope_json_envelope_trait() {
        use finstack_core::config::ResultsMeta;

        let total = Money::new(1000.0, Currency::USD);
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
        assert!(json.contains("finstack.attribution/1"));

        let parsed = serde_json::from_str::<AttributionResultEnvelope>(&json)
            .expect("from_json should succeed");
        assert_eq!(parsed.schema, ATTRIBUTION_SCHEMA_V1);
        assert_eq!(
            parsed.result.attribution.total_pnl,
            envelope.result.attribution.total_pnl
        );

        let reader = std::io::Cursor::new(json.as_bytes());
        let parsed_from_reader = serde_json::from_reader::<_, AttributionResultEnvelope>(reader)
            .expect("from_reader should succeed");
        assert_eq!(parsed_from_reader.schema, ATTRIBUTION_SCHEMA_V1);
    }
}
