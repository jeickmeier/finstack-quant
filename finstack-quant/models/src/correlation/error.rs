//! Models-owned correlation errors.
//!
//! Matrix validation and nearest-correlation failures come from
//! [`finstack_quant_analytics::correlation::Error`]. This enum adds the
//! credit-domain variants that analytics never constructs: factor volatilities,
//! recovery inputs, and Student-t degrees of freedom.

use finstack_quant_analytics::correlation::Error as MatrixError;
use finstack_quant_core::math::linalg::CholeskyError;

/// Convenience result type for models correlation constructors.
pub type Result<T> = std::result::Result<T, Error>;

/// Error type for models correlation constructors and matrix validation.
///
/// Matrix failures wrap [`MatrixError`] via [`From`]. Domain variants are
/// raised only by latent-factor, recovery, and copula constructors. Cholesky
/// failures preserve the canonical core error and its structured diagnostics.
#[derive(Debug, Clone, PartialEq, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Error {
    /// Matrix validation or nearest-correlation failure from analytics.
    #[error(transparent)]
    Matrix(
        /// Analytics matrix-validation or nearest-correlation failure.
        #[from]
        MatrixError,
    ),
    /// Cholesky factorization failure from core linear algebra.
    #[error(transparent)]
    Cholesky(
        /// Canonical factorization error, including dimensions or the offending
        /// row, column, value, diagonal, and numerical threshold where relevant.
        #[from]
        CholeskyError,
    ),
    /// Volatility vector length does not match number of factors.
    ///
    /// Returned by validated factor-model constructors when the caller supplies
    /// a volatility vector whose length disagrees with the declared number of
    /// factors.
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
    /// is `< 0.0` or non-finite.
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
    /// Random-factor-loading volatility is non-finite or outside its range.
    #[error("Invalid RFL loading volatility {value}: must be finite and in [0, 0.5]")]
    InvalidLoadingVolatility {
        /// The offending loading standard deviation.
        value: f64,
    },
}

impl From<Error> for finstack_quant_core::Error {
    /// Fold a correlation error into the canonical core error taxonomy.
    ///
    /// Iterative failures (`DidNotConverge`, `EigenDecompositionFailed`) map
    /// to [`finstack_quant_core::InputError::SolverConvergenceFailed`] so
    /// hosts classify them as computation errors; every other variant is an
    /// input-validation failure and maps to
    /// [`finstack_quant_core::Error::Validation`] with the full message.
    fn from(error: Error) -> Self {
        match error {
            Error::Matrix(
                matrix @ (MatrixError::DidNotConverge { .. }
                | MatrixError::EigenDecompositionFailed),
            ) => {
                let (iterations, residual) = match matrix {
                    MatrixError::DidNotConverge { max_iter, tol } => (max_iter, tol),
                    _ => (0, f64::NAN),
                };
                finstack_quant_core::Error::Input(
                    finstack_quant_core::InputError::SolverConvergenceFailed {
                        iterations,
                        residual,
                        last_x: f64::NAN,
                        reason: matrix.to_string(),
                    },
                )
            }
            other => finstack_quant_core::Error::Validation(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::error::ErrorKind;

    #[test]
    fn convergence_failures_fold_to_computation_kind() {
        let err: finstack_quant_core::Error = Error::Matrix(MatrixError::DidNotConverge {
            max_iter: 10,
            tol: 1e-8,
        })
        .into();
        assert_eq!(err.kind(), ErrorKind::Computation);
        assert!(err.to_string().contains("10 iterations"));
        let err: finstack_quant_core::Error =
            Error::Matrix(MatrixError::EigenDecompositionFailed).into();
        assert_eq!(err.kind(), ErrorKind::Computation);
    }

    #[test]
    fn validation_failures_fold_to_validation_kind() {
        let err: finstack_quant_core::Error =
            Error::InvalidStudentTDegreesOfFreedom { value: 1.0 }.into();
        assert_eq!(err.kind(), ErrorKind::Validation);
        assert!(err.to_string().contains("degrees of freedom"));
        let err: finstack_quant_core::Error = Error::Matrix(MatrixError::DiagonalNotOne {
            index: 0,
            value: 2.0,
        })
        .into();
        assert_eq!(err.kind(), ErrorKind::Validation);
    }
}
