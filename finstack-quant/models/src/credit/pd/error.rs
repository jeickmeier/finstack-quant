//! Error types for PD calibration and term structure construction.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors from PD calibration and term structure construction.
#[derive(Debug, Clone, PartialEq, Error, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PdCalibrationError {
    /// PD value is not in the valid range (0, 1).
    #[error("PD value {value} is outside (0, 1)")]
    PdOutOfRange {
        /// The invalid PD value.
        value: f64,
    },

    /// Asset correlation is not in the valid range (0, 1).
    #[error("asset correlation {value} is outside (0, 1)")]
    InvalidCorrelation {
        /// The invalid correlation value.
        value: f64,
    },

    /// Empty input where at least one value is required.
    #[error("empty input: at least one value is required")]
    EmptyInput,

    /// A value in the input is outside the expected range.
    #[error("value {value} is outside [{min}, {max}]")]
    ValueOutOfRange {
        /// The offending value.
        value: f64,
        /// Minimum allowed value.
        min: f64,
        /// Maximum allowed value.
        max: f64,
    },

    /// Grades in a master scale are not properly ordered.
    #[error("master scale grades must have ascending upper_pd values")]
    GradesNotSorted,

    /// A non-finite value was encountered.
    #[error("non-finite value encountered: {value}")]
    NonFiniteValue {
        /// The non-finite value.
        value: f64,
    },

    /// A scoring result has no calibrated or native probability.
    #[error("scoring result has no implied PD; request an explicit calibration first")]
    MissingImpliedPd,
}

impl PdCalibrationError {
    /// Classify this error for host-language exception mapping.
    ///
    /// Every variant rejects a caller-supplied probability, correlation,
    /// default-rate series or master-scale definition, so the kind is always
    /// validation.
    #[must_use]
    pub fn kind(&self) -> finstack_quant_core::error::ErrorKind {
        finstack_quant_core::error::ErrorKind::Validation
    }
}
