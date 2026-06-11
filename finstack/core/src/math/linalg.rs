//! Linear algebra utilities for correlation and covariance matrices.
//!
//! Provides essential matrix operations for financial modeling, particularly
//! Cholesky decomposition for generating correlated random variables in Monte
//! Carlo simulations and portfolio risk calculations.
//!
//! # Algorithms
//!
//! - **Cholesky decomposition** (unpivoted): Factorize Σ = L L^T for positive definite
//!   matrices. Used for solver normal equations via [`cholesky_decomposition`] and
//!   [`cholesky_solve`].
//! - **Pivoted Cholesky for correlation matrices**: Numerically robust factorization via
//!   [`cholesky_correlation`] using relative-tolerance diagonal pivoting. Handles
//!   near-singular and positive-semidefinite correlation matrices safely. The permutation
//!   is internalized; the returned [`CorrelationFactor`] applies shocks in original
//!   variable order.
//! - **Correlation application**: Transform independent normals to correlated via L
//! - **Matrix validation**: Check positive-definiteness and correlation properties
//!
//! # Use Cases
//!
//! - **Monte Carlo**: Generate correlated asset paths — use [`cholesky_correlation`]
//! - **Portfolio risk**: Covariance matrix factorization for VaR
//! - **Factor models**: Decompose returns into systematic factors
//! - **Copula models**: Correlation structure in credit derivatives
//!
//! # Examples
//!
//! ```
//! use finstack_core::math::linalg::{cholesky_correlation, apply_correlation};
//!
//! // 2x2 correlation matrix: [[1.0, 0.5], [0.5, 1.0]]
//! let corr = vec![1.0, 0.5, 0.5, 1.0];
//! let factor = cholesky_correlation(&corr, 2).expect("valid correlation matrix");
//!
//! // Transform independent standard normals to correlated
//! let z = vec![1.0, 0.0]; // Independent N(0,1) shocks
//! let mut z_corr = vec![0.0; 2];
//! factor.apply(&z, &mut z_corr).unwrap();
//! // z_corr now contains correlated shocks with correlation 0.5
//! ```
//!
//! # References
//!
//! - **Pivoted Cholesky**:
//!   - Higham, N. J. (2002). *Accuracy and Stability of Numerical Algorithms* (2nd ed.).
//!     SIAM. Algorithm 10.2 (Cholesky with complete pivoting).
//!   - Golub, G. H., & Van Loan, C. F. (2013). *Matrix Computations* (4th ed.).
//!     Johns Hopkins University Press. Algorithm 4.2.5.
//!
//! - **Correlation Matrices**:
//!   - Rebonato, R., & Jäckel, P. (2000). "The Most General Methodology to Create
//!     a Valid Correlation Matrix for Risk Management and Option Pricing Purposes."
//!     *Journal of Risk*, 2(2), 17-27.
//!
//! - **Monte Carlo Applications**:
//!   - Glasserman, P. (2003). *Monte Carlo Methods in Financial Engineering*.
//!     Springer. Section 2.4 (Generating multivariate samples).

use crate::{error, Result};
use thiserror::Error;

/// Default singular threshold for Cholesky decomposition.
///
/// Used only by the generic unpivoted path (`cholesky_decomposition` / `cholesky_solve`).
/// The correlation-specific path (`cholesky_correlation`) uses a *relative* tolerance
/// computed from the matrix's own diagonal magnitude.
pub const SINGULAR_THRESHOLD: f64 = 1e-10;

/// Default tolerance for diagonal elements in correlation matrices.
pub const DIAGONAL_TOLERANCE: f64 = 1e-6;

/// Default tolerance for symmetry checks in correlation matrices.
pub const SYMMETRY_TOLERANCE: f64 = 1e-6;

/// Relative pivot tolerance for [`cholesky_correlation`].
///
/// A pivot is considered numerically zero when it is below
/// `PIVOT_TOLERANCE_RELATIVE * max_diagonal`. This makes the threshold
/// scale-invariant and appropriate for both normalised correlation matrices
/// (diagonal ≈ 1) and general covariance matrices with large entries.
pub const PIVOT_TOLERANCE_RELATIVE: f64 = 1e-10;

/// Error type for Cholesky decomposition failures.
#[derive(Debug, Clone, PartialEq, Error, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum CholeskyError {
    /// Matrix is not positive semi-definite (diagonal element became negative).
    #[error("Matrix is not positive semi-definite: diagonal element {diag} is negative (position [{row}, {row}])")]
    NotPositiveDefinite {
        /// The negative diagonal value
        diag: f64,
        /// The row/column index where failure occurred
        row: usize,
    },
    /// Matrix is numerically singular (division by near-zero element).
    #[error("Matrix is numerically singular: division by {value} (threshold {threshold}) at position [{row}, {col}])")]
    Singular {
        /// The near-zero value that caused the failure
        value: f64,
        /// The row index
        row: usize,
        /// The column index
        col: usize,
        /// The threshold used (1e-10)
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
}

// ─── Pivoted correlation Cholesky ─────────────────────────────────────────────

/// Cholesky factor for a correlation or covariance matrix, computed with complete
/// diagonal pivoting for numerical robustness.
///
/// The factor is stored in the **original variable ordering** — the permutation that
/// was applied internally during factorisation is inverted before storage so that
/// callers do not need to think about pivot order. In particular, [`apply`] and
/// [`factor_matrix`] produce outputs aligned with the input variable indices.
///
/// # Near-singular and semidefinite matrices
///
/// Factorisation stops when the largest remaining diagonal element drops below
/// `PIVOT_TOLERANCE_RELATIVE * max_diagonal`. Rows/columns beyond that point are
/// treated as numerically zero, so rank-deficient correlation matrices (e.g. when
/// one asset is a perfect linear combination of others) are handled gracefully rather
/// than rejected outright.
///
/// If a diagonal becomes *negative* beyond floating-point noise the matrix is not
/// positive-semidefinite and [`cholesky_correlation`] returns an error.
///
/// # References
///
/// - Higham, N. J. (2002). *Accuracy and Stability of Numerical Algorithms* (2nd ed.).
///   SIAM. Algorithm 10.2 (Cholesky with complete pivoting).
///
/// [`apply`]: CorrelationFactor::apply
/// [`factor_matrix`]: CorrelationFactor::factor_matrix
#[derive(Debug, Clone)]
pub struct CorrelationFactor {
    /// Factor matrix in original variable order (n×n, row-major).
    ///
    /// After pivoted Cholesky and unpermutation, this matrix satisfies
    /// `factor * factor^T = correlation` in original variable order, but it is
    /// not guaranteed to remain lower triangular.
    factor: Vec<f64>,
    /// Matrix dimension.
    n: usize,
    /// Number of numerically non-zero pivots (effective rank).
    ///
    /// For well-conditioned correlation matrices this equals `n`. For
    /// near-singular matrices it may be smaller.
    effective_rank: usize,
}

impl CorrelationFactor {
    /// Matrix dimension.
    #[must_use]
    pub fn n(&self) -> usize {
        self.n
    }

    /// Effective numerical rank (number of pivots above relative tolerance).
    ///
    /// For a well-conditioned full-rank correlation matrix this equals `n`.
    /// For a rank-deficient or near-singular matrix it is less than `n`.
    #[must_use]
    pub fn effective_rank(&self) -> usize {
        self.effective_rank
    }

    /// Whether the matrix is numerically full-rank.
    #[must_use]
    pub fn is_full_rank(&self) -> bool {
        self.effective_rank == self.n
    }

    /// Factor matrix in original variable order (n×n, row-major).
    ///
    /// The returned matrix satisfies `factor * factor^T = correlation` in
    /// original variable order. When pivoting occurs it may contain non-zero
    /// entries above the diagonal.
    #[must_use]
    pub fn factor_matrix(&self) -> &[f64] {
        &self.factor
    }

    /// Apply the stored factor to independent N(0,1) shocks to produce
    /// correlated shocks.
    ///
    /// Computes `z_corr = factor * z_indep` in original variable order. Both
    /// slices must have length `n`.
    ///
    /// # Errors
    ///
    /// Returns [`CholeskyError::DimensionMismatch`] if either slice length differs from `n`.
    pub fn apply(
        &self,
        independent: &[f64],
        correlated: &mut [f64],
    ) -> std::result::Result<(), CholeskyError> {
        if independent.len() != self.n {
            return Err(CholeskyError::DimensionMismatch {
                expected: self.n,
                actual: independent.len(),
            });
        }
        if correlated.len() != self.n {
            return Err(CholeskyError::DimensionMismatch {
                expected: self.n,
                actual: correlated.len(),
            });
        }
        let n = self.n;
        for (i, out) in correlated.iter_mut().enumerate() {
            let mut sum = 0.0;
            for (j, &z_j) in independent.iter().enumerate() {
                sum += self.factor[i * n + j] * z_j;
            }
            *out = sum;
        }
        Ok(())
    }

    /// Construct directly from a pre-validated lower-triangular factor slice.
    ///
    /// `factor` must be `n * n` elements; upper triangle is ignored (expected zero).
    /// `effective_rank` must satisfy `effective_rank <= n`.
    #[must_use]
    pub fn from_parts(factor: Vec<f64>, n: usize, effective_rank: usize) -> Self {
        debug_assert_eq!(factor.len(), n * n);
        debug_assert!(effective_rank <= n);
        Self {
            factor,
            n,
            effective_rank,
        }
    }
}

/// Compute the Cholesky factorisation of a correlation or covariance matrix using
/// complete diagonal pivoting for numerical robustness.
///
/// This is the **recommended function for correlation-matrix consumers** (Monte Carlo,
/// factor models, copulas). For solver normal equations use [`cholesky_decomposition`]
/// and [`cholesky_solve`] instead.
///
/// # Algorithm
///
/// At each step the largest remaining diagonal element is selected as the pivot
/// (Higham's Algorithm 10.2). If it is below
/// `PIVOT_TOLERANCE_RELATIVE * max(max_diagonal, 1.0)` factorisation stops and the
/// remaining block is treated as numerically zero (semidefinite truncation). If it is
/// strictly negative by more than floating-point noise the matrix is indefinite and an
/// error is returned.
///
/// The permutation is inverted before storage so the returned factor is in the
/// **original variable ordering** of the input matrix.
///
/// # Arguments
///
/// * `matrix` — Symmetric positive-semidefinite matrix (n×n, row-major)
/// * `n` — Matrix dimension
///
/// # Returns
///
/// [`CorrelationFactor`] with the unpermuted lower-triangular factor.
///
/// # Errors
///
/// - [`CholeskyError::DimensionMismatch`] if `matrix.len() != n * n`
/// - [`CholeskyError::NonFiniteInput`] if any entry is NaN or infinite
/// - [`CholeskyError::NotPositiveDefinite`] if a pivot is significantly negative
///
/// # Example
///
/// ```
/// use finstack_core::math::linalg::cholesky_correlation;
///
/// // Well-conditioned 2×2
/// let corr = vec![1.0, 0.5, 0.5, 1.0];
/// let f = cholesky_correlation(&corr, 2).unwrap();
/// assert!(f.is_full_rank());
///
/// // Near-singular (rho ≈ 1): pivoted Cholesky handles it gracefully
/// let near_singular = vec![1.0, 0.9999999, 0.9999999, 1.0];
/// let f2 = cholesky_correlation(&near_singular, 2).unwrap();
/// assert!(f2.effective_rank() <= 2);
/// ```
pub fn cholesky_correlation(
    matrix: &[f64],
    n: usize,
) -> std::result::Result<CorrelationFactor, CholeskyError> {
    if matrix.len() != n * n {
        return Err(CholeskyError::DimensionMismatch {
            expected: n,
            actual: matrix.len(),
        });
    }
    if n == 0 {
        return Ok(CorrelationFactor::from_parts(vec![], 0, 0));
    }

    // Reject non-finite inputs up front: NaN otherwise propagates through the
    // pivot selection (`total_cmp` orders NaN above all numbers) and would be
    // absorbed into an Ok(NaN factor).
    for row in 0..n {
        for col in 0..n {
            let value = matrix[row * n + col];
            if !value.is_finite() {
                return Err(CholeskyError::NonFiniteInput { value, row, col });
            }
        }
    }

    // Maximum diagonal value — used to set relative tolerance.
    let max_diag = (0..n)
        .map(|i| matrix[i * n + i])
        .fold(f64::NEG_INFINITY, f64::max);
    let tol = PIVOT_TOLERANCE_RELATIVE * max_diag.abs().max(1.0);

    // Work copy of the matrix that we reduce in-place (Schur complement updates).
    let mut a: Vec<f64> = matrix.to_vec();

    // perm[k] holds the original variable index placed at pivot position k.
    let mut perm: Vec<usize> = (0..n).collect();

    // Lower-triangular factor in pivoted order (unpermuted before return).
    let mut l_piv = vec![0.0_f64; n * n];

    let mut effective_rank = 0usize;

    for step in 0..n {
        // Complete diagonal pivoting: find largest remaining diagonal.
        let pivot_idx = (step..n)
            .max_by(|&ai, &bi| a[ai * n + ai].total_cmp(&a[bi * n + bi]))
            .unwrap_or(step);

        let pivot_val = a[pivot_idx * n + pivot_idx];

        // Significantly negative pivot → matrix is indefinite.
        if pivot_val < -tol {
            return Err(CholeskyError::NotPositiveDefinite {
                diag: pivot_val,
                row: perm[pivot_idx],
            });
        }

        // Pivot below relative tolerance → semidefinite truncation.
        if pivot_val <= tol {
            break;
        }

        effective_rank += 1;

        // Symmetric row/column swap to move the best pivot to position `step`.
        if pivot_idx != step {
            perm.swap(step, pivot_idx);
            for col in 0..n {
                a.swap(step * n + col, pivot_idx * n + col);
            }
            for row in 0..n {
                a.swap(row * n + step, row * n + pivot_idx);
            }
            for col in 0..step {
                l_piv.swap(step * n + col, pivot_idx * n + col);
            }
        }

        // Diagonal entry of L.
        let l_kk = pivot_val.sqrt();
        l_piv[step * n + step] = l_kk;

        // Sub-diagonal column of L.
        for row in (step + 1)..n {
            l_piv[row * n + step] = a[row * n + step] / l_kk;
        }

        // Schur complement update (rank-1 downdate of remaining block).
        for row in (step + 1)..n {
            let l_row_step = l_piv[row * n + step];
            for col in (step + 1)..=row {
                let update = l_row_step * l_piv[col * n + step];
                a[row * n + col] -= update;
                a[col * n + row] = a[row * n + col];
            }
        }
    }

    // Unpermute: place L_piv columns/rows back in original variable order.
    // perm[k] = original variable index placed at pivot position k.
    //
    // For the active pivots (k < effective_rank): L_orig[perm[i], perm[j]] = L_piv[i, j].
    // For the truncated rows (k >= effective_rank): only the j < effective_rank
    // sub-diagonal entries are non-trivial; the diagonal and entries beyond are zero.
    let mut l_orig = vec![0.0_f64; n * n];
    for i in 0..n {
        let orig_row = perm[i];
        for j in 0..i.min(effective_rank) {
            l_orig[orig_row * n + perm[j]] = l_piv[i * n + j];
        }
        // Diagonal entry only if within active rank.
        if i < effective_rank {
            l_orig[orig_row * n + perm[i]] = l_piv[i * n + i];
        }
    }

    Ok(CorrelationFactor::from_parts(l_orig, n, effective_rank))
}

// ─── Symmetric eigendecomposition (shared helper) ──────────────────────────────

/// Symmetric eigendecomposition of an `n × n` matrix stored row-major.
///
/// Returns `(eigenvalues, eigenvectors)` where `eigenvectors[i * n + k]` is
/// the `i`-th component of the `k`-th eigenvector — i.e. column-major in the
/// eigenvector index, matching the convention used by
/// [`apply_correlation`] and by most linear-algebra references.
///
/// The routine delegates to `nalgebra::SymmetricEigen` (Householder
/// tridiagonalization followed by symmetric QR iteration, `O(n³)`), which is
/// numerically stable for the symmetric-
/// matrix case and significantly faster than hand-rolled Jacobi sweeps for
/// `n > 30`. It tolerates non-positive-definite input — callers that need
/// the PSD projection step can clamp negative eigenvalues before
/// reconstructing.
///
/// # Errors
///
/// Returns [`CholeskyError::DimensionMismatch`] if `matrix.len() != n * n`.
///
/// # Example
///
/// ```
/// use finstack_core::math::linalg::symmetric_eigen;
///
/// // Diagonal matrix: eigenvalues are the diagonal entries.
/// let m = vec![2.0, 0.0, 0.0, 5.0];
/// let (vals, _vecs) = symmetric_eigen(&m, 2).expect("symmetric");
/// let mut sorted = vals.clone();
/// sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
/// assert!((sorted[0] - 2.0).abs() < 1e-12);
/// assert!((sorted[1] - 5.0).abs() < 1e-12);
/// ```
///
/// Exposed as a shared helper so downstream crates (`finstack-valuations`,
/// etc.) can delegate their eigendecomposition without pulling `nalgebra`
/// in as a direct dependency.
pub fn symmetric_eigen(
    matrix: &[f64],
    n: usize,
) -> std::result::Result<(Vec<f64>, Vec<f64>), CholeskyError> {
    if matrix.len() != n * n {
        return Err(CholeskyError::DimensionMismatch {
            expected: n,
            actual: matrix.len(),
        });
    }
    if n == 0 {
        return Ok((Vec::new(), Vec::new()));
    }

    let a = nalgebra::DMatrix::from_fn(n, n, |i, j| matrix[i * n + j]);
    let eig = nalgebra::SymmetricEigen::new(a);
    let eigenvalues: Vec<f64> = (0..n).map(|i| eig.eigenvalues[i]).collect();
    let mut eigenvectors = vec![0.0_f64; n * n];
    for i in 0..n {
        for k in 0..n {
            eigenvectors[i * n + k] = eig.eigenvectors[(i, k)];
        }
    }
    Ok((eigenvalues, eigenvectors))
}

// ─── Generic (unpivoted) Cholesky — solver path ────────────────────────────────

/// Cholesky decomposition of a correlation/covariance matrix.
///
/// Computes L such that Σ = L L^T, where Σ is the correlation matrix.
/// Uses the standard algorithm with numerical stability improvements.
///
/// # Arguments
///
/// * `matrix` - Symmetric positive definite matrix (n x n, row-major)
/// * `n` - Matrix dimension
///
/// # Returns
///
/// Lower triangular Cholesky factor L (n x n, row-major)
///
/// # Errors
///
/// Returns `CholeskyError` if:
/// - Matrix is not positive semi-definite (diagonal becomes negative)
/// - Matrix is numerically singular (division by near-zero)
/// - Matrix dimensions don't match
///
/// # Example
///
/// ```
/// use finstack_core::math::linalg::cholesky_decomposition;
///
/// // Correlation matrix: [[1.0, 0.5], [0.5, 1.0]]
/// let corr = vec![1.0, 0.5, 0.5, 1.0];
/// let chol = cholesky_decomposition(&corr, 2).expect("Cholesky decomposition should succeed");
/// // chol = [[1.0, 0.0], [0.5, 0.866...]]
/// ```
pub fn cholesky_decomposition(
    matrix: &[f64],
    n: usize,
) -> std::result::Result<Vec<f64>, CholeskyError> {
    if matrix.len() != n * n {
        return Err(CholeskyError::DimensionMismatch {
            expected: n,
            actual: matrix.len(),
        });
    }

    let mut l = vec![0.0; n * n];

    for i in 0..n {
        for j in 0..=i {
            let mut sum = 0.0;
            for k in 0..j {
                sum += l[i * n + k] * l[j * n + k];
            }

            if i == j {
                let diag = matrix[i * n + i] - sum;
                if diag < 0.0 {
                    return Err(CholeskyError::NotPositiveDefinite { diag, row: i });
                }
                l[i * n + j] = diag.sqrt();
                // Check if diagonal is too small (singular)
                if l[i * n + j].abs() < SINGULAR_THRESHOLD {
                    return Err(CholeskyError::Singular {
                        value: l[i * n + j],
                        row: i,
                        col: j,
                        threshold: SINGULAR_THRESHOLD,
                    });
                }
            } else {
                if l[j * n + j].abs() < SINGULAR_THRESHOLD {
                    return Err(CholeskyError::Singular {
                        value: l[j * n + j],
                        row: i,
                        col: j,
                        threshold: SINGULAR_THRESHOLD,
                    });
                }
                l[i * n + j] = (matrix[i * n + j] - sum) / l[j * n + j];
            }
        }
    }

    Ok(l)
}

/// Cholesky decomposition into a caller-provided buffer (avoids allocation).
///
/// The output buffer `l` must have length `n * n` and will be overwritten.
pub fn cholesky_decomposition_into(
    matrix: &[f64],
    n: usize,
    l: &mut [f64],
) -> std::result::Result<(), CholeskyError> {
    if matrix.len() != n * n || l.len() != n * n {
        return Err(CholeskyError::DimensionMismatch {
            expected: n,
            actual: matrix.len(),
        });
    }

    l.fill(0.0);

    for i in 0..n {
        for j in 0..=i {
            let mut sum = 0.0;
            for k in 0..j {
                sum += l[i * n + k] * l[j * n + k];
            }

            if i == j {
                let diag = matrix[i * n + i] - sum;
                if diag < 0.0 {
                    return Err(CholeskyError::NotPositiveDefinite { diag, row: i });
                }
                l[i * n + j] = diag.sqrt();
                if l[i * n + j].abs() < SINGULAR_THRESHOLD {
                    return Err(CholeskyError::Singular {
                        value: l[i * n + j],
                        row: i,
                        col: j,
                        threshold: SINGULAR_THRESHOLD,
                    });
                }
            } else {
                if l[j * n + j].abs() < SINGULAR_THRESHOLD {
                    return Err(CholeskyError::Singular {
                        value: l[j * n + j],
                        row: i,
                        col: j,
                        threshold: SINGULAR_THRESHOLD,
                    });
                }
                l[i * n + j] = (matrix[i * n + j] - sum) / l[j * n + j];
            }
        }
    }

    Ok(())
}

/// Apply correlation via Cholesky factor to independent shocks.
///
/// Transforms independent N(0,1) shocks into correlated shocks.
///
/// # Arguments
///
/// * `chol` - Lower triangular Cholesky factor (n x n, row-major)
/// * `independent` - Independent shocks (length n)
/// * `correlated` - Output correlated shocks (length n)
///
/// # Errors
///
/// Returns `CholeskyError::DimensionMismatch` if:
/// - `chol.len() != independent.len() * independent.len()` (Cholesky factor is not n x n)
/// - `correlated.len() != independent.len()` (output buffer has wrong length)
///
/// # Example
///
/// ```
/// use finstack_core::math::linalg::{cholesky_decomposition, apply_correlation};
///
/// let corr = vec![1.0, 0.5, 0.5, 1.0];
/// let chol = cholesky_decomposition(&corr, 2).expect("Cholesky decomposition should succeed");
///
/// let z = vec![1.0, 0.0]; // Independent shocks
/// let mut z_corr = vec![0.0; 2];
/// apply_correlation(&chol, &z, &mut z_corr).expect("dimensions match");
/// ```
pub fn apply_correlation(
    chol: &[f64],
    independent: &[f64],
    correlated: &mut [f64],
) -> std::result::Result<(), CholeskyError> {
    let n = independent.len();
    if chol.len() != n * n {
        return Err(CholeskyError::DimensionMismatch {
            expected: n * n,
            actual: chol.len(),
        });
    }
    if correlated.len() != n {
        return Err(CholeskyError::DimensionMismatch {
            expected: n,
            actual: correlated.len(),
        });
    }

    for i in 0..n {
        correlated[i] = 0.0;
        for j in 0..=i {
            correlated[i] += chol[i * n + j] * independent[j];
        }
    }

    Ok(())
}

/// Solve linear system Ax = b using Cholesky decomposition L of A (A = L L^T).
///
/// Solves L y = b (forward substitution) then L^T x = y (backward substitution).
///
/// # Arguments
///
/// * `chol` - Lower triangular Cholesky factor L (n x n, row-major)
/// * `b` - Right-hand side vector (length n)
/// * `x` - Output solution vector (length n)
pub fn cholesky_solve(chol: &[f64], b: &[f64], x: &mut [f64]) -> Result<()> {
    let n = b.len();
    if chol.len() != n * n || x.len() != n {
        return Err(crate::error::InputError::DimensionMismatch.into());
    }
    let diagonal_scale = (0..n)
        .map(|i| chol[i * n + i].abs())
        .fold(0.0_f64, f64::max)
        .max(f64::MIN_POSITIVE);
    let singular_threshold = SINGULAR_THRESHOLD * diagonal_scale;

    // Forward substitution: Solve L y = b
    for i in 0..n {
        let mut sum = 0.0;
        for j in 0..i {
            sum += chol[i * n + j] * x[j];
        }
        let diag = chol[i * n + i];
        if diag.abs() < singular_threshold {
            return Err(crate::error::InputError::Invalid.into());
        }
        x[i] = (b[i] - sum) / diag;
    }

    // Backward substitution: Solve L^T x = y
    // x currently holds y
    for i in (0..n).rev() {
        let mut sum = 0.0;
        for j in (i + 1)..n {
            sum += chol[j * n + i] * x[j]; // L[j][i] is L^T[i][j]
        }
        // diag is the same L[i][i]
        let diag = chol[i * n + i];
        if diag.abs() < singular_threshold {
            return Err(crate::error::InputError::Invalid.into());
        }
        x[i] = (x[i] - sum) / diag;
    }

    Ok(())
}

/// Helper to create correlation matrix from correlation pairs.
///
/// # Arguments
///
/// * `n` - Matrix dimension
/// * `correlations` - List of (i, j, ρ_ij) tuples
///
/// # Returns
///
/// Symmetric correlation matrix (n x n, row-major)
///
/// # Errors
///
/// Returns [`CholeskyError::DimensionMismatch`] if an index in `correlations`
/// is out of bounds, or [`crate::Error::Validation`] if a diagonal pair `(i, i)` is
/// supplied.
///
/// # Example
///
/// ```
/// use finstack_core::math::linalg::build_correlation_matrix;
///
/// let correlations = vec![(0, 1, 0.5)];
/// let matrix = build_correlation_matrix(2, &correlations).unwrap();
/// // matrix = [[1.0, 0.5], [0.5, 1.0]]
/// ```
pub fn build_correlation_matrix(
    n: usize,
    correlations: &[(usize, usize, f64)],
) -> crate::Result<Vec<f64>> {
    let mut matrix = vec![0.0; n * n];

    for i in 0..n {
        matrix[i * n + i] = 1.0;
    }

    for &(i, j, rho) in correlations {
        if i >= n || j >= n {
            return Err(crate::Error::Validation(format!(
                "correlation index out of bounds: ({i}, {j}) for matrix size {n}"
            )));
        }
        if i == j {
            return Err(crate::Error::Validation(format!(
                "correlation entry ({i}, {j}) is on the diagonal; diagonal elements are fixed at 1.0"
            )));
        }
        matrix[i * n + j] = rho;
        matrix[j * n + i] = rho;
    }

    Ok(matrix)
}

/// Validate that a matrix is a valid correlation matrix.
///
/// Checks:
/// 1. Diagonal elements are 1.0
/// 2. Off-diagonal elements are in [-1, 1]
/// 3. Matrix is symmetric
/// 4. Matrix is positive semi-definite (via Cholesky)
///
/// # Example
///
/// ```
/// use finstack_core::math::linalg::validate_correlation_matrix;
///
/// let valid = vec![1.0, 0.5, 0.5, 1.0];
/// assert!(validate_correlation_matrix(&valid, 2).is_ok());
///
/// let invalid = vec![1.0, 1.5, 1.5, 1.0]; // Correlation > 1
/// assert!(validate_correlation_matrix(&invalid, 2).is_err());
/// ```
pub fn validate_correlation_matrix(matrix: &[f64], n: usize) -> Result<()> {
    if matrix.len() != n * n {
        return Err(crate::error::InputError::DimensionMismatch.into());
    }

    // Check diagonal
    for i in 0..n {
        let diag = matrix[i * n + i];
        if (diag - 1.0).abs() > DIAGONAL_TOLERANCE {
            return Err(crate::error::InputError::Invalid.into());
        }
    }

    // Check off-diagonal range and symmetry
    for i in 0..n {
        for j in 0..n {
            if i == j {
                continue;
            }

            let val = matrix[i * n + j];
            if !(-1.0..=1.0).contains(&val) {
                return Err(crate::error::InputError::Invalid.into());
            }

            // Check symmetry
            let val_sym = matrix[j * n + i];
            if (val - val_sym).abs() > SYMMETRY_TOLERANCE {
                return Err(crate::error::InputError::Invalid.into());
            }
        }
    }

    // Check positive semi-definite using the pivoted path so that nearly-rank-deficient
    // but valid correlation matrices are accepted rather than rejected by an absolute
    // threshold.
    match cholesky_correlation(matrix, n) {
        Ok(_) => Ok(()),
        Err(CholeskyError::NotPositiveDefinite { .. }) => Err(error::InputError::Invalid.into()),
        Err(CholeskyError::DimensionMismatch { .. }) => {
            Err(error::InputError::DimensionMismatch.into())
        }
        // cholesky_correlation only produces NotPositiveDefinite and DimensionMismatch.
        Err(_) => Err(error::InputError::Invalid.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cholesky_2x2() {
        // Correlation matrix: [[1.0, 0.5], [0.5, 1.0]]
        let corr = vec![1.0, 0.5, 0.5, 1.0];
        let chol = cholesky_decomposition(&corr, 2)
            .expect("Cholesky decomposition should succeed in test");

        // Expected: [[1.0, 0.0], [0.5, 0.866...]]
        assert!((chol[0] - 1.0).abs() < 1e-10);
        assert!((chol[1] - 0.0).abs() < 1e-10);
        assert!((chol[2] - 0.5).abs() < 1e-10);
        assert!((chol[3] - 0.8660254037844387).abs() < 1e-10);
    }

    #[test]
    fn test_cholesky_identity() {
        let identity = vec![1.0, 0.0, 0.0, 1.0];
        let chol = cholesky_decomposition(&identity, 2)
            .expect("Cholesky decomposition should succeed in test");

        // Should equal identity
        assert_eq!(chol, identity);
    }

    #[test]
    fn test_apply_correlation() {
        let corr = vec![1.0, 0.5, 0.5, 1.0];
        let chol = cholesky_decomposition(&corr, 2)
            .expect("Cholesky decomposition should succeed in test");

        let z = vec![1.0, 0.0];
        let mut z_corr = vec![0.0; 2];
        apply_correlation(&chol, &z, &mut z_corr)
            .expect("apply_correlation should succeed in test");

        assert!((z_corr[0] - 1.0).abs() < 1e-10);
        assert!((z_corr[1] - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_build_correlation_matrix() {
        let correlations = vec![(0, 1, 0.5)];
        let matrix = build_correlation_matrix(2, &correlations).expect("valid correlations");

        assert!((matrix[0] - 1.0).abs() < 1e-10); // [0,0]
        assert!((matrix[1] - 0.5).abs() < 1e-10); // [0,1]
        assert!((matrix[2] - 0.5).abs() < 1e-10); // [1,0]
        assert!((matrix[3] - 1.0).abs() < 1e-10); // [1,1]
    }

    #[test]
    fn test_validate_correlation_matrix() {
        // Valid matrix
        let valid = vec![1.0, 0.5, 0.5, 1.0];
        assert!(validate_correlation_matrix(&valid, 2).is_ok());

        // Invalid: diagonal not 1.0
        let invalid_diag = vec![0.9, 0.5, 0.5, 1.0];
        assert!(validate_correlation_matrix(&invalid_diag, 2).is_err());

        // Invalid: off-diagonal > 1.0
        let invalid_range = vec![1.0, 1.5, 1.5, 1.0];
        assert!(validate_correlation_matrix(&invalid_range, 2).is_err());

        // Invalid: not symmetric
        let invalid_sym = vec![1.0, 0.5, 0.3, 1.0];
        assert!(validate_correlation_matrix(&invalid_sym, 2).is_err());

        // Invalid: not positive definite (correlation > 1 is invalid anyway)
        let invalid_pd = vec![1.0, 1.1, 1.1, 1.0];
        assert!(validate_correlation_matrix(&invalid_pd, 2).is_err());
    }

    #[test]
    fn test_validate_correlation_matrix_dimension_mismatch_returns_error() {
        let wrong_shape = vec![1.0, 0.5, 0.5];
        assert!(
            validate_correlation_matrix(&wrong_shape, 2).is_err(),
            "dimension mismatch should return Err instead of panicking"
        );
    }

    // ── H6 documentation tests: symmetry tolerance semantics ──
    //
    // The reviewer suggested switching to relative tolerance.
    // This was rejected because correlation entries are naturally bounded in
    // [-1, 1], making absolute tolerance scale-appropriate for this domain.
    // A pure relative tolerance would behave erratically near zero correlation.
    // These tests document the expected boundary behaviour.
    #[test]
    fn test_validate_correlation_matrix_symmetry_tolerance_boundary() {
        // Asymmetry below SYMMETRY_TOLERANCE (1e-6) is accepted.
        let almost_sym = vec![1.0, 0.5, 0.5 + 5.0e-7, 1.0];
        assert!(
            validate_correlation_matrix(&almost_sym, 2).is_ok(),
            "Near-symmetric matrix below tolerance should be accepted"
        );

        // Asymmetry just above SYMMETRY_TOLERANCE is rejected.
        let barely_asym = vec![1.0, 0.5, 0.5 + 2.0e-6, 1.0];
        assert!(
            validate_correlation_matrix(&barely_asym, 2).is_err(),
            "Asymmetry above tolerance should be rejected"
        );

        // Near-zero correlation: absolute tolerance still applies correctly.
        let near_zero_sym = vec![1.0, 1.0e-8, 1.0e-8, 1.0];
        assert!(
            validate_correlation_matrix(&near_zero_sym, 2).is_ok(),
            "Near-zero symmetric correlation should be accepted"
        );
    }

    #[test]
    fn test_cholesky_fails_on_non_pd() {
        // Not positive definite - use a matrix that fails Cholesky properly
        // Matrix with correlation slightly > 1 (clearly not valid)
        let non_pd = vec![1.0, 1.01, 1.01, 1.0];
        let result = cholesky_decomposition(&non_pd, 2);
        assert!(result.is_err());
        // Verify we get descriptive error
        match result.expect_err("Should fail for non-positive-definite matrix") {
            CholeskyError::NotPositiveDefinite { diag, row } => {
                assert!(diag < 0.0);
                assert!(row < 2);
            }
            _ => panic!("Expected NotPositiveDefinite error"),
        }
    }

    #[test]
    fn test_cholesky_descriptive_errors() {
        // Test dimension mismatch
        let small = vec![1.0, 0.5, 0.5, 1.0];
        match cholesky_decomposition(&small, 3) {
            Err(CholeskyError::DimensionMismatch { expected, actual }) => {
                assert_eq!(expected, 3);
                assert_eq!(actual, 4);
            }
            _ => panic!("Expected DimensionMismatch error"),
        }

        // Test near-singular matrix (correlation ≈ 1)
        let near_singular = vec![1.0, 0.9999, 0.9999, 1.0];
        // This might succeed or fail depending on numerical precision
        let result = cholesky_decomposition(&near_singular, 2);
        // Either way, we should get a descriptive error if it fails
        if let Err(e) = result {
            match e {
                CholeskyError::NotPositiveDefinite { .. } | CholeskyError::Singular { .. } => {}
                _ => panic!("Unexpected error type"),
            }
        }
    }

    // ── Pivoted Cholesky tests ────────────────────────────────────────────────

    /// Helper: multiply two n×n lower-triangular row-major matrices; returns L * L^T.
    fn mat_mul_lt(l: &[f64], n: usize) -> Vec<f64> {
        let mut out = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                let mut s = 0.0;
                // L * L^T: sum_k L[i,k] * L[j,k]
                for k in 0..n {
                    s += l[i * n + k] * l[j * n + k];
                }
                out[i * n + j] = s;
            }
        }
        out
    }

    #[test]
    fn pivoted_cholesky_2x2_well_conditioned() {
        let corr = vec![1.0, 0.5, 0.5, 1.0];
        let f = cholesky_correlation(&corr, 2).expect("should succeed");
        assert!(f.is_full_rank());
        assert_eq!(f.effective_rank(), 2);

        // Reconstruction: L * L^T should recover original matrix.
        let recon = mat_mul_lt(f.factor_matrix(), 2);
        for i in 0..4 {
            assert!(
                (recon[i] - corr[i]).abs() < 1e-12,
                "recon[{i}] = {}",
                recon[i]
            );
        }
    }

    #[test]
    fn pivoted_cholesky_apply_preserves_correlated_shocks_in_original_order() {
        let corr = vec![1.0, 0.5, 0.5, 1.0];
        let f = cholesky_correlation(&corr, 2).expect("should succeed");
        let z = vec![0.0, 1.0];
        let mut z_corr = vec![0.0; 2];

        f.apply(&z, &mut z_corr).expect("dimension match");

        assert!((z_corr[0] - 0.5).abs() < 1e-12, "z_corr[0] = {}", z_corr[0]);
        assert!((z_corr[1] - 1.0).abs() < 1e-12, "z_corr[1] = {}", z_corr[1]);
    }

    #[test]
    fn pivoted_cholesky_rejects_nan_diagonal() {
        // NaN must surface as a validation error, not Ok(NaN factor):
        // total_cmp orders NaN above all numbers so it would be picked as the
        // pivot and silently absorbed.
        let corr = vec![f64::NAN, 0.5, 0.5, 1.0];
        match cholesky_correlation(&corr, 2) {
            Err(CholeskyError::NonFiniteInput { row, col, .. }) => {
                assert_eq!((row, col), (0, 0));
            }
            other => panic!("expected NonFiniteInput, got {other:?}"),
        }
    }

    #[test]
    fn pivoted_cholesky_rejects_infinite_off_diagonal() {
        let corr = vec![1.0, f64::INFINITY, f64::INFINITY, 1.0];
        assert!(matches!(
            cholesky_correlation(&corr, 2),
            Err(CholeskyError::NonFiniteInput { .. })
        ));
    }

    #[test]
    fn pivoted_cholesky_apply_returns_error_on_dimension_mismatch() {
        let corr = vec![1.0, 0.5, 0.5, 1.0];
        let f = cholesky_correlation(&corr, 2).expect("should succeed");
        let mut z_corr = vec![0.0; 2];

        match f.apply(&[1.0], &mut z_corr) {
            Err(CholeskyError::DimensionMismatch { expected, actual }) => {
                assert_eq!(expected, 2);
                assert_eq!(actual, 1);
            }
            other => panic!("expected DimensionMismatch, got {other:?}"),
        }
    }

    #[test]
    fn pivoted_cholesky_identity_2x2() {
        let identity = vec![1.0, 0.0, 0.0, 1.0];
        let f = cholesky_correlation(&identity, 2).expect("should succeed");
        assert!(f.is_full_rank());
        let recon = mat_mul_lt(f.factor_matrix(), 2);
        for i in 0..4 {
            assert!((recon[i] - identity[i]).abs() < 1e-12);
        }
    }

    #[test]
    fn pivoted_cholesky_identity_3x3() {
        let n = 3usize;
        let mut m = vec![0.0; n * n];
        for i in 0..n {
            m[i * n + i] = 1.0;
        }
        let f = cholesky_correlation(&m, n).expect("identity must succeed");
        assert_eq!(f.effective_rank(), 3);
        let recon = mat_mul_lt(f.factor_matrix(), n);
        for i in 0..n * n {
            assert!((recon[i] - m[i]).abs() < 1e-12);
        }
    }

    #[test]
    fn pivoted_cholesky_near_singular_accepts() {
        // Old unpivoted path with absolute 1e-10 threshold would fail on this
        // because the second diagonal after elimination is ~(1 - 0.9999^2) ≈ 2e-4
        // but with rho=0.9999999 it approaches 1e-6, below the old threshold.
        // Pivoted path must accept and return effective_rank <= 2.
        let rho = 0.9999999_f64;
        let near_singular = vec![1.0, rho, rho, 1.0];
        let f = cholesky_correlation(&near_singular, 2).expect("pivoted must not reject near-PSD");
        // Effective rank may be 1 or 2 depending on exact arithmetic.
        assert!(f.effective_rank() <= 2);
        // L * L^T should approximate original (within numerical tolerance for
        // near-singular matrix).
        let recon = mat_mul_lt(f.factor_matrix(), 2);
        // Off-diagonals should be approximately rho.
        assert!(
            (recon[1] - rho).abs() < 1e-4,
            "off-diagonal reconstruction error: {}",
            recon[1]
        );
    }

    #[test]
    fn pivoted_cholesky_rank_deficient_3x3() {
        // Perfect linear dependence: third variable = first variable (rho_13 = 1).
        // This is a valid PSD matrix of rank 2.
        let corr = vec![1.0, 0.5, 1.0, 0.5, 1.0, 0.5, 1.0, 0.5, 1.0];
        let f = cholesky_correlation(&corr, 3).expect("rank-deficient PSD must succeed");
        // Matrix is rank 2 so effective_rank should be 2.
        assert_eq!(
            f.effective_rank(),
            2,
            "expected rank 2 for perfectly linearly dependent matrix"
        );
        // Reconstruction should match original.
        let recon = mat_mul_lt(f.factor_matrix(), 3);
        for (i, (&orig, &rec)) in corr.iter().zip(recon.iter()).enumerate() {
            assert!(
                (orig - rec).abs() < 1e-10,
                "reconstruction mismatch at [{},{}]: orig={orig}, recon={rec}",
                i / 3,
                i % 3,
            );
        }
    }

    #[test]
    fn pivoted_cholesky_rejects_indefinite() {
        // ρ = 1.01 makes the matrix indefinite.
        let indefinite = vec![1.0, 1.01, 1.01, 1.0];
        let result = cholesky_correlation(&indefinite, 2);
        assert!(result.is_err());
        match result.expect_err("should be indefinite") {
            CholeskyError::NotPositiveDefinite { .. } => {}
            e => panic!("expected NotPositiveDefinite, got {e:?}"),
        }
    }

    #[test]
    fn pivoted_cholesky_dimension_mismatch() {
        let small = vec![1.0, 0.5, 0.5, 1.0];
        match cholesky_correlation(&small, 3) {
            Err(CholeskyError::DimensionMismatch { expected, actual }) => {
                assert_eq!(expected, 3);
                assert_eq!(actual, 4);
            }
            other => panic!("expected DimensionMismatch, got {other:?}"),
        }
    }

    #[test]
    fn pivoted_cholesky_apply_shocks_original_order() {
        // For a 3×3 matrix with a known off-diagonal structure, verify that
        // shocks are generated in the original variable order regardless of which
        // pivot was chosen first.
        let corr = vec![1.0, 0.3, 0.8, 0.3, 1.0, 0.2, 0.8, 0.2, 1.0];
        let f = cholesky_correlation(&corr, 3).expect("must succeed");
        assert!(f.is_full_rank());

        // Reconstruction confirms ordering.
        let recon = mat_mul_lt(f.factor_matrix(), 3);
        for (i, (&orig, &rec)) in corr.iter().zip(recon.iter()).enumerate() {
            assert!(
                (orig - rec).abs() < 1e-12,
                "ordering violation at [{},{}]: orig={orig} recon={rec}",
                i / 3,
                i % 3,
            );
        }

        // apply() produces finite shocks of correct length.
        let z = vec![1.0, 0.0, -1.0];
        let mut out = vec![0.0; 3];
        f.apply(&z, &mut out).expect("dimension match");
        assert!(out.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn pivoted_cholesky_validate_accepts_near_singular() {
        // Demonstrate that validate_correlation_matrix now accepts matrices that
        // the old absolute-threshold path would have rejected as singular.
        // rho = 0.9999 gives second diagonal ≈ 2e-4, well above relative tolerance.
        let rho = 0.9999_f64;
        let corr = vec![1.0, rho, rho, 1.0];
        assert!(
            validate_correlation_matrix(&corr, 2).is_ok(),
            "near-singular but valid PSD matrix should pass validation"
        );
    }

    /// Regression: unpivoted solver path is unchanged.
    #[test]
    fn solver_path_unchanged() {
        // This exercises cholesky_decomposition + cholesky_solve as used by
        // solver_multi LM normal equations. Behavior must not change.
        // Solve [[4, 2], [2, 3]] x = [8, 7] → x = [1.4, 2.2] (approx).
        let a = vec![4.0_f64, 2.0, 2.0, 3.0];
        let l = cholesky_decomposition(&a, 2).expect("positive definite");
        let b = vec![8.0_f64, 7.0];
        let mut x = vec![0.0; 2];
        cholesky_solve(&l, &b, &mut x).expect("solve should succeed");
        // A x = b: check residual A*x - b ≈ 0
        let res0 = a[0] * x[0] + a[1] * x[1] - b[0];
        let res1 = a[2] * x[0] + a[3] * x[1] - b[1];
        assert!(res0.abs() < 1e-12, "residual[0] = {res0}");
        assert!(res1.abs() < 1e-12, "residual[1] = {res1}");
    }

    #[test]
    fn cholesky_solve_uses_relative_diagonal_threshold_for_scaled_systems() {
        let chol = vec![1.0e-12_f64];
        let b = vec![2.0e-24_f64];
        let mut x = vec![0.0_f64];

        cholesky_solve(&chol, &b, &mut x).expect("scaled one-dimensional solve should succeed");

        assert!((x[0] - 2.0).abs() < 1e-12, "x={}", x[0]);
    }
}
