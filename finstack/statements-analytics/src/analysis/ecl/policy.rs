//! Embedded accounting-policy registry for ECL defaults.

use super::{
    CeclConfig, CeclMethodology, EclConfig, LgdType, MacroScenario, ReversionMethod, StagingConfig,
};
use finstack_core::config::FinstackConfig;
use finstack_core::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::OnceLock;

const EMBEDDED_ECL_POLICY: &str = include_str!("../../../data/accounting/ecl_policy.v1.json");

static EMBEDDED_REGISTRY: OnceLock<Result<EclPolicyRegistry>> = OnceLock::new();

/// Config extension key for overriding ECL accounting-policy defaults.
pub const ECL_POLICY_EXTENSION_KEY: &str = "statements_analytics.ecl_policy.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EclPolicyRegistry {
    schema_version: String,
    default_ifrs9_policy_id: String,
    default_cecl_policy_id: String,
    ifrs9_policies: Vec<Ifrs9PolicyRecord>,
    cecl_policies: Vec<CeclPolicyRecord>,
    binding_defaults: BindingDefaultsRecord,
}

impl EclPolicyRegistry {
    fn default_ifrs9_policy(&self) -> Result<&Ifrs9PolicyRecord> {
        self.ifrs9_policy(&self.default_ifrs9_policy_id)
    }

    fn ifrs9_policy(&self, id: &str) -> Result<&Ifrs9PolicyRecord> {
        self.ifrs9_policies
            .iter()
            .find(|record| has_id(&record.ids, id))
            .ok_or_else(|| not_found("IFRS 9 ECL policy", id))
    }

    fn default_cecl_policy(&self) -> Result<&CeclPolicyRecord> {
        self.cecl_policy(&self.default_cecl_policy_id)
    }

    fn cecl_policy(&self, id: &str) -> Result<&CeclPolicyRecord> {
        self.cecl_policies
            .iter()
            .find(|record| has_id(&record.ids, id))
            .ok_or_else(|| not_found("CECL policy", id))
    }

    fn validate(&self) -> Result<()> {
        if self.schema_version != "finstack.ecl_policy/1" {
            return Err(Error::Validation(format!(
                "unsupported ECL policy schema version '{}'",
                self.schema_version
            )));
        }
        validate_ids(
            "IFRS 9 policy",
            self.ifrs9_policies
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;
        validate_ids(
            "CECL policy",
            self.cecl_policies
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;
        self.default_ifrs9_policy()?;
        self.default_cecl_policy()?;
        for record in &self.ifrs9_policies {
            record.validate()?;
        }
        for record in &self.cecl_policies {
            record.validate()?;
        }
        self.binding_defaults.validate()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ifrs9PolicyRecord {
    ids: Vec<String>,
    source: String,
    source_version: String,
    effective_date: String,
    ecl: Ifrs9EclRecord,
    staging: StagingPolicyRecord,
}

impl Ifrs9PolicyRecord {
    fn validate(&self) -> Result<()> {
        validate_metadata("IFRS 9 policy", &self.source, &self.source_version)?;
        validate_nonblank("IFRS 9 policy effective date", &self.effective_date)?;
        self.ecl.validate()?;
        self.staging.validate()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ifrs9EclRecord {
    bucket_width_years: f64,
    base_scenario_id: String,
    base_scenario_weight: f64,
    base_scenario_lgd_override: Option<f64>,
    lgd_type: LgdType,
}

impl Ifrs9EclRecord {
    fn validate(&self) -> Result<()> {
        validate_positive(self.bucket_width_years, "IFRS 9 bucket width years")?;
        validate_nonblank("IFRS 9 base scenario id", &self.base_scenario_id)?;
        validate_unit_interval(self.base_scenario_weight, "IFRS 9 base scenario weight")?;
        if let Some(lgd) = self.base_scenario_lgd_override {
            validate_unit_interval(lgd, "IFRS 9 base scenario LGD override")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StagingPolicyRecord {
    pd_delta_absolute: f64,
    pd_delta_relative: f64,
    rating_downgrade_notches: u32,
    dpd_stage2_threshold: u32,
    dpd_stage3_threshold: u32,
    qualitative_triggers_enabled: bool,
    stage3_qualitative_triggers_enabled: bool,
    cure_periods_stage2_to_1: u32,
    cure_periods_stage3_to_2: u32,
}

impl StagingPolicyRecord {
    fn validate(&self) -> Result<()> {
        validate_unit_interval(self.pd_delta_absolute, "PD absolute delta threshold")?;
        validate_positive(self.pd_delta_relative, "PD relative threshold")?;
        Ok(())
    }

    fn config(&self) -> StagingConfig {
        StagingConfig {
            pd_delta_absolute: self.pd_delta_absolute,
            pd_delta_relative: self.pd_delta_relative,
            rating_downgrade_notches: self.rating_downgrade_notches,
            dpd_stage2_threshold: self.dpd_stage2_threshold,
            dpd_stage3_threshold: self.dpd_stage3_threshold,
            qualitative_triggers_enabled: self.qualitative_triggers_enabled,
            stage3_qualitative_triggers_enabled: self.stage3_qualitative_triggers_enabled,
            cure_periods_stage2_to_1: self.cure_periods_stage2_to_1,
            cure_periods_stage3_to_2: self.cure_periods_stage3_to_2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CeclPolicyRecord {
    ids: Vec<String>,
    source: String,
    source_version: String,
    effective_date: String,
    bucket_width_years: f64,
    forecast_horizon_years: f64,
    reversion_method: ReversionMethod,
    historical_annual_pd: f64,
    base_scenario_id: String,
    base_scenario_weight: f64,
    base_scenario_lgd_override: Option<f64>,
    methodology: CeclMethodology,
}

impl CeclPolicyRecord {
    fn validate(&self) -> Result<()> {
        validate_metadata("CECL policy", &self.source, &self.source_version)?;
        validate_nonblank("CECL policy effective date", &self.effective_date)?;
        validate_positive(self.bucket_width_years, "CECL bucket width years")?;
        validate_nonnegative(self.forecast_horizon_years, "CECL forecast horizon years")?;
        validate_unit_interval(self.historical_annual_pd, "CECL historical annual PD")?;
        validate_nonblank("CECL base scenario id", &self.base_scenario_id)?;
        validate_unit_interval(self.base_scenario_weight, "CECL base scenario weight")?;
        if let Some(lgd) = self.base_scenario_lgd_override {
            validate_unit_interval(lgd, "CECL base scenario LGD override")?;
        }
        if let ReversionMethod::Linear { reversion_years } = self.reversion_method {
            validate_positive(reversion_years, "CECL linear reversion years")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingDefaultsRecord {
    exposure_dpd: u32,
    classify_stage_pd_delta_absolute: f64,
    classify_stage_dpd_30_trigger: bool,
    classify_stage_dpd_90_trigger: bool,
    classify_stage_cure_periods_stage2_to_1: u32,
    classify_stage_cure_periods_stage3_to_2: u32,
    compute_ecl_bucket_width_years: f64,
}

impl BindingDefaultsRecord {
    fn validate(&self) -> Result<()> {
        validate_unit_interval(
            self.classify_stage_pd_delta_absolute,
            "binding classify-stage PD absolute delta",
        )?;
        validate_positive(
            self.compute_ecl_bucket_width_years,
            "binding ECL bucket width years",
        )
    }
}

/// Return the default IFRS 9 ECL configuration from the embedded policy registry.
pub fn default_ecl_config() -> EclConfig {
    let policy = required_policy(registry().default_ifrs9_policy());
    ecl_config_from_policy(policy)
}

/// Return the default IFRS 9 ECL configuration from config or embedded policy.
pub fn default_ecl_config_from_config(config: &FinstackConfig) -> Result<EclConfig> {
    let registry = registry_from_config(config)?;
    registry.default_ifrs9_policy().map(ecl_config_from_policy)
}

fn ecl_config_from_policy(policy: &Ifrs9PolicyRecord) -> EclConfig {
    EclConfig {
        bucket_width_years: policy.ecl.bucket_width_years,
        scenarios: vec![MacroScenario {
            id: policy.ecl.base_scenario_id.clone(),
            weight: policy.ecl.base_scenario_weight,
            lgd_override: policy.ecl.base_scenario_lgd_override,
        }],
        staging: policy.staging.config(),
        lgd_type: policy.ecl.lgd_type,
        stage3_time_to_recovery_years: super::engine::DEFAULT_STAGE3_TIME_TO_RECOVERY_YEARS,
    }
}

/// Return the default IFRS 9 staging configuration from the embedded policy registry.
pub fn default_staging_config() -> StagingConfig {
    required_policy(registry().default_ifrs9_policy())
        .staging
        .config()
}

/// Return the default IFRS 9 staging configuration from config or embedded policy.
pub fn default_staging_config_from_config(config: &FinstackConfig) -> Result<StagingConfig> {
    let registry = registry_from_config(config)?;
    registry
        .default_ifrs9_policy()
        .map(|policy| policy.staging.config())
}

/// Return the default CECL configuration from the embedded policy registry.
pub fn default_cecl_config() -> CeclConfig {
    let policy = required_policy(registry().default_cecl_policy());
    cecl_config_from_policy(policy)
}

/// Return the default CECL configuration from config or embedded policy.
pub fn default_cecl_config_from_config(config: &FinstackConfig) -> Result<CeclConfig> {
    let registry = registry_from_config(config)?;
    registry.default_cecl_policy().map(cecl_config_from_policy)
}

fn cecl_config_from_policy(policy: &CeclPolicyRecord) -> CeclConfig {
    CeclConfig {
        bucket_width_years: policy.bucket_width_years,
        forecast_horizon_years: policy.forecast_horizon_years,
        reversion_method: policy.reversion_method,
        historical_annual_pd: policy.historical_annual_pd,
        scenarios: vec![MacroScenario {
            id: policy.base_scenario_id.clone(),
            weight: policy.base_scenario_weight,
            lgd_override: policy.base_scenario_lgd_override,
        }],
        methodology: policy.methodology,
    }
}

/// Return the Python binding default DPD value for new exposures.
pub fn binding_default_exposure_dpd() -> u32 {
    registry().binding_defaults.exposure_dpd
}

/// Return the Python binding default absolute PD delta for stage classification.
pub fn binding_default_classify_stage_pd_delta_absolute() -> f64 {
    registry().binding_defaults.classify_stage_pd_delta_absolute
}

/// Return the Python binding default for the 30-DPD Stage 2 trigger toggle.
pub fn binding_default_classify_stage_dpd_30_trigger() -> bool {
    registry().binding_defaults.classify_stage_dpd_30_trigger
}

/// Return the Python binding default for the 90-DPD Stage 3 trigger toggle.
pub fn binding_default_classify_stage_dpd_90_trigger() -> bool {
    registry().binding_defaults.classify_stage_dpd_90_trigger
}

/// Return the Python binding default Stage 2 to Stage 1 cure period.
pub fn binding_default_cure_periods_stage2_to_1() -> u32 {
    registry()
        .binding_defaults
        .classify_stage_cure_periods_stage2_to_1
}

/// Return the Python binding default Stage 3 to Stage 2 cure period.
pub fn binding_default_cure_periods_stage3_to_2() -> u32 {
    registry()
        .binding_defaults
        .classify_stage_cure_periods_stage3_to_2
}

/// Return the Python binding default ECL bucket width in years.
pub fn binding_default_compute_ecl_bucket_width_years() -> f64 {
    registry().binding_defaults.compute_ecl_bucket_width_years
}

#[allow(clippy::expect_used)]
fn registry() -> &'static EclPolicyRegistry {
    embedded_registry().expect("embedded ECL policy registry should load")
}

#[allow(clippy::expect_used)]
fn required_policy<T>(result: Result<T>) -> T {
    result.expect("embedded ECL policy registry value should exist")
}

pub(crate) fn embedded_registry() -> Result<&'static EclPolicyRegistry> {
    match EMBEDDED_REGISTRY.get_or_init(|| parse_registry_json(EMBEDDED_ECL_POLICY)) {
        Ok(registry) => Ok(registry),
        Err(err) => Err(err.clone()),
    }
}

pub(crate) fn registry_from_config(config: &FinstackConfig) -> Result<EclPolicyRegistry> {
    if let Some(value) = config.extensions.get(ECL_POLICY_EXTENSION_KEY) {
        let registry = serde_json::from_value(value.clone()).map_err(|err| {
            Error::Validation(format!(
                "failed to parse ECL policy registry extension: {err}"
            ))
        })?;
        validate_registry(registry)
    } else {
        Ok(embedded_registry()?.clone())
    }
}

fn parse_registry_json(raw: &str) -> Result<EclPolicyRegistry> {
    let registry = serde_json::from_str(raw).map_err(|err| {
        Error::Validation(format!(
            "failed to parse embedded ECL policy registry: {err}"
        ))
    })?;
    validate_registry(registry)
}

fn validate_registry(registry: EclPolicyRegistry) -> Result<EclPolicyRegistry> {
    registry.validate()?;
    Ok(registry)
}

fn validate_ids<'a>(kind: &str, records: impl Iterator<Item = &'a [String]>) -> Result<()> {
    let mut seen = BTreeSet::new();
    for ids in records {
        if ids.is_empty() {
            return Err(Error::Validation(format!(
                "ECL policy registry contains {kind} without an id"
            )));
        }
        for id in ids {
            let trimmed = id.trim();
            if trimmed.is_empty() {
                return Err(Error::Validation(format!(
                    "ECL policy registry contains blank {kind} id"
                )));
            }
            if !seen.insert(trimmed.to_string()) {
                return Err(Error::Validation(format!(
                    "ECL policy registry contains duplicate {kind} id '{trimmed}'"
                )));
            }
        }
    }
    Ok(())
}

fn validate_metadata(label: &str, source: &str, source_version: &str) -> Result<()> {
    validate_nonblank(label, source)?;
    validate_nonblank(label, source_version)
}

fn validate_nonblank(label: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        Err(Error::Validation(format!(
            "ECL policy registry has blank {label}"
        )))
    } else {
        Ok(())
    }
}

fn validate_positive(value: f64, label: &str) -> Result<()> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(Error::Validation(format!(
            "ECL policy registry has invalid {label} {value}"
        )))
    }
}

fn validate_nonnegative(value: f64, label: &str) -> Result<()> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(Error::Validation(format!(
            "ECL policy registry has invalid {label} {value}"
        )))
    }
}

fn validate_unit_interval(value: f64, label: &str) -> Result<()> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(Error::Validation(format!(
            "ECL policy registry has invalid {label} {value}"
        )))
    }
}

fn has_id(ids: &[String], id: &str) -> bool {
    ids.iter().any(|candidate| candidate == id)
}

fn not_found(kind: &str, id: &str) -> Error {
    Error::Validation(format!(
        "ECL policy registry does not contain {kind} '{id}'"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_registry_preserves_ifrs9_defaults() {
        let staging = default_staging_config();
        assert_eq!(staging.pd_delta_absolute, 0.01);
        assert_eq!(staging.pd_delta_relative, 2.0);
        assert_eq!(staging.dpd_stage2_threshold, 30);
        assert_eq!(staging.dpd_stage3_threshold, 90);
        assert_eq!(staging.cure_periods_stage2_to_1, 3);
        assert_eq!(staging.cure_periods_stage3_to_2, 12);

        let config = default_ecl_config();
        assert_eq!(config.bucket_width_years, 0.25);
        assert_eq!(config.scenarios[0].id, "base");
        assert_eq!(config.scenarios[0].weight, 1.0);
        assert_eq!(config.lgd_type, LgdType::PointInTime);
    }

    #[test]
    fn embedded_registry_preserves_cecl_and_binding_defaults() {
        let config = default_cecl_config();
        assert_eq!(config.bucket_width_years, 0.25);
        assert_eq!(config.forecast_horizon_years, 2.0);
        assert_eq!(config.reversion_method, ReversionMethod::Immediate);
        assert_eq!(config.historical_annual_pd, 0.02);
        assert_eq!(config.methodology, CeclMethodology::PdLgdEad);

        assert_eq!(binding_default_exposure_dpd(), 0);
        assert_eq!(binding_default_classify_stage_pd_delta_absolute(), 0.01);
        assert!(binding_default_classify_stage_dpd_30_trigger());
        assert!(binding_default_classify_stage_dpd_90_trigger());
        assert_eq!(binding_default_cure_periods_stage2_to_1(), 3);
        // Aligned with the core IFRS 9 staging default (EBA GL 12-month
        // probation); was previously 6 and diverged from
        // `ifrs9_policies[0].staging.cure_periods_stage3_to_2`.
        assert_eq!(binding_default_cure_periods_stage3_to_2(), 12);
        assert_eq!(binding_default_compute_ecl_bucket_width_years(), 0.25);
    }
}
