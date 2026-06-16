//! Input validation error types.
//!
//! This module defines [`InputError`], which captures all validation errors
//! related to user-supplied data. This includes structural issues, value
//! constraints, date/calendar problems, and missing references.

use crate::currency::Currency;
use crate::dates::BusinessDayConvention;
use time::Date;

use super::suggestions::format_suggestions;

/// Classification of a non-finite floating-point value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NonFiniteKind {
    /// Not-a-number.
    NaN,
    /// Positive infinity.
    PosInfinity,
    /// Negative infinity.
    NegInfinity,
}

impl core::fmt::Display for NonFiniteKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            NonFiniteKind::NaN => write!(f, "NaN"),
            NonFiniteKind::PosInfinity => write!(f, "infinity"),
            NonFiniteKind::NegInfinity => write!(f, "-infinity"),
        }
    }
}

/// Detailed user input validation failures.
///
/// This enum captures all validation errors related to user-supplied data,
/// including structural issues (too few points, non-monotonic sequences),
/// value constraints (negative/non-positive values), date/calendar problems,
/// and missing references in market data contexts.
///
/// # Variants
///
/// Each variant provides specific context about the validation failure to
/// enable actionable error messages and recovery logic.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::InputError;
///
/// // Too few data points
/// let err = InputError::TooFewPoints;
/// assert_eq!(err.to_string(), "At least two data points are required");
///
/// // Non-monotonic sequence
/// let err = InputError::NonMonotonicKnots;
/// assert_eq!(err.to_string(), "Times (knots) must be strictly increasing");
/// ```
#[derive(Debug, Clone, PartialEq, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum InputError {
    // ─────────────────────────────────────────────────────────────────────────
    // Basic Validation
    // ─────────────────────────────────────────────────────────────────────────
    /// Input must contain at least two distinct points (e.g. knots on a curve).
    #[error("At least two data points are required")]
    TooFewPoints,

    /// Knot/point times are not strictly increasing.
    #[error("Times (knots) must be strictly increasing")]
    NonMonotonicKnots,

    /// Encountered a value that must be strictly positive (e.g. discount factor).
    #[error("Values must be positive")]
    NonPositiveValue,

    /// Encountered a negative value where only non-negative values are allowed (e.g. hazard rate).
    #[error("Values must be non-negative")]
    NegativeValue,

    /// Shape/dimension mismatch in matrix-like input (e.g. vol surface grid).
    #[error("Input dimensions do not match")]
    DimensionMismatch,

    /// Fallback for miscellaneous validation problems not yet covered by a specific variant.
    #[error("Invalid input data")]
    Invalid,

    /// Requested allocation or data structure exceeds configured limit.
    #[error("Allocation too large for {what}: requested {requested_bytes} bytes, limit {limit_bytes} bytes")]
    TooLarge {
        /// Description of what exceeded the limit (e.g., "expression arena").
        what: String,
        /// Number of bytes requested.
        requested_bytes: usize,
        /// Configured limit in bytes.
        limit_bytes: usize,
    },

    /// Consecutive knots are too close together for stable interpolation.
    #[error("Consecutive knots are too close together for stable interpolation")]
    KnotSpacingTooSmall,

    // ─────────────────────────────────────────────────────────────────────────
    // Date and Calendar
    // ─────────────────────────────────────────────────────────────────────────
    /// The provided date range is inverted – the start date is after the end date.
    #[error("Invalid date range: start must be before end")]
    InvalidDateRange,

    /// Schedule range is invalid: start must be strictly before end.
    #[error("Invalid schedule range: start ({start}) must be before end ({end})")]
    InvalidScheduleRange {
        /// Schedule start date.
        start: time::Date,
        /// Schedule end date.
        end: time::Date,
    },

    /// `StubKind::None` was requested but the tenor does not divide evenly
    /// into the schedule range. Use `ShortBack` or `ShortFront` instead.
    #[error(
        "StubKind::None requires the tenor to divide evenly into the schedule period, \
         but the last generated date overshoots the end; use ShortBack or ShortFront"
    )]
    NonIntegerScheduleTenor,

    /// Invalid calendar date creation (e.g., February 30th).
    #[error("Invalid calendar date: {year}-{month:02}-{day:02}")]
    InvalidDate {
        /// Year component
        year: i32,
        /// Month component (1-12)
        month: u8,
        /// Day component (1-31)
        day: u8,
    },

    /// No business day found within maximum allowed adjustment period.
    #[error("Business day adjustment failed: no business day found within {max_days} days from {date} using {convention:?} convention")]
    AdjustmentFailed {
        /// The original date that couldn't be adjusted.
        date: Date,
        /// The business day convention that was attempted.
        convention: BusinessDayConvention,
        /// Maximum number of days searched.
        max_days: i32,
    },

    /// Date falls outside the allowed range (e.g., principal event after maturity).
    #[error("Date {date} is outside the allowed range [{} to {}]", .range.0, .range.1)]
    DateOutOfRange {
        /// The date that is out of range.
        date: Date,
        /// The allowed range (inclusive).
        range: (Date, Date),
    },

    /// Unknown or unsupported calendar identifier.
    ///
    /// This error occurs when attempting to look up a calendar by ID that doesn't
    /// exist in the registry.
    #[error("Calendar not found: '{requested}'{}", format_suggestions(.suggestions))]
    CalendarNotFound {
        /// The requested calendar ID that was not found.
        requested: String,
        /// Similar calendar IDs that might be what the user meant.
        suggestions: Vec<String>,
    },

    /// Day-count convention requires a holiday calendar but none was provided.
    #[error("DayCount::Bus252 requires a holiday calendar in DayCountContext")]
    MissingCalendarForBus252,

    /// Day-count convention requires a coupon frequency but none was provided.
    #[error("DayCount::ActActIsma requires a coupon frequency in DayCountContext")]
    MissingFrequencyForActActIsma,

    /// DayCount::ActActIsma only supports month/year coupon frequencies.
    #[error("DayCount::ActActIsma requires a Months/Years frequency, got {frequency}")]
    ActActIsmaUnsupportedFrequency {
        /// String form of the provided frequency (e.g., "2W", "10D").
        frequency: String,
    },

    // ─────────────────────────────────────────────────────────────────────────
    // Market Data and Lookups
    // ─────────────────────────────────────────────────────────────────────────
    /// Requested item (curve, surface, etc.) not found in a collection.
    #[error("Requested item not found: {id}")]
    NotFound {
        /// The identifier of the requested item that was not found
        id: String,
    },

    /// Requested curve not found in market context (with suggestions).
    #[error("Curve not found: {requested}{}", format_suggestions(.suggestions))]
    MissingCurve {
        /// The requested curve ID
        requested: String,
        /// Similar curve IDs that might be what the user meant
        suggestions: Vec<String>,
    },

    /// Requested curve exists, but has a different curve type than expected.
    ///
    /// This is returned when callers use typed getters (e.g. `get_discount`) and the
    /// stored curve under the same ID is of another kind (e.g. Hazard).
    #[error("Curve type mismatch for '{id}': expected '{expected}', got '{actual}'")]
    WrongCurveType {
        /// Curve identifier that was requested.
        id: String,
        /// Expected curve type (human-readable).
        expected: String,
        /// Actual curve type encountered (human-readable).
        actual: String,
    },

    /// Unknown or unsupported currency code supplied by the caller.
    #[error("Unknown currency code")]
    UnknownCurrency,

    /// Invalid tenor string format.
    #[error("Invalid tenor '{tenor}': {reason}")]
    InvalidTenor {
        /// The tenor string that failed to parse.
        tenor: String,
        /// Reason for the parsing failure.
        reason: String,
    },

    /// Invalid credit rating string.
    #[error("Invalid credit rating: '{value}'")]
    InvalidRating {
        /// The rating string that failed to parse.
        value: String,
    },

    // ─────────────────────────────────────────────────────────────────────────
    // Numeric Conversion
    // ─────────────────────────────────────────────────────────────────────────
    /// Decimal value cannot be represented as f64 (overflow or loss of precision).
    ///
    /// This error occurs when attempting to convert an internal `Decimal` amount
    /// to `f64` and the value is outside the representable range.
    #[error("Decimal conversion overflow: value cannot be represented as f64")]
    ConversionOverflow,

    /// Non-finite numeric value (NaN or infinity) encountered where finite required.
    ///
    /// This error occurs when constructing monetary amounts or performing
    /// calculations that require finite values.
    #[error("Non-finite value: expected finite number, got {kind}")]
    NonFiniteValue {
        /// Description of the non-finite value (e.g., "NaN", "infinity", "-infinity").
        kind: NonFiniteKind,
    },

    // ─────────────────────────────────────────────────────────────────────────
    // FX / Rates / Conversions
    // ─────────────────────────────────────────────────────────────────────────
    /// Invalid FX rate encountered (non-finite, non-positive, or otherwise unusable).
    #[error("Invalid FX rate for {from}->{to}: {rate}")]
    InvalidFxRate {
        /// Source currency.
        from: Currency,
        /// Target currency.
        to: Currency,
        /// The invalid rate value.
        rate: f64,
    },

    /// Invalid business-day basis for Bus/252 day-count (must be positive).
    #[error("Invalid Bus/252 basis: expected positive, got {basis}")]
    InvalidBusBasis {
        /// Divisor used for Bus/252 (e.g., 252).
        basis: u16,
    },

    // ─────────────────────────────────────────────────────────────────────────
    // Volatility
    // ─────────────────────────────────────────────────────────────────────────
    /// Volatility conversion failed due to solver not converging.
    ///
    /// This error occurs when the numerical solver cannot find a volatility
    /// that produces the target price within tolerance.
    #[error("Volatility conversion failed: solver did not converge within tolerance {tolerance} (residual: {residual:.2e})")]
    VolatilityConversionFailed {
        /// The solver tolerance that was used.
        tolerance: f64,
        /// The residual at the best guess.
        residual: f64,
    },

    /// Lognormal volatility requires positive forward rate.
    ///
    /// The Black (lognormal) model is undefined for non-positive forward rates.
    /// Use [`VolatilityConvention::ShiftedLognormal`](crate::math::volatility::VolatilityConvention::ShiftedLognormal)
    /// with an appropriate shift for negative rate environments.
    #[error("Lognormal volatility requires positive forward rate (got {forward:.6}); use ShiftedLognormal with shift >= {required_shift:.6}")]
    NonPositiveForwardForLognormal {
        /// The forward rate that was provided.
        forward: f64,
        /// The minimum shift required to make the shifted forward positive.
        required_shift: f64,
    },

    /// Shifted lognormal conversion has non-positive shifted forward.
    ///
    /// The shifted forward (F + shift) must be positive for the model to be valid.
    #[error("Shifted forward must be positive: forward ({forward:.6}) + shift ({shift:.6}) = {shifted:.6}")]
    NonPositiveShiftedForward {
        /// The forward rate.
        forward: f64,
        /// The shift amount.
        shift: f64,
        /// The resulting shifted forward.
        shifted: f64,
    },

    /// Invalid volatility value (must be positive and finite).
    #[error("Invalid volatility: expected positive finite value, got {value}")]
    InvalidVolatility {
        /// The invalid volatility value.
        value: f64,
    },

    /// Invalid time to expiry (must be non-negative).
    #[error("Invalid time to expiry: expected non-negative, got {value:.6}")]
    InvalidTimeToExpiry {
        /// The invalid time to expiry value.
        value: f64,
    },

    // ─────────────────────────────────────────────────────────────────────────
    // Bumps and Scenarios
    // ─────────────────────────────────────────────────────────────────────────
    /// Unsupported bump operation on a market data type.
    ///
    /// This error occurs when attempting to apply a bump with an unsupported
    /// combination of mode, units, or bump type for a given curve/surface.
    #[error("Unsupported bump operation: {reason}")]
    UnsupportedBump {
        /// Description of why the bump is not supported.
        reason: String,
    },

    // ─────────────────────────────────────────────────────────────────────────
    // Joint calendar FX date logic
    // ─────────────────────────────────────────────────────────────────────────
    /// Joint calendar date adjustment did not converge within a small fixed iteration budget.
    #[error("Joint calendar adjustment did not converge within {max_iterations} iterations")]
    JointCalendarNonConvergent {
        /// Original date being adjusted.
        date: Date,
        /// Business day convention applied.
        convention: BusinessDayConvention,
        /// Maximum iterations attempted.
        max_iterations: u32,
    },

    /// Joint calendar business-day counting exceeded a safety iteration limit.
    #[error(
        "Joint calendar business-day roll exceeded iteration limit: requested {n_days} days, hit {max_iters} iterations"
    )]
    JointCalendarIterationLimitExceeded {
        /// Start date for the roll.
        start: Date,
        /// Requested joint business days to add.
        n_days: u32,
        /// Maximum iterations allowed.
        max_iters: u32,
    },

    // ─────────────────────────────────────────────────────────────────────────
    // Solver Failures
    // ─────────────────────────────────────────────────────────────────────────
    /// Root-finding solver failed to converge within tolerance.
    ///
    /// This error provides diagnostic information about why the solver failed,
    /// including iteration count, final residual, and the specific failure mode.
    #[error("Solver failed after {iterations} iterations: {reason} (residual: {residual:.6e}, last x: {last_x:.6e})")]
    SolverConvergenceFailed {
        /// Number of iterations performed before failure.
        iterations: usize,
        /// Final residual value |f(x)|.
        residual: f64,
        /// Last x value tried.
        last_x: f64,
        /// Human-readable reason for failure.
        reason: String,
    },

    // ─────────────────────────────────────────────────────────────────────────
    // Return Series / Analytics
    // ─────────────────────────────────────────────────────────────────────────
    /// A row of a return panel violated the analytics input contract.
    ///
    /// Used by `finstack-quant-analytics::Performance` to flag the specific ticker,
    /// row, and reason a return series was rejected (non-finite value, total
    /// wipeout, length mismatch with the date grid, etc.).
    #[error("Invalid return series for ticker {ticker:?} at row {index}: {reason}")]
    InvalidReturnSeries {
        /// Ticker name as provided by the caller.
        ticker: String,
        /// Zero-based row index of the offending observation.
        index: usize,
        /// Human-readable description of the failure.
        reason: String,
    },

    /// FX triangulation failed - unable to compute cross rate via pivot currency.
    ///
    /// This error identifies which leg of the triangulation was missing.
    #[error("FX triangulation failed for {from}->{to} via {pivot}: {missing_leg}")]
    FxTriangulationFailed {
        /// Source currency.
        from: Currency,
        /// Target currency.
        to: Currency,
        /// Pivot currency used for triangulation.
        pivot: Currency,
        /// Description of the missing leg (e.g., "EUR->USD not found").
        missing_leg: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_too_few_points_display() {
        let err = InputError::TooFewPoints;
        assert_eq!(err.to_string(), "At least two data points are required");
    }

    #[test]
    fn test_non_monotonic_knots_display() {
        let err = InputError::NonMonotonicKnots;
        assert_eq!(err.to_string(), "Times (knots) must be strictly increasing");
    }

    #[test]
    fn test_invalid_date_display() {
        let err = InputError::InvalidDate {
            year: 2024,
            month: 2,
            day: 30,
        };
        assert_eq!(err.to_string(), "Invalid calendar date: 2024-02-30");
    }

    #[test]
    fn test_missing_curve_display() {
        let err = InputError::MissingCurve {
            requested: "USD_OS".to_string(),
            suggestions: vec!["USD_OIS".to_string()],
        };
        let msg = err.to_string();
        assert!(msg.contains("USD_OS"));
        assert!(msg.contains("Did you mean"));
        assert!(msg.contains("USD_OIS"));
    }

    #[test]
    fn test_volatility_conversion_failed_display() {
        let err = InputError::VolatilityConversionFailed {
            tolerance: 1e-8,
            residual: 0.001,
        };
        let msg = err.to_string();
        assert!(msg.contains("Volatility conversion failed"));
        assert!(msg.contains("1e-8") || msg.contains("0.00000001"));
    }

    #[test]
    fn test_non_positive_forward_display() {
        let err = InputError::NonPositiveForwardForLognormal {
            forward: -0.01,
            required_shift: 0.02,
        };
        let msg = err.to_string();
        assert!(msg.contains("Lognormal volatility"));
        assert!(msg.contains("-0.01"));
    }
}
