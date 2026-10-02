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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
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
    pub model: String,
}

/// Zone classification across all scoring models.
///
/// Represents the risk category derived from a model's score:
/// - `Safe`: low bankruptcy probability.
/// - `Grey`: ambiguous / requires further analysis.
/// - `Distress`: high bankruptcy probability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
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
#[serde(rename_all = "snake_case")]
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

    /// A binary indicator was not exactly zero or one.
    #[error("input field '{field}' must be exactly 0 or 1, got {value}")]
    InvalidBinaryIndicator {
        /// Name of the offending field.
        field: &'static str,
        /// The invalid indicator.
        value: f64,
    },
}

impl CreditScoringError {
    /// Classify this error for host-language exception mapping.
    ///
    /// Every variant rejects a caller-supplied financial ratio or indicator,
    /// so the kind is always validation.
    #[must_use]
    pub fn kind(&self) -> finstack_quant_core::error::ErrorKind {
        finstack_quant_core::error::ErrorKind::Validation
    }
}

/// Validate that a value is finite, returning `CreditScoringError::NonFiniteInput` if not.
pub(crate) fn check_finite(field: &'static str, value: f64) -> Result<(), CreditScoringError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(CreditScoringError::NonFiniteInput { field, value })
    }
}
