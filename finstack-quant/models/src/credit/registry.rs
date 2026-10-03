//! Registry for external credit assumptions that are expected to change over time.

use crate::credit::lgd::seniority::{BetaRecovery, SeniorityCalibration, SeniorityClass};
use crate::credit::pd::MasterScaleGrade;
use finstack_quant_core::embedded_registry::EmbeddedJsonRegistry;
use finstack_quant_core::types::CreditRating;
use finstack_quant_core::{Error, HashMap, InputError, Result};
use serde::{Deserialize, Serialize};

static EMBEDDED_REGISTRY: EmbeddedJsonRegistry<CreditAssumptionRegistry> =
    EmbeddedJsonRegistry::new(
        include_str!("../../data/credit/credit_assumptions.v1.json"),
        None,
        "credit assumptions",
    );

/// Versioned credit-assumption registry loaded from JSON.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CreditAssumptionRegistry {
    schema: String,
    default_rating_factor_table_id: String,
    default_seniority_calibration_id: String,
    default_pd_master_scale_id: String,
    default_downturn_lgd_id: String,
    default_workout_lgd_id: String,
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
            .find(|record| record.ids.iter().any(|candidate| candidate == id))
            .ok_or_else(|| not_found("PD master scale", id))?;
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
        if self.schema != "finstack_quant.credit_assumptions/1" {
            return Err(Error::Validation(format!(
                "unsupported credit assumptions schema version '{}'",
                self.schema
            )));
        }

        finstack_quant_core::validation::validate_unique_ids(
            "credit assumptions registry",
            "rating factor table",
            self.rating_factor_tables
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;
        finstack_quant_core::validation::validate_unique_ids(
            "credit assumptions registry",
            "seniority calibration",
            self.seniority_calibrations
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;
        finstack_quant_core::validation::validate_unique_ids(
            "credit assumptions registry",
            "PD master scale",
            self.pd_master_scales
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;
        finstack_quant_core::validation::validate_unique_ids(
            "credit assumptions registry",
            "downturn LGD preset",
            self.downturn_lgd_presets
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;
        finstack_quant_core::validation::validate_unique_ids(
            "credit assumptions registry",
            "workout LGD defaults",
            self.workout_lgd_defaults
                .iter()
                .map(|record| record.ids.as_slice()),
        )?;

        // A default id that names no record is a malformed registry, not a
        // caller lookup miss, so it stays a validation error.
        let missing_default = |err: Error| Error::Validation(format!("invalid default: {err}"));
        self.rating_factor_table(&self.default_rating_factor_table_id)
            .map_err(missing_default)?;
        self.seniority_calibration(&self.default_seniority_calibration_id)
            .map_err(missing_default)?;
        self.pd_master_scale_grades(&self.default_pd_master_scale_id)
            .map_err(missing_default)?;
        self.downturn_lgd_preset(&self.default_downturn_lgd_id)
            .map_err(missing_default)?;
        self.workout_lgd_defaults(&self.default_workout_lgd_id)
            .map_err(missing_default)?;
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
            finstack_quant_core::validation::validate_f64_unit_interval(
                record.add_on,
                "downturn LGD add-on",
            )?;
            finstack_quant_core::validation::validate_f64_unit_interval(
                record.floor,
                "downturn LGD floor",
            )?;
        }

        for record in &self.workout_lgd_defaults {
            if record.workout_years <= 0.0 || !record.workout_years.is_finite() {
                return Err(Error::Validation(format!(
                    "workout LGD defaults '{}' has invalid workout years {}",
                    first_id(&record.ids),
                    record.workout_years
                )));
            }
            finstack_quant_core::validation::validate_f64_unit_interval(
                record.discount_rate,
                "workout discount rate",
            )?;
            finstack_quant_core::validation::validate_f64_unit_interval(
                record.direct_cost_rate,
                "direct workout cost rate",
            )?;
            finstack_quant_core::validation::validate_f64_unit_interval(
                record.indirect_cost_rate,
                "indirect workout cost rate",
            )?;
        }

        Ok(())
    }
}

/// Load the embedded versioned registry of credit assumptions.
///
/// The registry supplies the library defaults for WARF tables, recovery
/// calibrations, PD master scales, and downturn/workout LGD presets. It is
/// parsed and validated lazily, then cached for subsequent
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

fn validate_registry(registry: CreditAssumptionRegistry) -> Result<CreditAssumptionRegistry> {
    registry.validate()?;
    Ok(registry)
}

fn first_id(ids: &[String]) -> &str {
    ids.first().map_or("<missing>", String::as_str)
}

/// Lookup miss for `id` in the `kind` section of the registry. Its kind is
/// [`ErrorKind::NotFound`](finstack_quant_core::error::ErrorKind::NotFound), so
/// hosts raise their missing-key error (`KeyError` / `not_found`).
fn not_found(kind: &str, id: &str) -> Error {
    InputError::NotFound {
        id: format!("credit assumptions registry {kind} '{id}'"),
    }
    .into()
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
        assert_eq!(registry.default_pd_master_scale_id(), "sp_assumptions");
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
    fn unknown_registry_id_is_a_not_found_error() {
        let registry = embedded_registry().expect("embedded registry should load");
        let err = registry
            .rating_factor_table("nope")
            .expect_err("unknown id should fail");
        assert_eq!(err.kind(), finstack_quant_core::error::ErrorKind::NotFound);
        assert!(
            err.to_string().contains("rating factor table 'nope'"),
            "unexpected error: {err}"
        );
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
