use serde::Deserialize;
use serde_json::Value;

// Shared envelope used by embedded registry files (similar to market conventions).
#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RegistryFile<R> {
    pub(super) schema: Option<String>,
    pub(super) version: Option<u32>,
    pub(super) entries: Vec<RegistryEntry<R>>,
}

#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RegistryEntry<R> {
    pub(super) ids: Vec<String>,
    pub(super) record: R,
}

// -----------------------------------------------------------------------------//
// Schedule IM (BCBS-IOSCO grid)
// -----------------------------------------------------------------------------//

#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ScheduleImFile {
    pub(super) schema: Option<String>,
    pub(super) version: Option<u32>,
    pub(super) entries: Vec<RegistryEntry<ScheduleImRecord>>,
}

#[allow(dead_code)] // Fields accessed via serde Deserialize
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

#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ScheduleBucketBoundaries {
    pub(super) short_to_medium: f64,
    pub(super) medium_to_long: f64,
}

#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ScheduleImRate {
    pub(super) asset_class: String,
    pub(super) bucket: String,
    pub(super) rate: f64,
}

// -----------------------------------------------------------------------------//
// Collateral schedules and defaults
// -----------------------------------------------------------------------------//

#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CollateralSchedulesFile {
    pub(super) schema: Option<String>,
    pub(super) version: Option<u32>,
    pub(super) asset_class_defaults: Vec<AssetClassDefault>,
    pub(super) entries: Vec<RegistryEntry<CollateralScheduleRecord>>,
}

#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AssetClassDefault {
    pub(super) asset_class: String,
    pub(super) standard_haircut: f64,
    pub(super) fx_addon: f64,
    #[serde(default)]
    pub(super) concentration_limit: Option<f64>,
}

#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CollateralScheduleRecord {
    pub(super) eligible: Vec<CollateralEligibilityRecord>,
    pub(super) default_haircut: Option<f64>,
    pub(super) rehypothecation_allowed: bool,
}

#[allow(dead_code)] // Fields accessed via serde Deserialize
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

#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MaturityConstraintsRecord {
    #[serde(default)]
    pub(super) min_remaining_years: Option<f64>,
    #[serde(default)]
    pub(super) max_remaining_years: Option<f64>,
}

// -----------------------------------------------------------------------------//
// Defaults (VM/IM thresholds, timing, settlement)
// -----------------------------------------------------------------------------//

#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DefaultsFile {
    pub(super) schema: Option<String>,
    pub(super) version: Option<u32>,
    pub(super) defaults: DefaultsRecord,
}

#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DefaultsRecord {
    pub(super) vm: VmDefaultsRecord,
    pub(super) im: ImDefaultsRecord,
    pub(super) timing: TimingDefaultsRecord,
    pub(super) cleared_settlement: ClearedSettlementRecord,
}

#[allow(dead_code)] // Fields accessed via serde Deserialize
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

#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ImDefaultsRecord {
    pub(super) simm: ImMethodDefaultsRecord,
    pub(super) schedule: ImMethodDefaultsRecord,
    pub(super) cleared: ImMethodDefaultsRecord,
    pub(super) repo_haircut: ImMethodDefaultsRecord,
}

#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct ImMethodDefaultsRecord {
    pub(super) mpor_days: u32,
    pub(super) threshold: f64,
    pub(super) mta: f64,
    pub(super) segregated: bool,
}

#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TimingDefaultsRecord {
    pub(super) standard: MarginCallTimingRecord,
    pub(super) regulatory_vm: MarginCallTimingRecord,
    pub(super) ccp: MarginCallTimingRecord,
}

#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(super) struct MarginCallTimingRecord {
    pub(super) notification_deadline_hours: u8,
    pub(super) response_deadline_hours: u8,
    pub(super) dispute_resolution_days: u8,
    pub(super) delivery_grace_days: u8,
}

#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ClearedSettlementRecord {
    pub(super) rounding: f64,
    pub(super) settlement_lag: u32,
}

// -----------------------------------------------------------------------------//
// CCP methodologies
// -----------------------------------------------------------------------------//

#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CcpFile {
    pub(super) schema: Option<String>,
    pub(super) version: Option<u32>,
    pub(super) entries: Vec<RegistryEntry<CcpRecord>>,
}

#[allow(dead_code)] // Fields accessed via serde Deserialize
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

// -----------------------------------------------------------------------------//
// XVA defaults
// -----------------------------------------------------------------------------//

#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct XvaDefaultsFile {
    pub(super) schema: Option<String>,
    pub(super) version: Option<u32>,
    pub(super) defaults: XvaDefaultsRecord,
}

#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct XvaDefaultsRecord {
    pub(super) deterministic_exposure: XvaDeterministicExposureRecord,
    pub(super) stochastic_exposure: XvaStochasticExposureRecord,
}

#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct XvaDeterministicExposureRecord {
    pub(super) time_grid_points: usize,
    pub(super) time_grid_step_years: f64,
    pub(super) recovery_rate: f64,
    #[serde(default)]
    pub(super) own_recovery_rate: Option<f64>,
}

#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct XvaStochasticExposureRecord {
    pub(super) num_paths: usize,
    pub(super) seed: u64,
    pub(super) pfe_quantile: f64,
}

// -----------------------------------------------------------------------------//
// SIMM parameters
// -----------------------------------------------------------------------------//

#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SimmFile {
    pub(super) schema: Option<String>,
    pub(super) version: Option<u32>,
    pub(super) entries: Vec<RegistryEntry<SimmRecord>>,
}

#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SimmRecord {
    pub(super) mpor_days: u32,
    pub(super) ir_delta_weights: Value,
    pub(super) cq_delta_weights: Value,
    #[serde(default)]
    pub(super) cq_bucket_weights: Value,
    pub(super) cnq_delta_weight: f64,
    pub(super) equity_delta_weight: f64,
    pub(super) fx_delta_weight: f64,
    #[serde(default)]
    pub(super) fx_intra_bucket_correlation: Option<f64>,
    pub(super) risk_class_correlations: Vec<RiskClassCorrelationRecord>,
    pub(super) commodity_bucket_weights: Value,
    #[serde(default)]
    pub(super) commodity_inter_bucket_correlations: Vec<f64>,
    #[serde(default)]
    pub(super) ir_tenor_correlations: Value,
    #[serde(default)]
    pub(super) ir_inter_currency_correlation: Option<f64>,
    #[serde(default)]
    pub(super) ir_vega_weight: Option<f64>,
    #[serde(default)]
    pub(super) cq_vega_weight: Option<f64>,
    #[serde(default)]
    pub(super) cq_intra_bucket_correlation: Option<f64>,
    #[serde(default)]
    pub(super) cq_inter_bucket_correlations: Value,
    #[serde(default)]
    pub(super) cq_concentration_thresholds: Value,
    #[serde(default)]
    pub(super) cnq_vega_weight: Option<f64>,
    #[serde(default)]
    pub(super) equity_vega_weight: Option<f64>,
    #[serde(default)]
    pub(super) fx_vega_weight: Option<f64>,
    #[serde(default)]
    pub(super) commodity_vega_weight: Option<f64>,
    #[serde(default)]
    pub(super) curvature_scale_factor: Option<f64>,
    #[serde(default)]
    pub(super) concentration_thresholds: Value,
}

#[allow(dead_code)] // Fields accessed via serde Deserialize
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RiskClassCorrelationRecord {
    pub(super) a: String,
    pub(super) b: String,
    pub(super) rho: f64,
}
