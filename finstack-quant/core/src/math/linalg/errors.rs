//! Linear algebra error types, tolerances, and matrix shape/symmetry validators.

use crate::error::InputError;
use thiserror::Error;

/// Relative singular threshold for Cholesky decomposition and substitution.
///
/// The generic unpivoted factorization compares factor pivots with this fraction
/// of the square root of the largest input diagonal magnitude. Substitution uses
/// the largest factor diagonal magnitude. Both criteria are invariant to a
/// uniform rescaling of the system. The correlation-specific path
/// (`cholesky_correlation`) uses the relative pivot tolerance instead.
pub(super) const SINGULAR_THRESHOLD: f64 = 1e-10;

/// Default tolerance for diagonal elements in correlation matrices.
///
/// A correlation matrix has a diagonal of exactly 1 by construction, so any
/// deviation is either floating-point noise from forming `rho = cov / (sd_i *
/// sd_j)` — which is `O(eps * kappa)`, in practice below 1e-12 — or a genuine
/// defect in the input. A diagonal of, say, 1.000001 signals a mis-normalized
/// covariance matrix rather than rounding, so the tolerance is set at
/// noise scale to reject it instead of passing it through to Cholesky.
pub(super) const DIAGONAL_TOLERANCE: f64 = 1e-10;

/// Default tolerance for symmetry checks in correlation matrices.
///
/// Symmetry, like the unit diagonal, holds exactly by construction; asymmetry
/// above noise scale indicates a transposition or indexing bug in the caller.
pub const SYMMETRY_TOLERANCE: f64 = 1e-10;

/// Relative pivot tolerance for [`cholesky_correlation`].
///
/// A pivot is considered numerically zero when it is below
/// `PIVOT_TOLERANCE_RELATIVE * max_diagonal`. This makes the threshold
/// scale-invariant and appropriate for both normalised correlation matrices
/// (diagonal ≈ 1) and general covariance matrices with large entries.
pub(super) const PIVOT_TOLERANCE_RELATIVE: f64 = 1e-10;

/// Number of entries in an `n × n` row-major matrix, or `None` when `n * n`
/// overflows `usize` (from `n = 65536` on 32-bit targets such as wasm32).
///
/// Every flat-matrix size check compares against this rather than an
/// unchecked `n * n`, which wraps in release builds and lets a wrong-length
/// buffer pass the check.
#[inline]
pub(super) fn square_len(n: usize) -> Option<usize> {
    n.checked_mul(n)
}

/// Reject a buffer that is not an `n × n` matrix of finite entries.
pub(super) fn check_square_finite(
    matrix: &[f64],
    n: usize,
) -> std::result::Result<(), CholeskyError> {
    if square_len(n) != Some(matrix.len()) {
        return Err(CholeskyError::DimensionMismatch {
            expected: n,
            actual: matrix.len(),
        });
    }
    for (index, &value) in matrix.iter().enumerate() {
        if !value.is_finite() {
            return Err(CholeskyError::NonFiniteInput {
                value,
                row: index / n,
                col: index % n,
            });
        }
    }
    Ok(())
}

/// Reject an `n × n` matrix whose off-diagonal entries violate symmetry beyond
/// `SYMMETRY_TOLERANCE * max_abs_entry`.
pub(super) fn check_symmetric(matrix: &[f64], n: usize) -> std::result::Result<(), CholeskyError> {
    let scale = matrix.iter().map(|v| v.abs()).fold(0.0_f64, f64::max);
    let tolerance = SYMMETRY_TOLERANCE * scale;
    for row in 0..n {
        for col in (row + 1)..n {
            let diff = (matrix[row * n + col] - matrix[col * n + row]).abs();
            if diff > tolerance {
                return Err(CholeskyError::NotSymmetric {
                    row,
                    col,
                    diff,
                    tolerance,
                });
            }
        }
    }
    Ok(())
}

/// `n * n` for an error message, or a note that it overflows.
pub(super) fn square_len_label(n: usize) -> String {
    square_len(n).map_or_else(
        || "more entries than fit in memory".to_owned(),
        |len| len.to_string(),
    )
}

/// Detailed error type for correlation matrix operations.
///
/// Validation variants preserve the first failure detected while checking a
/// row-major flattened correlation matrix. Iterative correlation algorithms
/// can additionally report convergence and eigendecomposition failures through
/// this shared public error surface.
#[derive(Debug, Clone, PartialEq, Error, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CorrelationError {
    /// Matrix size does not match expected n×n.
    #[error("Invalid matrix size: expected {expected}×{expected} entries, got {actual}")]
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
    /// Iterative algorithm exhausted its iteration budget before converging.
    #[error("Did not converge within {max_iter} iterations (tolerance {tol})")]
    DidNotConverge {
        /// Iteration budget that was exhausted.
        max_iter: usize,
        /// Frobenius-norm convergence tolerance that was not reached.
        tol: f64,
    },
    /// Symmetric eigendecomposition failed during a correlation operation.
    #[error("Symmetric eigendecomposition failed")]
    EigenDecompositionFailed,
    /// Solver options for nearest-correlation projection are invalid.
    #[error("Invalid nearest-correlation options: max_iter={max_iter}, tol={tol}")]
    InvalidOptions {
        /// Requested iteration budget (must be positive).
        max_iter: usize,
        /// Requested Frobenius-norm tolerance (must be finite and positive).
        tol: f64,
    },
}

impl From<CorrelationError> for crate::Error {
    /// Fold a correlation failure into the core taxonomy: iterative failures
    /// become `InputError::SolverConvergenceFailed`, and everything else becomes
    /// a validation error with the full message.
    fn from(error: CorrelationError) -> Self {
        match error {
            CorrelationError::DidNotConverge { max_iter, tol } => {
                crate::Error::Input(InputError::SolverConvergenceFailed {
                    iterations: max_iter,
                    residual: tol,
                    last_x: f64::NAN,
                    reason: error.to_string(),
                })
            }
            CorrelationError::EigenDecompositionFailed => {
                crate::Error::Input(InputError::SolverConvergenceFailed {
                    iterations: 0,
                    residual: f64::NAN,
                    last_x: f64::NAN,
                    reason: error.to_string(),
                })
            }
            other => crate::Error::Validation(other.to_string()),
        }
    }
}

/// Error type for Cholesky decomposition failures.
#[derive(Debug, Clone, PartialEq, Error, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CholeskyError {
    /// Matrix is not positive semi-definite (diagonal element became negative).
    #[error(
        "Matrix is not positive semi-definite: diagonal element {diag} is negative (position [{row}, {row}])"
    )]
    NotPositiveDefinite {
        /// The negative diagonal value
        diag: f64,
        /// The row/column index where failure occurred
        row: usize,
    },
    /// Matrix is numerically singular (division by near-zero element).
    #[error(
        "Matrix is numerically singular: division by {value} (threshold {threshold}) at position [{row}, {col}])"
    )]
    Singular {
        /// The near-zero value that caused the failure
        value: f64,
        /// The row index
        row: usize,
        /// The column index
        col: usize,
        /// Scale-relative threshold used for this matrix
        threshold: f64,
    },
    /// Matrix dimension mismatch.
    #[error("Matrix dimension mismatch: expected {expected}x{expected}, got {actual} elements")]
    DimensionMismatch {
        /// Expected dimension
        expected: usize,
        /// Actual number of elements
        actual: usize,
    },
    /// Matrix contains a non-finite entry (NaN or infinity).
    #[error("Matrix contains non-finite entry {value} at position [{row}, {col}]")]
    NonFiniteInput {
        /// The offending value
        value: f64,
        /// The row index
        row: usize,
        /// The column index
        col: usize,
    },
    /// Matrix is not symmetric within the relative symmetry tolerance.
    #[error(
        "Matrix is not symmetric at [{row}, {col}]: |A[{row},{col}] - A[{col},{row}]| = {diff} > {tolerance}"
    )]
    NotSymmetric {
        /// Upper-triangle row index where asymmetry was detected.
        row: usize,
        /// Upper-triangle column index where asymmetry was detected.
        col: usize,
        /// Absolute difference `|A[row, col] - A[col, row]|`.
        diff: f64,
        /// Scale-relative symmetry tolerance that was exceeded.
        tolerance: f64,
    },
    /// Computed correlated shock overflowed to a non-finite value.
    #[error("Computed correlated shock at row {row} is non-finite: {value}")]
    NonFiniteOutput {
        /// Non-finite computed entry.
        value: f64,
        /// Row index where non-finite output occurred.
        row: usize,
    },
}

impl CholeskyError {
    /// Coarse semantic error category matching [`crate::Error::kind`].
    #[must_use]
    pub const fn kind(&self) -> crate::ErrorKind {
        match self {
            Self::DimensionMismatch { .. }
            | Self::NonFiniteInput { .. }
            | Self::NotSymmetric { .. } => crate::ErrorKind::Validation,
            Self::NotPositiveDefinite { .. }
            | Self::Singular { .. }
            | Self::NonFiniteOutput { .. } => crate::ErrorKind::Computation,
        }
    }
}

// ─── Pivoted correlation Cholesky ─────────────────────────────────────────────
