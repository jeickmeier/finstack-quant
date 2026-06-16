//! Error types for financial computation and validation failures.
//!
//! This module defines the unified error hierarchy used throughout Finstack Quant's
//! core library. All errors bubble up through the [`enum@Error`] enum, which wraps
//! domain-specific failures like input validation, interpolation bounds,
//! currency mismatches, and calibration failures.
//!
//! # Design Philosophy
//!
//! - **Actionable errors**: Each variant includes enough context for callers to
//!   diagnose and potentially recover from failures
//! - **Non-exhaustive**: Error variants may expand in minor releases; always
//!   match with a catch-all `_` pattern for forward compatibility
//! - **Fuzzy suggestions**: Missing curve errors include similar IDs based on
//!   edit distance to guide users toward corrections
//! - **Serializable**: All error types support serde wire formats
//!
//! # Error Categories
//!
//! - **Input validation** ([`InputError`]): User-supplied data fails constraints
//!   (e.g., non-monotonic knots, invalid dates, missing curves)
//! - **Currency safety** ([`Error::CurrencyMismatch`]): Attempted cross-currency
//!   arithmetic without explicit conversion
//! - **Interpolation** ([`Error::InterpOutOfBounds`]): Query point falls outside
//!   curve bounds
//! - **Calibration** ([`Error::Calibration`]): Numerical solver or fitting
//!   procedure failed to converge
//! - **Validation** ([`Error::Validation`]): Market data fails no-arbitrage or
//!   structural checks
//!
//! # Examples
//!
//! ## Handling common input errors
//!
//! ```rust
//! use finstack_quant_core::{Error, InputError};
//!
//! fn parse_knots(data: &[(f64, f64)]) -> Result<(), Error> {
//!     if data.len() < 2 {
//!         return Err(InputError::TooFewPoints.into());
//!     }
//!
//!     // Check monotonicity
//!     for window in data.windows(2) {
//!         if window[1].0 <= window[0].0 {
//!             return Err(InputError::NonMonotonicKnots.into());
//!         }
//!     }
//!
//!     Ok(())
//! }
//!
//! let invalid_data = vec![(1.0, 0.95), (0.5, 0.90)]; // Non-monotonic
//! assert!(parse_knots(&invalid_data).is_err());
//! ```
//!
//! ## Currency mismatch detection
//!
//! ```rust
//! use finstack_quant_core::Error;
//! use finstack_quant_core::money::Money;
//! use finstack_quant_core::currency::Currency;
//!
//! let usd = Money::new(100.0, Currency::USD);
//! let eur = Money::new(85.0, Currency::EUR);
//!
//! // Attempting to add different currencies returns CurrencyMismatch
//! let result = usd.checked_add(eur);
//! assert!(result.is_err());
//!
//! match result {
//!     Err(Error::CurrencyMismatch { expected, actual }) => {
//!         assert_eq!(expected, Currency::USD);
//!         assert_eq!(actual, Currency::EUR);
//!     }
//!     _ => panic!("Expected currency mismatch"),
//! }
//! ```
//!
//! ## Using error suggestions for missing curves
//!
//! ```rust
//! use finstack_quant_core::Error;
//!
//! let available = vec![
//!     "USD_OIS".to_string(),
//!     "EUR_OIS".to_string(),
//!     "GBP_GILT".to_string(),
//! ];
//!
//! // Typo in curve name
//! let err = Error::missing_curve_with_suggestions("USD_OS", &available);
//! let msg = format!("{}", err);
//!
//! // Error message includes suggestions
//! assert!(msg.contains("USD_OIS") || msg.contains("Did you mean"));
//! ```
//!
//! # See Also
//!
//! - [`crate::Result`] - Type alias for `Result<T, Error>`
//! - [`InputError`] - Specific validation failure modes
//!
//! # References
//!
//! The fuzzy matching algorithm uses Levenshtein edit distance:
//! - Levenshtein, V. I. (1966). "Binary codes capable of correcting deletions,
//!   insertions, and reversals." *Soviet Physics Doklady*, 10(8), 707-710.

mod inputs;
mod suggestions;

// Re-export InputError for public API
pub use inputs::InputError;
/// Classification of a non-finite floating-point value.
pub use inputs::NonFiniteKind;

// Re-export suggestion utilities for internal use
pub(crate) use suggestions::{format_suggestions, fuzzy_suggestions};

use crate::currency::Currency;

/// Unified error type for all high-level APIs.
///
/// All user-facing validation issues bubble up via the [`Input`](Error::Input)
/// wrapper so callers can pattern-match on [`InputError`] for actionable
/// feedback.
///
/// # Variants
///
/// - **Input**: Wraps [`InputError`] for all validation failures
/// - **InterpOutOfBounds**: Query point outside interpolator domain
/// - **CurrencyMismatch**: Binary operation on incompatible currencies
/// - **Calibration**: Numerical fitting or solver convergence failure
/// - **Validation**: Market data structural checks failed
/// - **UnknownMetric**: Requested metric ID not recognized
/// - **MetricNotApplicable**: Metric cannot be computed for given instrument type
/// - **MetricCalculationFailed**: Metric computation encountered an error
/// - **CircularDependency**: Circular dependency detected in metric dependency graph
/// - **Internal**: Unexpected system-level failures
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::{Error, InputError};
///
/// // Convert InputError to Error
/// let input_err: Error = InputError::TooFewPoints.into();
/// assert!(matches!(input_err, Error::Input(_)));
///
/// // Pattern match on error variants
/// fn handle_error(err: Error) -> String {
///     match err {
///         Error::Input(e) => format!("Invalid input: {}", e),
///         Error::CurrencyMismatch { expected, actual } => {
///             format!("Cannot mix {} and {}", expected, actual)
///         }
///         Error::InterpOutOfBounds => "Query outside curve range".to_string(),
///         Error::Calibration { message, .. } => format!("Calibration failed: {}", message),
///         Error::Validation(msg) => format!("Validation error: {}", msg),
///         Error::UnknownMetric { metric_id, .. } => format!("Unknown metric: {}", metric_id),
///         Error::Internal(message) => format!("Internal error: {}", message),
///         _ => "Unknown error".to_string(), // Non-exhaustive enum
///     }
/// }
///
/// let msg = handle_error(input_err);
/// assert!(msg.contains("Invalid input"));
/// ```
#[derive(Debug, Clone, PartialEq, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum Error {
    /// User input validation error.
    #[error(transparent)]
    Input(#[from] InputError),

    /// Interpolator evaluation exceeded grid bounds.
    #[error("Interpolation input out of bounds")]
    InterpOutOfBounds,

    /// Currency mismatch in a binary [`Money`](crate::money::Money) operation.
    #[error("Currency mismatch: expected {expected}, got {actual}")]
    CurrencyMismatch {
        /// The expected (left-hand) currency.
        expected: Currency,
        /// The actual (right-hand) currency encountered.
        actual: Currency,
    },

    /// Calibration process failure.
    #[error("Calibration error: {message}")]
    Calibration {
        /// Human-readable error description.
        message: String,
        /// Error category for programmatic handling.
        category: String,
    },

    /// Market data validation failure (no-arbitrage, monotonicity, bounds).
    #[error("Validation error: {0}")]
    Validation(String),

    /// Unknown metric requested.
    ///
    /// This error occurs when attempting to compute or parse a metric ID that
    /// is not recognized in the standard metrics registry.
    #[error("Unknown metric '{metric_id}'{}", format_suggestions(.available))]
    UnknownMetric {
        /// The requested metric ID that was not recognized.
        metric_id: String,
        /// List of available standard metric IDs for user reference.
        available: Vec<String>,
    },

    /// Metric not applicable to instrument type.
    ///
    /// This error occurs when attempting to compute a metric that does not
    /// make sense for the given instrument type (e.g., YTM for a swap).
    #[error("Metric '{metric_id}' is not applicable to instrument type '{instrument_type}'")]
    MetricNotApplicable {
        /// The metric ID that was requested.
        metric_id: String,
        /// The instrument type for which the metric is not applicable.
        instrument_type: String,
    },

    /// Metric calculation failed.
    ///
    /// This error wraps the underlying cause when a metric calculator
    /// encounters an error during computation (e.g., missing market data).
    #[error("Metric '{metric_id}' calculation failed: {cause}")]
    MetricCalculationFailed {
        /// The metric ID that failed to compute.
        metric_id: String,
        /// The underlying error that caused the failure.
        #[source]
        cause: Box<Error>,
    },

    /// Circular dependency detected in metric dependency graph.
    ///
    /// This error occurs when metric dependencies form a cycle, making it
    /// impossible to determine a valid evaluation order.
    #[error("Circular dependency detected in metrics: {}", .path.join(" -> "))]
    CircularDependency {
        /// The path of metric IDs forming the cycle.
        path: Vec<String>,
    },

    /// Catch-all for unexpected internal failures.
    #[error("Internal system error: {0}")]
    Internal(String),
}

/// Convenience result type used throughout the core crate.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Create a MissingCurve error with suggestions based on available curves.
    ///
    /// Performs fuzzy matching to find similar curve IDs.
    ///
    /// # Example
    ///
    /// ```
    /// use finstack_quant_core::Error;
    ///
    /// let available = vec!["USD_OIS".to_string(), "EUR_OIS".to_string(), "GBP_OIS".to_string()];
    /// let err = Error::missing_curve_with_suggestions("USD_OS", &available);
    ///
    /// let msg = format!("{}", err);
    /// assert!(msg.contains("Did you mean"));
    /// ```
    pub fn missing_curve_with_suggestions(
        requested: impl Into<String>,
        available: &[impl AsRef<str>],
    ) -> Self {
        let requested_str = requested.into();
        let suggestions = fuzzy_suggestions(&requested_str, available.iter().map(AsRef::as_ref));
        Self::Input(InputError::MissingCurve {
            requested: requested_str,
            suggestions,
        })
    }

    /// Create a CalendarNotFound error with suggestions based on available calendars.
    ///
    /// Performs fuzzy matching to find similar calendar IDs.
    ///
    /// # Example
    ///
    /// ```
    /// use finstack_quant_core::Error;
    ///
    /// let available = &["nyse", "target2", "gblo"];
    /// let err = Error::calendar_not_found_with_suggestions("nyes", available);
    ///
    /// let msg = format!("{}", err);
    /// assert!(msg.contains("Did you mean"));
    /// ```
    pub fn calendar_not_found_with_suggestions(
        requested: impl Into<String>,
        available: &[&str],
    ) -> Self {
        let requested_str = requested.into();
        let suggestions = fuzzy_suggestions(&requested_str, available.iter().copied());
        Self::Input(InputError::CalendarNotFound {
            requested: requested_str,
            suggestions,
        })
    }

    /// Create an UnknownMetric error with the list of available standard metrics.
    ///
    /// # Example
    ///
    /// ```
    /// use finstack_quant_core::Error;
    ///
    /// let available = vec!["dv01".to_string(), "duration_mod".to_string(), "ytm".to_string()];
    /// let err = Error::unknown_metric("dv1", available);
    ///
    /// let msg = format!("{}", err);
    /// assert!(msg.contains("dv01") || msg.contains("Did you mean"));
    /// ```
    pub fn unknown_metric(metric_id: impl Into<String>, available: Vec<String>) -> Self {
        Self::UnknownMetric {
            metric_id: metric_id.into(),
            available,
        }
    }

    /// Create a MetricNotApplicable error.
    ///
    /// # Example
    ///
    /// ```
    /// use finstack_quant_core::Error;
    ///
    /// let err = Error::metric_not_applicable("ytm", "Swap");
    ///
    /// let msg = format!("{}", err);
    /// assert!(msg.contains("ytm"));
    /// assert!(msg.contains("Swap"));
    /// ```
    pub fn metric_not_applicable(
        metric_id: impl Into<String>,
        instrument_type: impl Into<String>,
    ) -> Self {
        Self::MetricNotApplicable {
            metric_id: metric_id.into(),
            instrument_type: instrument_type.into(),
        }
    }

    /// Create a MetricCalculationFailed error wrapping an underlying cause.
    ///
    /// # Example
    ///
    /// ```
    /// use finstack_quant_core::{Error, InputError};
    ///
    /// let cause = Error::Input(InputError::NotFound { id: "curve".to_string() });
    /// let err = Error::metric_calculation_failed("dv01", cause);
    ///
    /// let msg = format!("{}", err);
    /// assert!(msg.contains("dv01"));
    /// assert!(msg.contains("calculation failed"));
    /// ```
    pub fn metric_calculation_failed(metric_id: impl Into<String>, cause: Error) -> Self {
        Self::MetricCalculationFailed {
            metric_id: metric_id.into(),
            cause: Box::new(cause),
        }
    }

    /// Create a CircularDependency error with the dependency path.
    ///
    /// # Example
    ///
    /// ```
    /// use finstack_quant_core::Error;
    ///
    /// let path = vec!["metric_a".to_string(), "metric_b".to_string(), "metric_a".to_string()];
    /// let err = Error::circular_dependency(path);
    ///
    /// let msg = format!("{}", err);
    /// assert!(msg.contains("metric_a"));
    /// assert!(msg.contains("->"));
    /// ```
    pub fn circular_dependency(path: Vec<String>) -> Self {
        Self::CircularDependency { path }
    }

    /// Create an internal error with invariant/debugging context.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display() {
        let err: Error = InputError::Invalid.into();
        assert_eq!(format!("{}", err), "Invalid input data");

        let not_found_err: Error = InputError::NotFound {
            id: "test_curve".to_string(),
        }
        .into();
        assert_eq!(
            format!("{}", not_found_err),
            "Requested item not found: test_curve"
        );
    }

    #[test]
    fn test_missing_curve_with_suggestions() {
        let available = vec![
            "USD_OIS".to_string(),
            "USD_GOVT".to_string(),
            "EUR_OIS".to_string(),
            "GBP_GILT".to_string(),
        ];

        // Test exact fuzzy match
        let err = Error::missing_curve_with_suggestions("USD_OS", &available);
        let msg = format!("{}", err);
        assert!(msg.contains("USD_OIS") || msg.contains("Did you mean"));

        // Test prefix match
        let err2 = Error::missing_curve_with_suggestions("USD", &available);
        let msg2 = format!("{}", err2);
        assert!(msg2.contains("USD_OIS") || msg2.contains("USD_GOVT"));

        // Test no match
        let err3 = Error::missing_curve_with_suggestions("JPY_UNKNOWN", &available);
        let msg3 = format!("{}", err3);
        assert!(msg3.contains("Curve not found"));
    }

    #[test]
    fn test_unknown_metric() {
        let available = vec![
            "dv01".to_string(),
            "duration_mod".to_string(),
            "ytm".to_string(),
        ];
        let err = Error::unknown_metric("dv1", available);

        let msg = format!("{}", err);
        assert!(msg.contains("Unknown metric 'dv1'"));

        match err {
            Error::UnknownMetric {
                metric_id,
                available: avail,
            } => {
                assert_eq!(metric_id, "dv1");
                assert_eq!(avail.len(), 3);
                assert!(avail.contains(&"dv01".to_string()));
            }
            _ => panic!("Expected UnknownMetric variant"),
        }
    }

    #[test]
    fn test_metric_not_applicable() {
        let err = Error::metric_not_applicable("ytm", "Swap");

        let msg = format!("{}", err);
        assert!(msg.contains("ytm"));
        assert!(msg.contains("Swap"));
        assert!(msg.contains("not applicable"));

        match err {
            Error::MetricNotApplicable {
                metric_id,
                instrument_type,
            } => {
                assert_eq!(metric_id, "ytm");
                assert_eq!(instrument_type, "Swap");
            }
            _ => panic!("Expected MetricNotApplicable variant"),
        }
    }

    #[test]
    fn test_metric_calculation_failed() {
        let cause = Error::Input(InputError::NotFound {
            id: "missing_curve".to_string(),
        });
        let err = Error::metric_calculation_failed("dv01", cause.clone());

        let msg = format!("{}", err);
        assert!(msg.contains("dv01"));
        assert!(msg.contains("calculation failed"));
        assert!(msg.contains("missing_curve"));

        match err {
            Error::MetricCalculationFailed {
                metric_id,
                cause: boxed_cause,
            } => {
                assert_eq!(metric_id, "dv01");
                assert_eq!(*boxed_cause, cause);
            }
            _ => panic!("Expected MetricCalculationFailed variant"),
        }
    }

    #[test]
    fn test_circular_dependency() {
        let path = vec![
            "metric_a".to_string(),
            "metric_b".to_string(),
            "metric_c".to_string(),
            "metric_a".to_string(),
        ];
        let err = Error::circular_dependency(path.clone());

        let msg = format!("{}", err);
        assert!(msg.contains("Circular dependency"));
        assert!(msg.contains("metric_a -> metric_b -> metric_c -> metric_a"));

        match err {
            Error::CircularDependency { path: cycle_path } => {
                assert_eq!(cycle_path, path);
                assert_eq!(cycle_path.len(), 4);
                assert_eq!(cycle_path[0], cycle_path[3]); // Cycle back to start
            }
            _ => panic!("Expected CircularDependency variant"),
        }
    }

    #[test]
    fn test_metric_errors_are_clonable() {
        let err1 = Error::unknown_metric("test", vec!["dv01".to_string()]);
        let err2 = err1.clone();
        assert_eq!(err1, err2);

        let err3 = Error::metric_not_applicable("ytm", "Swap");
        let err4 = err3.clone();
        assert_eq!(err3, err4);

        let cause = Error::internal("metric test failure");
        let err5 = Error::metric_calculation_failed("test", cause);
        let err6 = err5.clone();
        assert_eq!(err5, err6);

        let err7 = Error::circular_dependency(vec!["a".to_string(), "b".to_string()]);
        let err8 = err7.clone();
        assert_eq!(err7, err8);
    }

    #[test]
    fn internal_error_carries_context() {
        let err = Error::Internal("bootstrap diverged".into());
        let msg = err.to_string();
        assert!(
            msg.contains("bootstrap diverged"),
            "Error should contain context: {msg}"
        );
    }
}
