//! Error types for correlation matrix utilities.
//!
//! This module contains the structured diagnostics returned by
//! [`crate::correlation::validate_correlation_matrix`] and
//! [`crate::correlation::nearest_correlation::nearest_correlation_matrix`].
//!
//! The errors focus on caller-fixable input problems:
//! - wrong flattened matrix size
//! - non-unit diagonal entries
//! - asymmetric correlation matrices
//! - out-of-bounds correlation values
//! - matrices that are not positive semidefinite
//!
//! Use these variants when surfacing validation failures to configuration or
//! calibration layers.

/// Convenience result type used throughout the correlation crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Error type for correlation matrix validation.
///
/// This enum preserves the first validation failure detected when checking a
/// row-major flattened correlation matrix.
#[derive(Debug, Clone, PartialEq, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum Error {
    /// Matrix size does not match expected n×n.
    #[error("Invalid matrix size: expected {expected}×{expected}={}, got {actual}", expected * expected)]
    InvalidSize {
        /// Expected number of factors (n for n×n matrix).
        expected: usize,
        /// Actual length of the matrix array.
        actual: usize,
    },
    /// Diagonal element is not 1.
    #[error("Diagonal element [{index},{index}] = {value}, expected 1.0")]
    DiagonalNotOne {
        /// Index of the invalid diagonal element.
        index: usize,
        /// Actual value found on diagonal.
        value: f64,
    },
    /// Matrix is not symmetric.
    #[error("Matrix not symmetric: |ρ[{i},{j}] - ρ[{j},{i}]| = {diff}")]
    NotSymmetric {
        /// Row index.
        i: usize,
        /// Column index.
        j: usize,
        /// Absolute difference `|rho[i,j] - rho[j,i]|`.
        diff: f64,
    },
    /// Matrix is not positive semi-definite (Cholesky failed).
    #[error("Matrix not positive semi-definite: Cholesky failed at row {row}")]
    NotPositiveSemiDefinite {
        /// Row where Cholesky decomposition failed.
        row: usize,
    },
    /// Correlation value out of bounds [-1, 1].
    #[error("Correlation ρ[{i},{j}] = {value} out of bounds [-1, 1]")]
    OutOfBounds {
        /// Row index.
        i: usize,
        /// Column index.
        j: usize,
        /// Out-of-bounds value.
        value: f64,
    },
    /// Volatility vector length does not match number of factors.
    ///
    /// Returned by validated factor-model constructors when the caller supplies
    /// a volatility vector whose length disagrees with the declared number of
    /// factors. Previously the caller's vector was silently dropped and
    /// replaced with unit volatilities, which masked serious misconfiguration.
    #[error("Volatility vector length mismatch: expected {expected}, got {actual}")]
    VolatilityLengthMismatch {
        /// Expected number of factors.
        expected: usize,
        /// Length of the volatility vector supplied by the caller.
        actual: usize,
    },
    /// Volatility value is negative or non-finite.
    ///
    /// Returned by validated factor-model constructors when a volatility entry
    /// is `< 0.0` or non-finite. Silent clamping would mask sign-flip bugs
    /// upstream (e.g., `-0.2` becoming `0.01`).
    #[error("Invalid volatility at index {index}: {value} (must be finite and >= 0.0)")]
    InvalidVolatility {
        /// Index of the offending volatility value.
        index: usize,
        /// The offending value.
        value: f64,
    },
    /// Recovery-model input is invalid.
    #[error("Invalid recovery input `{field}` = {value}: {requirement}")]
    InvalidRecoveryInput {
        /// Name of the invalid recovery input field.
        field: String,
        /// The offending value.
        value: f64,
        /// Human-readable requirement violated by the value.
        requirement: String,
    },
    /// Student-t degrees of freedom is invalid.
    #[error("Invalid Student-t degrees of freedom {value}: must be finite and > 2.0")]
    InvalidStudentTDegreesOfFreedom {
        /// The offending degrees-of-freedom value.
        value: f64,
    },
}
