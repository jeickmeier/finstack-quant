//! Registry for external credit assumptions that are expected to change over time.

use crate::collections::HashMap;
use crate::config::FinstackConfig;
use crate::credit::lgd::seniority::{BetaRecovery, SeniorityCalibration, SeniorityClass};
use crate::credit::pd::MasterScaleGrade;
use crate::embedded_registry::EmbeddedJsonRegistry;
use crate::types::CreditRating;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Configuration extension key for replacing the embedded credit assumptions registry.
pub const CREDIT_ASSUMPTIONS_EXTENSION_KEY: &str = "core.credit_assumptions.v1";

static EMBEDDED_REGISTRY: EmbeddedJsonRegistry<CreditAssumptionRegistry> =
    EmbeddedJsonRegistry::new(
        include_str!("../../data/credit/credit_assumptions.v1.json"),
        CREDIT_ASSUMPTIONS_EXTENSION_KEY,
        "credit assumptions",
    );

/// Versioned credit-assumption registry loaded from JSON.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreditAssumptionRegistry {
    schema_version: String,
    default_rating_factor_table_id: String,
    default_seniority_calibration_id: String,
    default_pd_master_scale_id: String,
    default_downturn_lgd_id: String,
    default_workout_lgd_id: String,
    default_market_recovery_rate: f64,
    rating_factor_tables: Vec<RatingFactorTableRecord>,
    seniority_calibrations: Vec<SeniorityCalibrationRecord>,
    pd_master_scales: Vec<PdMasterScaleRecord>,
    downturn_lgd_presets: Vec<DownturnLgdPresetRecord>,
    workout_lgd_defaults: Vec<WorkoutLgdDefaultsRecord>,
}

impl CreditAssumptionRegistry {
    /// Returns the default WARF factor table id.
    pub fn default_rating_factor_table_id(&self) -> &str {
        &self.default_rating_factor_table_id
    }

    /// Returns the default seniority recovery calibration id.
    pub fn default_seniority_calibration_id(&self) -> &str {
        &self.default_seniority_calibration_id
    }

    /// Returns the default PD master scale id.
    pub fn default_pd_master_scale_id(&self) -> &str {
        &self.default_pd_master_scale_id
    }

    /// Returns the default downturn LGD preset id.
    pub fn default_downturn_lgd_id(&self) -> &str {
        &self.default_downturn_lgd_id
    }

    /// Returns the default workout LGD preset id.
    pub fn default_workout_lgd_id(&self) -> &str {
        &self.default_workout_lgd_id
    }

    /// Returns the default market recovery-rate assumption.
    #[must_use]
    pub fn default_market_recovery_rate(&self) -> f64 {
        self.default_market_recovery_rate
    }

    pub(crate) fn rating_factor_table(&self, id: &str) -> Result<RatingFactorTableParts> {
        let record = self
            .rating_factor_tables
            .iter()
            .find(|record| record.ids.iter().any(|c| c == id))
            .ok_or_else(|| not_found("rating factor table", id))?;
        let factors = record
            .factors
            .iter()
            .map(|f| (f.rating, f.factor))
            .collect();
        Ok(RatingFactorTableParts {
            factors,
            agency: record.agency.clone(),
            methodology: record.methodology.clone(),
            default_factor: record.default_factor,
        })
    }

    pub(crate) fn seniority_calibration(&self, id: &str) -> Result<SeniorityCalibration> {
        let record = self
            .seniority_calibrations
            .iter()
            .find(|record| record.ids.iter().any(|c| c == id))
            .ok_or_else(|| not_found("seniority calibration", id))?;
        let classes = record
            .classes
            .iter()
            .map(|c| Ok((c.seniority, BetaRecovery::new(c.mean, c.std_dev)?)))
            .collect::<Result<Vec<_>>>()?;
        Ok(SeniorityCalibration {
            source: record.source.clone(),
            classes,
        })
    }

    pub(crate) fn pd_master_scale_grades(&self, id: &str) -> Result<Vec<MasterScaleGrade>> {
        let record = self
            .pd_master_scales
            .iter()
            .find(|record| {
                record.ids.iter().any(|candidate| candidate == id)
                    || record
                        .deprecated_ids
                        .iter()
                        .any(|candidate| candidate == id)
            })
            .ok_or_else(|| not_found("PD master scale", id))?;
        if record
            .deprecated_ids
            .iter()
            .any(|candidate| candidate == id)
        {
            tracing::warn!(
                deprecated_id = id,
                canonical_id = first_id(&record.ids),
                "deprecated PD master-scale registry id resolved"
            );
        }
        Ok(record
            .grades
            .iter()
            .map(|grade| MasterScaleGrade {
                label: grade.label.clone(),
                upper_pd: grade.upper_pd,
                central_pd: grade.central_pd,
            })
            .collect())
    }

    pub(crate) fn downturn_lgd_preset(&self, id: &str) -> Result<DownturnLgdPreset> {
        let record = self
            .downturn_lgd_presets
            .iter()
            .find(|record| record.ids.iter().any(|c| c == id))
            .ok_or_else(|| not_found("downturn LGD preset", id))?;
        Ok(DownturnLgdPreset {
            method: record.method.clone(),
            add_on: record.add_on,
            floor: record.floor,
        })
    }

    pub(crate) fn workout_lgd_defaults(&self, id: &str) -> Result<WorkoutLgdDefaults> {
        let record = self
            .workout_lgd_defaults
            .iter()
            .find(|record| record.ids.iter().any(|c| c == id))
            .ok_or_else(|| not_found("workout LGD defaults", id))?;
        Ok(WorkoutLgdDefaults {
            workout_years: record.workout_years,
            discount_rate: record.discount_rate,
            direct_cost_rate: record.direct_cost_rate,
            indirect_cost_rate: record.indirect_cost_rate,
        })
    }

    fn validate(&self) -> Result<()> {
        if self.schema_version != "finstack_quant.credit_assumptions/1" {
            return Err(Error::Validation(format!(
                "unsupported credit assumptions schema version '{}'",
                self.schema_version
            )));
        }

        validate_ids(
            "rating factor table",
            self.rating_factor_tables
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;
        validate_ids(
            "seniority calibration",
            self.seniority_calibrations
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;
        validate_pd_master_scale_ids(&self.pd_master_scales)?;
        validate_ids(
            "downturn LGD preset",
            self.downturn_lgd_presets
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;
        validate_ids(
            "workout LGD defaults",
            self.workout_lgd_defaults
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;

        self.rating_factor_table(&self.default_rating_factor_table_id)?;
        self.seniority_calibration(&self.default_seniority_calibration_id)?;
        self.pd_master_scale_grades(&self.default_pd_master_scale_id)?;
        self.downturn_lgd_preset(&self.default_downturn_lgd_id)?;
        self.workout_lgd_defaults(&self.default_workout_lgd_id)?;
        validate_unit_interval(
            self.default_market_recovery_rate,
            "default market recovery rate",
        )?;

        for record in &self.rating_factor_tables {
            if record.default_factor < 0.0 || !record.default_factor.is_finite() {
                return Err(Error::Validation(format!(
                    "rating factor table '{}' has invalid default factor {}",
                    first_id(&record.ids),
                    record.default_factor
                )));
            }
            for factor in &record.factors {
                if factor.factor < 0.0 || !factor.factor.is_finite() {
                    return Err(Error::Validation(format!(
                        "rating factor table '{}' has invalid factor {} for {:?}",
                        first_id(&record.ids),
                        factor.factor,
                        factor.rating
                    )));
                }
            }
        }

        for record in &self.pd_master_scales {
            for grade in &record.grades {
                if grade.upper_pd <= 0.0
                    || grade.upper_pd > 1.0
                    || grade.central_pd <= 0.0
                    || grade.central_pd > 1.0
                    || grade.central_pd > grade.upper_pd
                {
                    return Err(Error::Validation(format!(
                        "PD master scale '{}' has invalid grade '{}'",
                        first_id(&record.ids),
                        grade.label
                    )));
                }
            }
        }

        for record in &self.downturn_lgd_presets {
            if record.method != "regulatory_floor" {
                return Err(Error::Validation(format!(
                    "downturn LGD preset '{}' has unsupported method '{}'",
                    first_id(&record.ids),
                    record.method
                )));
            }
            validate_unit_interval(record.add_on, "downturn LGD add-on")?;
            validate_unit_interval(record.floor, "downturn LGD floor")?;
        }

        for record in &self.workout_lgd_defaults {
            if record.workout_years <= 0.0 || !record.workout_years.is_finite() {
                return Err(Error::Validation(format!(
                    "workout LGD defaults '{}' has invalid workout years {}",
                    first_id(&record.ids),
                    record.workout_years
                )));
            }
            validate_unit_interval(record.discount_rate, "workout discount rate")?;
            validate_unit_interval(record.direct_cost_rate, "direct workout cost rate")?;
            validate_unit_interval(record.indirect_cost_rate, "indirect workout cost rate")?;
        }

        Ok(())
    }
}

/// Load the embedded versioned registry of credit assumptions.
///
/// The registry supplies the library defaults for WARF tables, recovery
/// calibrations, PD master scales, downturn/workout LGD presets, and market
/// recovery. It is parsed and validated lazily, then cached for subsequent
/// callers; consumers should select an explicit named entry when a governing
/// policy requires a methodology other than the embedded default.
///
/// # Errors
///
/// Returns [`Error::Validation`] if the bundled JSON cannot be parsed or fails
/// its schema/version, identifier uniqueness, default-reference, probability,
/// recovery, or calibration validation. An error represents a package defect,
/// not missing market data that can safely be projected at runtime.
pub fn embedded_registry() -> Result<&'static CreditAssumptionRegistry> {
    EMBEDDED_REGISTRY.load(validate_registry)
}

/// Return the embedded default market recovery rate.
///
/// Returns `Err` if the embedded credit-assumptions JSON fails to parse or
/// validate. This is the preferred entry point for fallible call sites and
/// for any new code; existing infallible builders use
/// `default_market_recovery_rate_or_panic`. The value is a unit-interval
/// recovery assumption (for example, `0.40` means 40% recovery), not LGD; a
/// caller calculates LGD as `1 - recovery` only when that is the relevant
/// modeling convention.
///
/// # Errors
///
/// Propagates registry-loading and validation errors from
/// [`embedded_registry`]. A successful result is guaranteed finite and inside
/// `[0, 1]` by registry validation.
pub fn default_market_recovery_rate() -> Result<f64> {
    Ok(embedded_registry()?.default_market_recovery_rate())
}

/// Return the embedded default market recovery rate for infallible builders.
///
/// # Safety invariant
///
/// The embedded credit-assumptions JSON is shipped as a compile-time asset
/// and validated lazily on first access. The unit-test
/// `embedded_registry_loads_expected_defaults` (see below) and the integration
/// test `default_market_recovery_rate_or_panic_succeeds_for_embedded_asset`
/// both load the embedded registry through the same code path used here, so
/// a malformed or missing asset is guaranteed to fail in CI before this
/// function can panic at runtime.
///
/// Prefer [`default_market_recovery_rate`] in any code that already returns
/// `Result`; this variant exists solely for builder constructors whose
/// public signatures must remain infallible.
#[must_use]
#[allow(clippy::expect_used)]
pub(crate) fn default_market_recovery_rate_or_panic() -> f64 {
    embedded_registry()
        .expect("embedded credit assumptions registry is a compile-time asset")
        .default_market_recovery_rate()
}

/// Load a credit-assumptions registry from configuration or the embedded fallback.
///
/// A value under [`CREDIT_ASSUMPTIONS_EXTENSION_KEY`] replaces every embedded
/// default after strict registry validation. Without that extension, this
/// returns a clone of the cached embedded registry, so callers can own their
/// selected assumptions without mutating global state.
///
/// # Errors
///
/// Returns [`Error::Validation`] if a configured extension exists but is
/// malformed or violates schema, ID, default-reference, probability, or
/// recovery/calibration invariants. Invalid configured data does not silently
/// fall back to the embedded registry, because that would conceal a material
/// credit-model configuration error.
///
/// # Arguments
///
/// * `config` - Library configuration that may contain a validated credit
///   assumption-registry extension; otherwise the embedded registry is cloned.
pub fn registry_from_config(config: &FinstackConfig) -> Result<CreditAssumptionRegistry> {
    EMBEDDED_REGISTRY.load_from_config(config, validate_registry)
}

fn validate_registry(registry: CreditAssumptionRegistry) -> Result<CreditAssumptionRegistry> {
    registry.validate()?;
    Ok(registry)
}

fn first_id(ids: &[String]) -> &str {
    ids.first().map_or("<missing>", String::as_str)
}

fn not_found(kind: &str, id: &str) -> Error {
    Error::Validation(format!(
        "credit assumptions registry does not contain {kind} '{id}'"
    ))
}

fn validate_ids<'a>(kind: &str, records: impl Iterator<Item = &'a [String]>) -> Result<()> {
    let mut seen = BTreeSet::new();
    for ids in records {
        if ids.is_empty() {
            return Err(Error::Validation(format!(
                "credit assumptions registry contains {kind} without an id"
            )));
        }
        for id in ids {
            if id.trim().is_empty() {
                return Err(Error::Validation(format!(
                    "credit assumptions registry contains blank {kind} id"
                )));
            }
            if !seen.insert(id.clone()) {
                return Err(Error::Validation(format!(
                    "credit assumptions registry contains duplicate {kind} id '{id}'"
                )));
            }
        }
    }
    Ok(())
}

fn validate_pd_master_scale_ids(records: &[PdMasterScaleRecord]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for record in records {
        if record.ids.is_empty() {
            return Err(Error::Validation(
                "credit assumptions registry contains PD master scale without an id".to_string(),
            ));
        }
        for id in record.ids.iter().chain(&record.deprecated_ids) {
            if id.trim().is_empty() {
                return Err(Error::Validation(
                    "credit assumptions registry contains blank PD master scale id".to_string(),
                ));
            }
            if !seen.insert(id.clone()) {
                return Err(Error::Validation(format!(
                    "credit assumptions registry contains duplicate PD master scale id '{id}'"
                )));
            }
        }
    }
    Ok(())
}

fn validate_unit_interval(value: f64, label: &str) -> Result<()> {
    if (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(Error::Validation(format!(
            "credit assumptions registry has invalid {label} {value}"
        )))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RatingFactorTableParts {
    pub(crate) factors: HashMap<CreditRating, f64>,
    pub(crate) agency: String,
    pub(crate) methodology: String,
    pub(crate) default_factor: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct DownturnLgdPreset {
    pub(crate) method: String,
    pub(crate) add_on: f64,
    pub(crate) floor: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct WorkoutLgdDefaults {
    pub(crate) workout_years: f64,
    pub(crate) discount_rate: f64,
    pub(crate) direct_cost_rate: f64,
    pub(crate) indirect_cost_rate: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RatingFactorTableRecord {
    ids: Vec<String>,
    agency: String,
    methodology: String,
    source: String,
    source_version: String,
    effective_date: String,
    default_factor: f64,
    factors: Vec<RatingFactorRecord>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
struct RatingFactorRecord {
    rating: CreditRating,
    factor: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SeniorityCalibrationRecord {
    ids: Vec<String>,
    source: String,
    #[serde(default)]
    study_period: Option<StudyPeriod>,
    classes: Vec<SeniorityClassRecord>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
struct SeniorityClassRecord {
    seniority: SeniorityClass,
    mean: f64,
    std_dev: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PdMasterScaleRecord {
    ids: Vec<String>,
    #[serde(default)]
    deprecated_ids: Vec<String>,
    source: String,
    #[serde(default)]
    study_period: Option<StudyPeriod>,
    grades: Vec<PdMasterScaleGradeRecord>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PdMasterScaleGradeRecord {
    label: String,
    upper_pd: f64,
    central_pd: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct DownturnLgdPresetRecord {
    ids: Vec<String>,
    method: String,
    add_on: f64,
    floor: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct WorkoutLgdDefaultsRecord {
    ids: Vec<String>,
    workout_years: f64,
    discount_rate: f64,
    direct_cost_rate: f64,
    indirect_cost_rate: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct StudyPeriod {
    start_year: u16,
    end_year: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_registry_loads_expected_defaults() {
        let registry = embedded_registry().expect("embedded registry should load");
        assert_eq!(registry.default_rating_factor_table_id(), "moodys_standard");
        assert_eq!(
            registry.default_seniority_calibration_id(),
            "moodys_recovery_1982_2023"
        );
        assert_eq!(registry.default_pd_master_scale_id(), "sp_assumptions_v1");
        assert_eq!(registry.default_downturn_lgd_id(), "basel_secured");
        assert_eq!(registry.default_workout_lgd_id(), "standard_workout");
    }

    #[test]
    fn registry_preserves_known_agency_values() {
        let registry = embedded_registry().expect("embedded registry should load");
        let warf = registry
            .rating_factor_table("moodys_standard")
            .expect("WARF table should exist");
        assert_eq!(warf.factors.get(&CreditRating::B), Some(&2720.0));

        let seniority = registry
            .seniority_calibration("sp")
            .expect("S&P recovery table should exist");
        let senior_secured = seniority
            .classes
            .iter()
            .find(|(class, _)| *class == SeniorityClass::SeniorSecured)
            .expect("senior secured class should exist");
        assert!((senior_secured.1.mean() - 0.53).abs() < 1e-12);
    }

    #[test]
    fn config_extension_loads_registry_schema() {
        let embedded = embedded_registry()
            .expect("embedded registry should load")
            .clone();
        let value = serde_json::to_value(&embedded).expect("registry should serialize");
        let mut config = FinstackConfig::default();
        config
            .extensions
            .insert(CREDIT_ASSUMPTIONS_EXTENSION_KEY, value)
            .expect("valid extension key");

        let loaded = registry_from_config(&config).expect("config registry should load");
        assert_eq!(
            loaded.default_rating_factor_table_id(),
            embedded.default_rating_factor_table_id()
        );
    }

    #[test]
    fn config_accepts_deprecated_pd_master_scale_defaults() {
        for legacy_id in [
            "sp_empirical",
            "sp_corporate_default_1981_2023",
            "moodys_empirical",
            "moodys_default_1983_2023",
        ] {
            let mut registry = embedded_registry()
                .expect("embedded registry should load")
                .clone();
            registry.default_pd_master_scale_id = legacy_id.to_string();
            registry
                .validate()
                .expect("deprecated default id should remain valid");

            let value = serde_json::to_value(&registry).expect("registry should serialize");
            let mut config = FinstackConfig::default();
            config
                .extensions
                .insert(CREDIT_ASSUMPTIONS_EXTENSION_KEY, value)
                .expect("valid extension key");
            let loaded = registry_from_config(&config).expect("legacy config should load");
            assert_eq!(loaded.default_pd_master_scale_id(), legacy_id);
        }
    }

    #[test]
    fn registry_rejects_zero_pd_upper_bound() {
        let mut registry = embedded_registry()
            .expect("embedded registry should load")
            .clone();
        registry.pd_master_scales[0].grades[0].upper_pd = 0.0;

        let err = registry
            .validate()
            .expect_err("zero upper PD should fail validation");
        assert!(
            err.to_string().contains("invalid grade"),
            "unexpected error: {err}"
        );
    }
}
