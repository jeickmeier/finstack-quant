//! Embedded accounting-policy registry for ECL defaults.

use super::{CeclConfig, CeclMethodology, EclConfig, LgdType, ReversionMethod, StagingConfig};
use finstack_quant_core::embedded_registry::EmbeddedJsonRegistry;
use finstack_quant_core::{Error, Result};
use serde::{Deserialize, Serialize};

static EMBEDDED_REGISTRY: EmbeddedJsonRegistry<EclPolicyRegistry> = EmbeddedJsonRegistry::new(
    include_str!("../../../data/accounting/ecl_policy.v1.json"),
    None,
    "ECL policy",
);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EclPolicyRegistry {
    schema: String,
    default_ifrs9_policy_id: String,
    default_cecl_policy_id: String,
    ifrs9_policies: Vec<Ifrs9PolicyRecord>,
    cecl_policies: Vec<CeclPolicyRecord>,
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
        if self.schema != "finstack_quant.ecl_policy/1" {
            return Err(Error::Validation(format!(
                "unsupported ECL policy schema version '{}'",
                self.schema
            )));
        }
        finstack_quant_core::validation::validate_unique_ids(
            "ECL policy registry",
            "IFRS 9 policy",
            self.ifrs9_policies
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;
        finstack_quant_core::validation::validate_unique_ids(
            "ECL policy registry",
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
        Ok(())
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
        finstack_quant_core::validation::validate_source_metadata(
            "IFRS 9 policy",
            &self.source,
            &self.source_version,
        )?;
        finstack_quant_core::validation::validate_non_blank(
            &self.effective_date,
            "IFRS 9 policy effective date",
        )?;
        self.ecl.validate()?;
        self.staging.validate()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ifrs9EclRecord {
    bucket_width_years: f64,
    lgd_type: LgdType,
}

impl Ifrs9EclRecord {
    fn validate(&self) -> Result<()> {
        finstack_quant_core::validation::validate_f64_positive(
            self.bucket_width_years,
            "IFRS 9 bucket width years",
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StagingPolicyRecord {
    pd_delta_absolute: f64,
    pd_delta_relative: Option<f64>,
    rating_downgrade_notches: u32,
    #[serde(default)]
    rating_scale_labels: Option<Vec<String>>,
    dpd_stage2_threshold: u32,
    dpd_stage3_threshold: u32,
    qualitative_triggers_enabled: bool,
    stage3_qualitative_triggers_enabled: bool,
    cure_periods_stage2_to_1: u32,
    cure_periods_stage3_to_2: u32,
}

impl StagingPolicyRecord {
    fn validate(&self) -> Result<()> {
        finstack_quant_core::validation::validate_f64_unit_interval(
            self.pd_delta_absolute,
            "PD absolute delta threshold",
        )?;
        if let Some(threshold) = self.pd_delta_relative {
            finstack_quant_core::validation::validate_f64_non_negative(
                threshold,
                "PD relative threshold",
            )?;
        }
        Ok(())
    }

    fn config(&self) -> StagingConfig {
        StagingConfig {
            pd_delta_absolute: self.pd_delta_absolute,
            pd_delta_relative: self.pd_delta_relative,
            rating_downgrade_notches: self.rating_downgrade_notches,
            rating_scale_labels: self.rating_scale_labels.clone(),
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
    #[serde(default = "default_cecl_impaired_dpd_threshold")]
    impaired_dpd_threshold: u32,
    #[serde(default = "default_cecl_impaired_qualitative_triggers_enabled")]
    impaired_qualitative_triggers_enabled: bool,
    #[serde(default = "default_cecl_impaired_time_to_recovery_years")]
    impaired_time_to_recovery_years: f64,
    #[serde(default = "default_cecl_discount_expected_losses")]
    discount_expected_losses: bool,
    methodology: CeclMethodology,
}

impl CeclPolicyRecord {
    fn validate(&self) -> Result<()> {
        finstack_quant_core::validation::validate_source_metadata(
            "CECL policy",
            &self.source,
            &self.source_version,
        )?;
        finstack_quant_core::validation::validate_non_blank(
            &self.effective_date,
            "CECL policy effective date",
        )?;
        finstack_quant_core::validation::validate_f64_positive(
            self.bucket_width_years,
            "CECL bucket width years",
        )?;
        finstack_quant_core::validation::validate_f64_non_negative(
            self.forecast_horizon_years,
            "CECL forecast horizon years",
        )?;
        finstack_quant_core::validation::validate_f64_unit_interval(
            self.historical_annual_pd,
            "CECL historical annual PD",
        )?;
        finstack_quant_core::validation::validate_f64_non_negative(
            self.impaired_time_to_recovery_years,
            "CECL impaired time to recovery years",
        )?;
        if let ReversionMethod::Linear { reversion_years } = self.reversion_method {
            finstack_quant_core::validation::validate_f64_positive(
                reversion_years,
                "CECL linear reversion years",
            )?;
        }
        Ok(())
    }
}

fn default_cecl_impaired_dpd_threshold() -> u32 {
    90
}

fn default_cecl_impaired_qualitative_triggers_enabled() -> bool {
    true
}

fn default_cecl_impaired_time_to_recovery_years() -> f64 {
    1.0
}

fn default_cecl_discount_expected_losses() -> bool {
    true
}

impl Default for EclConfig {
    /// The default IFRS 9 ECL configuration from the embedded policy registry.
    fn default() -> Self {
        let policy = required_policy(registry().default_ifrs9_policy());
        ecl_config_from_policy(policy)
    }
}

fn ecl_config_from_policy(policy: &Ifrs9PolicyRecord) -> EclConfig {
    EclConfig {
        bucket_width_years: policy.ecl.bucket_width_years,
        staging: policy.staging.config(),
        lgd_type: policy.ecl.lgd_type,
        ttc_lgd: None,
        downturn_lgd: None,
        stage3_time_to_recovery_years: super::engine::DEFAULT_STAGE3_TIME_TO_RECOVERY_YEARS,
    }
}

impl Default for StagingConfig {
    /// The default IFRS 9 staging configuration from the embedded policy registry.
    fn default() -> Self {
        required_policy(registry().default_ifrs9_policy())
            .staging
            .config()
    }
}

impl Default for CeclConfig {
    /// The default CECL configuration from the embedded policy registry.
    fn default() -> Self {
        let policy = required_policy(registry().default_cecl_policy());
        cecl_config_from_policy(policy)
    }
}

fn cecl_config_from_policy(policy: &CeclPolicyRecord) -> CeclConfig {
    CeclConfig {
        bucket_width_years: policy.bucket_width_years,
        forecast_horizon_years: policy.forecast_horizon_years,
        reversion_method: policy.reversion_method,
        historical_annual_pd: policy.historical_annual_pd,
        impaired_dpd_threshold: policy.impaired_dpd_threshold,
        impaired_qualitative_triggers_enabled: policy.impaired_qualitative_triggers_enabled,
        impaired_time_to_recovery_years: policy.impaired_time_to_recovery_years,
        discount_expected_losses: policy.discount_expected_losses,
        methodology: policy.methodology,
        warm_annual_loss_rate: None,
    }
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
    EMBEDDED_REGISTRY.load(validate_registry)
}

fn validate_registry(registry: EclPolicyRegistry) -> Result<EclPolicyRegistry> {
    registry.validate()?;
    Ok(registry)
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
        let staging = StagingConfig::default();
        assert_eq!(staging.pd_delta_absolute, 0.01);
        assert_eq!(staging.pd_delta_relative, Some(2.0));
        assert_eq!(staging.dpd_stage2_threshold, 30);
        assert_eq!(staging.dpd_stage3_threshold, 90);
        assert_eq!(staging.cure_periods_stage2_to_1, 3);
        assert_eq!(staging.cure_periods_stage3_to_2, 12);

        let config = EclConfig::default();
        assert_eq!(config.bucket_width_years, 0.25);
        assert_eq!(config.lgd_type, LgdType::PointInTime);
    }

    #[test]
    fn embedded_registry_preserves_cecl_defaults() {
        let config = CeclConfig::default();
        assert_eq!(config.bucket_width_years, 0.25);
        assert_eq!(config.forecast_horizon_years, 2.0);
        assert_eq!(config.reversion_method, ReversionMethod::Immediate);
        assert_eq!(config.historical_annual_pd, 0.02);
        assert_eq!(config.methodology, CeclMethodology::PdLgdEad);
    }

    #[test]
    fn persisted_ecl_enum_values_are_snake_case() {
        assert_eq!(
            serde_json::to_string(&LgdType::PointInTime).expect("serialize LGD type"),
            "\"point_in_time\""
        );
        assert_eq!(
            serde_json::to_string(&ReversionMethod::Immediate).expect("serialize reversion method"),
            "\"immediate\""
        );
        assert_eq!(
            serde_json::to_string(&CeclMethodology::PdLgdEad).expect("serialize CECL methodology"),
            "\"pd_lgd_ead\""
        );
    }

    #[test]
    fn retired_ecl_enum_values_are_rejected() {
        // schema-rejection-test: old persisted enum spellings must stay invalid.
        assert!(serde_json::from_str::<LgdType>("\"PointInTime\"").is_err());
        assert!(serde_json::from_str::<ReversionMethod>("\"Immediate\"").is_err());
        assert!(serde_json::from_str::<CeclMethodology>("\"PdLgdEad\"").is_err());
    }
}
