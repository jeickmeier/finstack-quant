//! Loading, merging, and wire-format support for margin registry data.
//!
//! Every type below is a deserialization target for an embedded registry JSON
//! file. Their fields are read by serde and never by a Rust call-site, so the
//! module as a whole opts out of `dead_code` rather than annotating each type.
#![allow(dead_code)]

use serde::Deserialize;
use serde_json::Value;

// Shared envelope used by embedded registry files (similar to market conventions).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RegistryEntry<R> {
    pub(super) ids: Vec<String>,
    pub(super) record: R,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ScheduleImFile {
    pub(super) schema: Option<String>,
    pub(super) version: Option<u32>,
    pub(super) entries: Vec<RegistryEntry<ScheduleImRecord>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ScheduleImRecord {
    pub(super) bucket_boundaries_years: ScheduleBucketBoundaries,
    pub(super) default_rate: f64,
    pub(super) default_asset_class: String,
    pub(super) default_maturity_years: f64,
    pub(super) mpor_days: u32,
    pub(super) rates: Vec<ScheduleImRate>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ScheduleBucketBoundaries {
    pub(super) short_to_medium: f64,
    pub(super) medium_to_long: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ScheduleImRate {
    pub(super) asset_class: String,
    pub(super) bucket: String,
    pub(super) rate: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CollateralSchedulesFile {
    pub(super) schema: Option<String>,
    pub(super) version: Option<u32>,
    pub(super) asset_class_defaults: Vec<AssetClassDefault>,
    pub(super) entries: Vec<RegistryEntry<CollateralScheduleRecord>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AssetClassDefault {
    pub(super) asset_class: String,
    pub(super) standard_haircut: f64,
    pub(super) fx_addon: f64,
    #[serde(default)]
    pub(super) concentration_limit: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CollateralScheduleRecord {
    pub(super) eligible: Vec<CollateralEligibilityRecord>,
    pub(super) default_haircut: Option<f64>,
    pub(super) rehypothecation_allowed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CollateralEligibilityRecord {
    pub(super) asset_class: String,
    #[serde(default)]
    pub(super) min_rating: Option<String>,
    #[serde(default)]
    pub(super) maturity_constraints: Option<MaturityConstraintsRecord>,
    pub(super) haircut: f64,
    pub(super) fx_haircut_addon: f64,
    #[serde(default)]
    pub(super) concentration_limit: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MaturityConstraintsRecord {
    #[serde(default)]
    pub(super) min_remaining_years: Option<f64>,
    #[serde(default)]
    pub(super) max_remaining_years: Option<f64>,
}

// Defaults (VM/IM thresholds, timing, settlement)

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DefaultsFile {
    pub(super) schema: Option<String>,
    pub(super) version: Option<u32>,
    pub(super) defaults: DefaultsRecord,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DefaultsRecord {
    pub(super) vm: VmDefaultsRecord,
    pub(super) im: ImDefaultsRecord,
    pub(super) timing: TimingDefaultsRecord,
    pub(super) cleared_settlement: ClearedSettlementRecord,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct VmDefaultsRecord {
    pub(super) threshold: f64,
    pub(super) mta: f64,
    pub(super) rounding: f64,
    pub(super) independent_amount: f64,
    pub(super) frequency: String,
    pub(super) settlement_lag: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ImDefaultsRecord {
    pub(super) simm: ImMethodDefaultsRecord,
    pub(super) schedule: ImMethodDefaultsRecord,
    pub(super) cleared: ImMethodDefaultsRecord,
    pub(super) repo_haircut: ImMethodDefaultsRecord,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct ImMethodDefaultsRecord {
    pub(super) mpor_days: u32,
    pub(super) threshold: f64,
    pub(super) mta: f64,
    pub(super) segregated: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TimingDefaultsRecord {
    pub(super) standard: MarginCallTimingRecord,
    pub(super) regulatory_vm: MarginCallTimingRecord,
    pub(super) ccp: MarginCallTimingRecord,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct MarginCallTimingRecord {
    pub(super) notification_deadline_hours: u8,
    pub(super) response_deadline_hours: u8,
    pub(super) dispute_resolution_days: u8,
    pub(super) delivery_grace_days: u8,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ClearedSettlementRecord {
    pub(super) rounding: f64,
    pub(super) settlement_lag: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CcpFile {
    pub(super) schema: Option<String>,
    pub(super) version: Option<u32>,
    pub(super) entries: Vec<RegistryEntry<CcpRecord>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CcpRecord {
    pub(super) mpor_days: u32,
    pub(super) conservative_rate: f64,
    #[serde(default)]
    pub(super) generic_var_confidence: Option<f64>,
    #[serde(default)]
    pub(super) generic_var_lookback_days: Option<u32>,
    #[serde(default)]
    pub(super) is_default: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SimmFile {
    pub(super) schema: Option<String>,
    pub(super) version: Option<u32>,
    pub(super) entries: Vec<RegistryEntry<SimmRecord>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SimmRecord {
    pub(super) ir_subcurve_correlation: f64,
    pub(super) commodity_intra_bucket_correlations: Value,
    pub(super) ir_historical_volatility_ratio: f64,
    pub(super) equity_historical_volatility_ratio: f64,
    pub(super) fx_historical_volatility_ratio: f64,
    pub(super) commodity_historical_volatility_ratio: f64,
    pub(super) fx_high_delta_weight: f64,
    pub(super) fx_regular_high_correlation: f64,
    pub(super) fx_high_high_correlation: f64,
    pub(super) cq_same_issuer_correlation: f64,
    pub(super) credit_residual_correlation: f64,
    pub(super) ir_delta_weights_low: Value,
    pub(super) ir_delta_weights_high: Value,
    pub(super) ir_delta_concentration_thresholds: Value,
    pub(super) ir_vega_concentration_thresholds: Value,
    pub(super) fx_delta_concentration_thresholds: Value,
    pub(super) fx_vega_concentration_thresholds: Value,
    pub(super) commodity_delta_concentration_thresholds: Value,
    pub(super) commodity_vega_concentration_thresholds: Value,
    pub(super) vega_concentration_thresholds: Value,
    pub(super) mpor_days: u32,
    pub(super) ir_delta_weights: Value,
    #[serde(default)]
    pub(super) cq_bucket_weights: Value,
    pub(super) cnq_delta_weight: f64,
    pub(super) equity_delta_weight: f64,
    pub(super) fx_delta_weight: f64,
    pub(super) fx_intra_bucket_correlation: f64,
    pub(super) risk_class_correlations: Vec<RiskClassCorrelationRecord>,
    pub(super) commodity_bucket_weights: Value,
    #[serde(default)]
    pub(super) commodity_inter_bucket_correlations: Vec<f64>,
    #[serde(default)]
    pub(super) ir_tenor_correlations: Value,
    pub(super) ir_inter_currency_correlation: f64,
    pub(super) ir_vega_weight: f64,
    pub(super) cq_vega_weight: f64,
    pub(super) cq_intra_bucket_correlation: f64,
    #[serde(default)]
    pub(super) cq_inter_bucket_correlations: Value,
    #[serde(default)]
    pub(super) cq_concentration_thresholds: Value,
    pub(super) cnq_vega_weight: f64,
    pub(super) equity_vega_weight: f64,
    pub(super) fx_vega_weight: f64,
    pub(super) commodity_vega_weight: f64,
    #[serde(default)]
    pub(super) concentration_thresholds: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RiskClassCorrelationRecord {
    pub(super) a: String,
    pub(super) b: String,
    pub(super) rho: f64,
}
