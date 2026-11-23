//! Core [`Error`], [`ErrorKind`], [`Result`], and error-chain formatting.

use super::InputError;

/// Classification of a non-finite floating-point value.
pub(crate) use super::suggestions::{format_suggestions, fuzzy_suggestions};

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
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Error {
    /// User input validation error.
    #[error(transparent)]
    Input(#[from] InputError),

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

    /// A scalar expression operation is undefined for the supplied row.
    #[error("expression node {node}, row {row}: {message}")]
    ExpressionEvaluation {
        /// Zero-based node in the compiled expression plan.
        node: u64,
        /// Zero-based row of the input columns.
        row: usize,
        /// Arithmetic operation and the reason it is undefined.
        message: String,
    },

    /// Catch-all for unexpected internal failures.
    #[error("Internal system error: {0}")]
    Internal(String),
}

/// Convenience result type used throughout the core crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Coarse classification of an [`Error`] for host-language exception mapping.
///
/// Language bindings map this three-way split onto their native exception
/// types (Python `KeyError` / `ValueError` / `RuntimeError`; JS error
/// `kind` of `"not_found"` / `"validation"` / `"computation"`), so the
/// partition is defined once here, by variant, never by message text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// A lookup by identifier failed (curve, calendar, FX pair, generic id).
    NotFound,
    /// The inputs were rejected before or during computation.
    Validation,
    /// The computation itself failed operationally (solver non-convergence,
    /// calibration, internal invariant, dependency cycle, allocation limit).
    Computation,
}

impl ErrorKind {
    /// The host-facing label of this kind: `"not_found"`, `"validation"` or
    /// `"computation"` (the same strings serde emits).
    ///
    /// Bindings expose it verbatim (the WASM error `kind` property); a host may
    /// add its own labels for host-only failures, such as the WASM
    /// `"invalid_type"` for a wrong JavaScript argument type, which never
    /// reaches Rust and is not an `ErrorKind`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            ErrorKind::NotFound => "not_found",
            ErrorKind::Validation => "validation",
            ErrorKind::Computation => "computation",
        }
    }
}

/// Format an error and its full `source()` chain as one message.
///
/// Levels are joined with `": "` (the `anyhow` convention); a cause whose text
/// the previous level already ends with is skipped, so wrappers that embed
/// their cause's message are not repeated. Bindings use this to carry the whole
/// diagnostic context into host exceptions, which hold only a message.
///
/// # Arguments
///
/// * `err` - The outermost error; its `source()` chain is followed to the end.
#[must_use]
pub fn format_chain(err: &(dyn std::error::Error + 'static)) -> String {
    let mut out = err.to_string();
    let mut source = err.source();
    while let Some(cause) = source {
        let message = cause.to_string();
        if !out.ends_with(&message) {
            out.push_str(": ");
            out.push_str(&message);
        }
        source = cause.source();
    }
    out
}

impl Error {
    /// Classify this error for host-language exception mapping.
    ///
    /// - Lookup misses (`InputError::{MissingCurve, NotFound, CalendarNotFound,
    ///   FxTriangulationFailed}`) → [`ErrorKind::NotFound`]
    /// - `Calibration`, `Internal`, `CircularDependency`, and
    ///   `InputError::{SolverConvergenceFailed, VolatilityConversionFailed}`
    ///   → [`ErrorKind::Computation`]
    /// - `MetricCalculationFailed` preserves its cause's category recursively
    /// - Everything else → [`ErrorKind::Validation`]
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        match self {
            Error::Input(
                InputError::MissingCurve { .. }
                | InputError::NotFound { .. }
                | InputError::CalendarNotFound { .. }
                | InputError::FxTriangulationFailed { .. },
            ) => ErrorKind::NotFound,
            Error::Calibration { .. }
            | Error::ExpressionEvaluation { .. }
            | Error::Internal(_)
            | Error::CircularDependency { .. }
            | Error::Input(
                InputError::SolverConvergenceFailed { .. }
                | InputError::VolatilityConversionFailed { .. },
            ) => ErrorKind::Computation,
            Error::MetricCalculationFailed { cause, .. } => cause.kind(),
            _ => ErrorKind::Validation,
        }
    }

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
}
