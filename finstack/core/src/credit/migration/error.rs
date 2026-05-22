//! Error types for credit migration model operations.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors produced by credit migration model operations.
///
/// Covers validation failures for rating scales, transition matrices,
/// generator matrices, and numerical failures in matrix computations.
#[derive(Debug, Clone, PartialEq, Error, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MigrationError {
    /// Matrix is not square.
    #[error("matrix is not square: {rows}x{cols}")]
    NotSquare {
        /// Number of rows.
        rows: usize,
        /// Number of columns.
        cols: usize,
    },

    /// Matrix dimension does not match rating scale size.
    #[error("matrix dimension {actual} does not match rating scale size {expected}")]
    DimensionMismatch {
        /// Expected dimension (from scale).
        expected: usize,
        /// Actual dimension (from data).
        actual: usize,
    },

    /// Row does not sum to the expected value within tolerance.
    #[error("row {row} sums to {sum}, expected {expected} (tolerance {tol})")]
    RowSumViolation {
        /// Row index (0-based).
        row: usize,
        /// Actual row sum.
        sum: f64,
        /// Expected sum (0.0 for generator, 1.0 for transition).
        expected: f64,
        /// Tolerance used for comparison.
        tol: f64,
    },

    /// Matrix entry falls outside the allowed range.
    #[error("entry ({row},{col}) = {value} is outside [{min},{max}]")]
    EntryOutOfRange {
        /// Row index.
        row: usize,
        /// Column index.
        col: usize,
        /// Actual value.
        value: f64,
        /// Minimum allowed value.
        min: f64,
        /// Maximum allowed value.
        max: f64,
    },

    /// Generator extraction failed because an eigenvalue is non-positive.
    #[error("generator extraction failed: eigenvalue {index} = {value} is non-positive")]
    NoValidGenerator {
        /// Eigenvalue index.
        index: usize,
        /// Eigenvalue value.
        value: f64,
    },

    /// Generator extraction failed because the matrix has complex eigenvalues.
    #[error("generator extraction failed: matrix has complex eigenvalues (no real Schur form)")]
    ComplexEigenvalues,

    /// Round-trip validation failed: exp(Q) is too far from P.
    #[error("round-trip error ||exp(Q)-P||_inf = {error} exceeds tolerance {tolerance}")]
    RoundTripError {
        /// Computed round-trip error (infinity norm).
        error: f64,
        /// Tolerance threshold.
        tolerance: f64,
    },

    /// State label not found in rating scale.
    #[error("state '{label}' not found in rating scale")]
    UnknownState {
        /// The unrecognized state label.
        label: String,
    },

    /// Rating scale must have at least 2 states.
    #[error("rating scale must have at least 2 states")]
    InsufficientStates,

    /// Duplicate state label in rating scale.
    #[error("duplicate state label '{label}'")]
    DuplicateLabel {
        /// The duplicated label.
        label: String,
    },

    /// Absorbing default state has non-zero off-diagonal entries.
    #[error("absorbing state {state} has non-zero off-diagonal entries")]
    NonAbsorbingDefault {
        /// State index.
        state: usize,
    },

    /// Time horizon must be positive.
    #[error("horizon must be positive, got {0}")]
    InvalidHorizon(f64),

    /// Label could not be resolved to a Moody's WARF factor.
    #[error("no WARF factor for label '{label}'")]
    NoWarfFactor {
        /// The unresolved label.
        label: String,
    },

    /// Scale contains no labels that map to known WARF factors.
    #[error("scale contains no labels with known WARF factors")]
    NoWarfMapping,

    /// Scales do not match for a binary matrix operation.
    #[error("scale mismatch: matrices have different rating scales")]
    ScaleMismatch,

    /// Matrix inversion failed (singular or numerically degenerate).
    #[error("matrix is singular or numerically degenerate; cannot compute inverse")]
    SingularMatrix,

    /// Internal invariant failed in migration model code.
    #[error("internal migration invariant violated: {0}")]
    Internal(String),
}
