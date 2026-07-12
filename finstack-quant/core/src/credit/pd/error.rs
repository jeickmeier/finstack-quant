//! Error types for PD calibration and term structure construction.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors from PD calibration and term structure construction.
#[derive(Debug, Clone, PartialEq, Error, Serialize, Deserialize)]
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

    /// Tenor must be positive.
    #[error("tenor {value} must be positive")]
    InvalidTenor {
        /// The invalid tenor value.
        value: f64,
    },

    /// No data points provided for term structure construction.
    #[error("term structure requires at least one data point")]
    EmptyTermStructure,

    /// No default state defined on the transition matrix's rating scale.
    #[error("transition matrix has no default state defined")]
    NoDefaultState,

    /// Rating not found in the transition matrix's scale.
    #[error("rating '{rating}' not found in transition matrix scale")]
    UnknownRating {
        /// The unrecognized rating label.
        rating: String,
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

    /// Central tendency calibration cannot include a zero annual default rate.
    ///
    /// No longer returned by [`central_tendency`](crate::credit::pd::central_tendency)
    /// since it switched to the arithmetic long-run average (zero-default years
    /// are valid observations); retained for serde stability.
    #[error("central_tendency requires strictly positive annual default rates, found 0.0")]
    ZeroAnnualDefaultRate,

    /// Cumulative PDs are not monotonically non-decreasing after isotonic regression.
    #[error("cumulative PDs are not non-decreasing after isotonic regression")]
    NonMonotonicCumulativePds,

    /// A requested tenor is not an integer multiple of the transition
    /// matrix's horizon, so matrix powers cannot reach it exactly.
    #[error(
        "tenor {tenor} is not an integer multiple of the transition matrix horizon {horizon}; \
         for non-integer multiples extract a generator (GeneratorMatrix::from_transition_matrix) \
         and use the generator-based `project` instead"
    )]
    TenorNotMultipleOfHorizon {
        /// The requested tenor in years.
        tenor: f64,
        /// The transition matrix horizon in years.
        horizon: f64,
    },
}
