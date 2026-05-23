//! Correlation matrix utilities shared across credit, rates, and portfolio
//! analytics.
//!
//! These helpers were originally in `finstack-valuations::correlation` but were
//! relocated so that downstream crates (e.g. `finstack-factor-model`) can
//! consume them without taking a dependency on `finstack-valuations`.
//!
//! # Components
//!
//! - [`error::Error`]: Structured validation diagnostics
//! - [`nearest_correlation::nearest_correlation_matrix`][]: Higham (2002)
//!   alternating-projection PSD repair
//! - [`validate_correlation_matrix`]: Thin wrapper over
//!   [`finstack_core::math::linalg::validate_correlation_matrix`] that
//!   classifies failures into [`error::Error`] variants

pub mod error;
pub mod nearest_correlation;

pub use error::{Error, Result};
pub use nearest_correlation::{nearest_correlation_matrix, NearestCorrelationOpts};

/// Tolerance used by [`validate_correlation_matrix`] to classify diagonal /
/// symmetry / boundedness violations.
const CORRELATION_TOLERANCE: f64 = 1e-10;

/// Validate a flattened row-major correlation matrix.
///
/// Delegates to [`finstack_core::math::linalg::validate_correlation_matrix`]
/// for the actual checks and classifies the first failure into an
/// [`error::Error`] variant for diagnostics.
///
/// Checks performed:
/// - Correct size (`matrix.len() == n * n`)
/// - Unit diagonal (within [`CORRELATION_TOLERANCE`])
/// - Symmetry (within [`CORRELATION_TOLERANCE`])
/// - All values within `[-1, 1]` (within [`CORRELATION_TOLERANCE`])
/// - Positive semi-definiteness (via Cholesky)
///
/// # Errors
///
/// Returns the first [`error::Error`] variant detected.
///
/// # Examples
///
/// ```
/// use finstack_analytics::correlation::validate_correlation_matrix;
///
/// let corr = vec![1.0, 0.5, 0.5, 1.0];
/// assert!(validate_correlation_matrix(&corr, 2).is_ok());
/// ```
pub fn validate_correlation_matrix(matrix: &[f64], n: usize) -> Result<()> {
    if matrix.len() != n * n {
        return Err(Error::InvalidSize {
            expected: n,
            actual: matrix.len(),
        });
    }
    if n == 0 {
        return Ok(());
    }

    // Diagonal
    for i in 0..n {
        let v = matrix[i * n + i];
        if (v - 1.0).abs() > CORRELATION_TOLERANCE {
            return Err(Error::DiagonalNotOne { index: i, value: v });
        }
    }

    // Symmetry and bounds
    for i in 0..n {
        for j in 0..n {
            let v = matrix[i * n + j];
            if !(-1.0 - CORRELATION_TOLERANCE..=1.0 + CORRELATION_TOLERANCE).contains(&v) {
                return Err(Error::OutOfBounds { i, j, value: v });
            }
            if i < j {
                let diff = (matrix[i * n + j] - matrix[j * n + i]).abs();
                if diff > CORRELATION_TOLERANCE {
                    return Err(Error::NotSymmetric { i, j, diff });
                }
            }
        }
    }

    // PSD via Cholesky
    if let Err(err) = finstack_core::math::linalg::cholesky_correlation(matrix, n) {
        let row = match err {
            finstack_core::math::linalg::CholeskyError::NotPositiveDefinite { row, .. } => row,
            _ => 0,
        };
        return Err(Error::NotPositiveSemiDefinite { row });
    }

    Ok(())
}
