//! Embedded calibration defaults.

use std::sync::OnceLock;

use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::{Error, Result};
use serde::Deserialize;

/// Config extension key for overriding calibration defaults.
pub const CALIBRATION_DEFAULTS_EXTENSION_KEY: &str = "valuations.calibration_defaults.v1";

const CALIBRATION_DEFAULTS: &str =
    include_str!("../../data/calibration/calibration_defaults.v1.json");

static EMBEDDED_DEFAULTS: OnceLock<Result<CalibrationDefaults>> = OnceLock::new();

/// Resolved calibration defaults.
#[derive(Debug, Clone)]
pub struct CalibrationDefaults {
    /// Validation defaults.
    pub validation: CalibrationValidationDefaults,
}

/// Defaults used by calibration preflight validation.
#[derive(Debug, Clone, Deserialize)]
pub struct CalibrationValidationDefaults {
    /// Absolute tolerance used when comparing configured and quoted recovery rates.
    pub recovery_rate_abs_tolerance: f64,
    /// Minimum LGD denominator used for hazard-rate initial guesses.
    pub minimum_lgd_for_hazard_guess: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DefaultsFile {
    schema: Option<String>,
    version: Option<u32>,
    validation: CalibrationValidationDefaults,
}

/// Return the embedded calibration defaults.
pub fn embedded_defaults() -> Result<&'static CalibrationDefaults> {
    match EMBEDDED_DEFAULTS.get_or_init(parse_embedded_defaults) {
        Ok(defaults) => Ok(defaults),
        Err(err) => Err(err.clone()),
    }
}

/// Loads calibration defaults from configuration or falls back to embedded defaults.
pub fn defaults_from_config(config: &FinstackConfig) -> Result<CalibrationDefaults> {
    if let Some(value) = config.extensions.get(CALIBRATION_DEFAULTS_EXTENSION_KEY) {
        let file: DefaultsFile = serde_json::from_value(value.clone()).map_err(|err| {
            Error::Validation(format!(
                "failed to parse calibration defaults extension: {err}"
            ))
        })?;
        defaults_from_file(file)
    } else {
        Ok(embedded_defaults()?.clone())
    }
}

fn parse_embedded_defaults() -> Result<CalibrationDefaults> {
    let file: DefaultsFile = serde_json::from_str(CALIBRATION_DEFAULTS).map_err(|err| {
        Error::Validation(format!(
            "failed to parse embedded calibration defaults: {err}"
        ))
    })?;
    defaults_from_file(file)
}

fn defaults_from_file(file: DefaultsFile) -> Result<CalibrationDefaults> {
    validate_file(&file)?;
    Ok(CalibrationDefaults {
        validation: file.validation,
    })
}

fn validate_file(file: &DefaultsFile) -> Result<()> {
    let _schema = &file.schema;
    let _version = file.version;
    validate_nonnegative_finite(
        "validation.recovery_rate_abs_tolerance",
        file.validation.recovery_rate_abs_tolerance,
    )?;
    validate_positive_finite(
        "validation.minimum_lgd_for_hazard_guess",
        file.validation.minimum_lgd_for_hazard_guess,
    )?;
    Ok(())
}

fn validate_nonnegative_finite(label: &str, value: f64) -> Result<()> {
    if !value.is_finite() || value < 0.0 {
        return Err(Error::Validation(format!(
            "{label} must be finite and non-negative"
        )));
    }
    Ok(())
}

fn validate_positive_finite(label: &str, value: f64) -> Result<()> {
    if !value.is_finite() || value <= 0.0 {
        return Err(Error::Validation(format!(
            "{label} must be finite and positive"
        )));
    }
    Ok(())
}
