//! Shared types for academic credit scoring models.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Result from any academic scoring model.
///
/// Provides a uniform interface across Altman Z-Score, Ohlson O-Score,
/// and Zmijewski probit models. The `score` field contains the raw
/// discriminant or regression output, `zone` classifies credit risk,
/// and `implied_pd` contains a probability only when the model has a native
/// probability transform or an explicit calibration was requested.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringResult {
    /// The raw score value (Z, Z', Z'', O, or Zmijewski Y).
    pub score: f64,
    /// Risk zone classification (Safe/Grey/Distress).
    pub zone: ScoringZone,
    /// Optional implied probability of default.
    ///
    /// Altman score results leave this as `None` unless an explicit,
    /// versioned heuristic calibration is requested. Ohlson and Zmijewski
    /// retain their native logistic and probit probabilities.
    pub implied_pd: Option<f64>,
    /// Name of the model that produced this result.
    pub model: &'static str,
}

/// Zone classification across all scoring models.
///
/// Represents the risk category derived from a model's score:
/// - `Safe`: low bankruptcy probability.
/// - `Grey`: ambiguous / requires further analysis.
/// - `Distress`: high bankruptcy probability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScoringZone {
    /// Safe zone (low bankruptcy probability).
    Safe,
    /// Grey zone (ambiguous).
    Grey,
    /// Distress zone (high bankruptcy probability).
    Distress,
}

/// Errors from credit scoring model computation.
#[derive(Debug, Clone, PartialEq, Error, Serialize, Deserialize)]
#[non_exhaustive]
pub enum CreditScoringError {
    /// An input ratio is NaN or infinite.
    #[error("input field '{field}' is not finite: {value}")]
    NonFiniteInput {
        /// Name of the offending field.
        field: &'static str,
        /// The non-finite value.
        value: f64,
    },

    /// An input value is outside the valid domain for the model.
    #[error("input field '{field}' = {value} is outside valid range [{min}, {max}]")]
    OutOfRange {
        /// Name of the offending field.
        field: &'static str,
        /// The out-of-range value.
        value: f64,
        /// Minimum allowed value (inclusive).
        min: f64,
        /// Maximum allowed value (inclusive).
        max: f64,
    },

    /// A binary indicator was not exactly zero or one.
    #[error("input field '{field}' must be exactly 0 or 1, got {value}")]
    InvalidBinaryIndicator {
        /// Name of the offending field.
        field: &'static str,
        /// The invalid indicator.
        value: f64,
    },
}

/// Validate that a value is finite, returning `CreditScoringError::NonFiniteInput` if not.
pub(crate) fn check_finite(field: &'static str, value: f64) -> Result<(), CreditScoringError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(CreditScoringError::NonFiniteInput { field, value })
    }
}
