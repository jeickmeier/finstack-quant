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
//! - **Ledoit-Wolf shrinkage**: Well-conditioned covariance estimation via [`ledoit_wolf_shrinkage`]
//!   (Ledoit & Wolf 2004, identity-scaled target, analytic optimal intensity)
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
//! use finstack_quant_core::math::linalg::cholesky_correlation;
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
//! - Higham, N. J. (2002). *Accuracy and Stability of Numerical Algorithms* (2nd ed.).
//!   SIAM. Algorithm 10.2 (Cholesky with complete pivoting). `docs/REFERENCES.md#higham-accuracy-and-stability`
//! - Golub, G. H., & Van Loan, C. F. (2013). *Matrix Computations* (4th ed.).
//!   Johns Hopkins University Press. Algorithm 4.2.5. `docs/REFERENCES.md#golub-van-loan-matrix-computations`
//!
//! - **Correlation Matrices**:
//! - Rebonato, R., & Jäckel, P. (2000). "The Most General Methodology to Create
//!   a Valid Correlation Matrix for Risk Management and Option Pricing Purposes."
//!   *Journal of Risk*, 2(2), 17-27. `docs/REFERENCES.md#rebonato-2004-volatility-correlation`
//!
//! - **Monte Carlo Applications**:
//! - Glasserman, P. (2003). *Monte Carlo Methods in Financial Engineering*.
//!   Springer. Section 2.4 (Generating multivariate samples). `docs/REFERENCES.md#glasserman-2004-monte-carlo`

use crate::{error, Result};
use thiserror::Error;

/// Relative singular threshold for Cholesky decomposition and substitution.
///
/// The generic unpivoted factorization compares factor pivots with this fraction
/// of the square root of the largest input diagonal magnitude. Substitution uses
/// the largest factor diagonal magnitude. Both criteria are invariant to a
/// uniform rescaling of the system. The correlation-specific path
/// (`cholesky_correlation`) uses [`PIVOT_TOLERANCE_RELATIVE`] instead.
pub const SINGULAR_THRESHOLD: f64 = 1e-10;

/// Default tolerance for diagonal elements in correlation matrices.
///
/// A correlation matrix has a diagonal of exactly 1 by construction, so any
/// deviation is either floating-point noise from forming `rho = cov / (sd_i *
/// sd_j)` — which is `O(eps * kappa)`, in practice below 1e-12 — or a genuine
/// defect in the input. A diagonal of, say, 1.000001 signals a mis-normalized
/// covariance matrix rather than rounding, so the tolerance is set at
/// noise scale to reject it instead of passing it through to Cholesky.
pub const DIAGONAL_TOLERANCE: f64 = 1e-10;

/// Default tolerance for symmetry checks in correlation matrices.
///
/// Symmetry, like the unit diagonal, holds exactly by construction; asymmetry
/// above noise scale indicates a transposition or indexing bug in the caller.
pub const SYMMETRY_TOLERANCE: f64 = 1e-10;

/// Slack allowed on the `[-1, 1]` bound for off-diagonal correlations.
///
/// Unlike the diagonal and symmetry checks, this bound needs *some* give: two
/// perfectly collinear series (a duplicated input, or genuinely identical
/// instruments) produce a computed correlation of `1 +/- a few ulp`, which is a
/// valid input that a strict bound would reject. The slack is therefore kept at
/// rounding scale — wide enough for last-bit error, far too narrow to admit a
/// value that is actually out of range.
pub const CORRELATION_BOUND_SLACK: f64 = 1e-12;

/// Relative pivot tolerance for [`cholesky_correlation`].
///
/// A pivot is considered numerically zero when it is below
/// `PIVOT_TOLERANCE_RELATIVE * max_diagonal`. This makes the threshold
/// scale-invariant and appropriate for both normalised correlation matrices
/// (diagonal ≈ 1) and general covariance matrices with large entries.
pub const PIVOT_TOLERANCE_RELATIVE: f64 = 1e-10;

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
    #[error("Invalid matrix size: expected {expected}×{expected}, got {actual} elements")]
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
}

/// Error type for Cholesky decomposition failures.
#[derive(Debug, Clone, PartialEq, Error, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
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
///   SIAM. Algorithm 10.2 (Cholesky with complete pivoting). `docs/REFERENCES.md#higham-accuracy-and-stability`
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
    /// Whether [`Self::factor`] is exactly lower triangular (all entries
    /// above the diagonal are `+0.0`).
    ///
    /// Certified by inspection of the stored matrix, independent of whether
    /// pivoting occurred. Enables a half-matrix hot path in [`Self::apply`].
    /// The dense and triangular loops agree bit-for-bit on finite shocks
    /// because the skipped terms are `0.0 * z` additions that cannot change
    /// the running sum.
    triangular: bool,
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
    ///
    /// # Arguments
    ///
    /// * `independent` - Independent standard-normal samples to transform in place.
    /// * `correlated` - Output buffer receiving correlated normal samples.
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
        if self.triangular {
            // Half-matrix path: entries above the diagonal are exactly zero,
            // so skipping them cannot change the running sum on finite input.
            for (i, out) in correlated.iter_mut().enumerate() {
                let row = i * n;
                let mut sum = 0.0;
                for (&f_j, &z_j) in self.factor[row..row + i + 1]
                    .iter()
                    .zip(independent[..=i].iter())
                {
                    sum += f_j * z_j;
                }
                *out = sum;
            }
        } else {
            for (i, out) in correlated.iter_mut().enumerate() {
                let mut sum = 0.0;
                for (j, &z_j) in independent.iter().enumerate() {
                    sum += self.factor[i * n + j] * z_j;
                }
                *out = sum;
            }
        }
        Ok(())
    }

    /// Construct a factor from an `n × n` row-major matrix.
    ///
    /// Exact lower-triangular matrices (every entry above the diagonal is
    /// `+0.0`) use the half-matrix [`Self::apply`] path. Any non-zero above
    /// the diagonal uses the dense path. Callers may pass a Cholesky factor
    /// or an arbitrary dense matrix.
    ///
    /// # Arguments
    ///
    /// * `factor` - Row-major `n × n` factor; length must be `n * n`.
    /// * `n` - Matrix dimension implied by `factor`.
    /// * `effective_rank` - Number of retained pivots; must satisfy
    ///   `effective_rank <= n`.
    #[must_use]
    pub fn from_parts(factor: Vec<f64>, n: usize, effective_rank: usize) -> Self {
        debug_assert_eq!(Some(factor.len()), n.checked_mul(n));
        debug_assert!(effective_rank <= n);
        let triangular = is_exactly_lower_triangular(&factor, n);
        Self {
            factor,
            n,
            effective_rank,
            triangular,
        }
    }
}

/// True when every strictly-upper-triangular entry is exactly `+0.0`.
fn is_exactly_lower_triangular(factor: &[f64], n: usize) -> bool {
    (0..n).all(|row| {
        factor[row * n + row + 1..row * n + n]
            .iter()
            .all(|&v| v == 0.0)
    })
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
/// `PIVOT_TOLERANCE_RELATIVE * max_diagonal` factorisation stops only when the
/// entire remaining block is numerically zero (semidefinite truncation). A
/// materially negative residual diagonal or a nonzero residual coupling makes
/// the matrix indefinite and returns an error. The tolerance scales with the
/// matrix, including covariance matrices whose variances are smaller than one.
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
/// use finstack_quant_core::math::linalg::cholesky_correlation;
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
    if n.checked_mul(n) != Some(matrix.len()) {
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
    let tol = PIVOT_TOLERANCE_RELATIVE * max_diag.abs();

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

        // A zero largest diagonal does not establish that the residual block
        // is PSD: e.g. [[0, 1], [1, 0]] has eigenvalues -1 and +1.
        if pivot_val <= tol {
            for row in step..n {
                let diagonal = a[row * n + row];
                if diagonal < -tol {
                    return Err(CholeskyError::NotPositiveDefinite {
                        diag: diagonal,
                        row: perm[row],
                    });
                }
                for col in step..row {
                    let coupling = a[row * n + col];
                    if coupling.abs() > tol {
                        // Rayleigh quotient for the signed two-variable
                        // direction opposing this residual coupling.
                        let negative_direction =
                            0.5 * diagonal + 0.5 * a[col * n + col] - coupling.abs();
                        return Err(CholeskyError::NotPositiveDefinite {
                            diag: negative_direction,
                            row: perm[row],
                        });
                    }
                }
            }
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

    // `from_parts` inspects the unpermuted factor. Unit-diagonal correlation
    // inputs typically swap, but a covariance already in pivot order can still
    // come out exactly lower triangular.
    Ok(CorrelationFactor::from_parts(l_orig, n, effective_rank))
}

// ─── Symmetric eigendecomposition (shared helper) ──────────────────────────────

/// Symmetric eigendecomposition of an `n × n` matrix stored row-major.
///
/// Returns `(eigenvalues, eigenvectors)` where `eigenvectors[i * n + k]` is
/// the `i`-th component of the `k`-th eigenvector — i.e. column-major in the
/// eigenvector index, matching the convention used by
/// [`apply_lower_triangular`] and by most linear-algebra references.
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
/// Returns [`CholeskyError::DimensionMismatch`] if `matrix.len() != n * n`, or
/// [`CholeskyError::NonFiniteInput`] if `matrix` contains NaN or infinity.
///
/// # Example
///
/// ```
/// use finstack_quant_core::math::linalg::symmetric_eigen;
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
/// Exposed as a shared helper so downstream crates (`finstack-quant-valuations`,
/// etc.) can delegate their eigendecomposition without pulling `nalgebra`
/// in as a direct dependency.
///
/// # Arguments
///
/// * `matrix` - Symmetric square matrix in row-major order with exactly `n * n`
///   entries.
/// * `n` - Matrix dimension used to interpret the flat row-major buffer.
pub fn symmetric_eigen(
    matrix: &[f64],
    n: usize,
) -> std::result::Result<(Vec<f64>, Vec<f64>), CholeskyError> {
    if n.checked_mul(n) != Some(matrix.len()) {
        return Err(CholeskyError::DimensionMismatch {
            expected: n,
            actual: matrix.len(),
        });
    }
    if n == 0 {
        return Ok((Vec::new(), Vec::new()));
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

/// Reshape a row-major flat buffer of length `n * n` into `n` nested rows.
///
/// # Arguments
///
/// * `flat` - Row-major square matrix storage; `flat.len()` must equal `n * n`
///   (extra trailing elements are ignored, a short buffer panics on slicing).
/// * `n` - Matrix dimension (rows == columns).
///
/// # Examples
///
/// ```
/// use finstack_quant_core::math::linalg::unflatten_square;
/// assert_eq!(unflatten_square(&[1.0, 2.0, 3.0, 4.0], 2), vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
/// ```
#[must_use]
pub fn unflatten_square(flat: &[f64], n: usize) -> Vec<Vec<f64>> {
    (0..n).map(|i| flat[i * n..(i + 1) * n].to_vec()).collect()
}

/// Transpose a flat row-major `rows × cols` matrix into `cols × rows`.
///
/// # Arguments
///
/// * `data` - Row-major buffer with exactly `rows * cols` entries.
/// * `rows` - Number of rows in `data`.
/// * `cols` - Number of columns in `data`.
///
/// # Panics
///
/// Panics if `data.len() != rows * cols`.
#[must_use]
pub fn transpose_row_major(data: &[f64], rows: usize, cols: usize) -> Vec<f64> {
    assert_eq!(
        data.len(),
        rows * cols,
        "transpose_row_major: shape mismatch"
    );
    let mut out = Vec::with_capacity(data.len());
    for c in 0..cols {
        for r in 0..rows {
            out.push(data[r * cols + c]);
        }
    }
    out
}

/// Cholesky decomposition of a correlation/covariance matrix.
///
/// Computes L such that Σ = L L^T, where Σ is the correlation matrix.
/// Normalizes by the largest input diagonal magnitude before factorization,
/// then restores the factor's units. The singularity threshold is relative
/// to the square root of that magnitude.
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
/// use finstack_quant_core::math::linalg::cholesky_decomposition;
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
    if n.checked_mul(n) != Some(matrix.len()) {
        return Err(CholeskyError::DimensionMismatch {
            expected: n,
            actual: matrix.len(),
        });
    }
    let mut l = vec![0.0; matrix.len()];
    cholesky_decomposition_into(matrix, n, &mut l)?;
    Ok(l)
}

/// Cholesky decomposition into a caller-provided buffer (avoids allocation).
///
/// The output buffer `l` must have length `n * n` and will be overwritten.
/// Singularity uses the same scale-relative criterion as
/// [`cholesky_decomposition`].
///
/// # Arguments
///
/// * `matrix` - Symmetric positive-definite input matrix in row-major order
///   with exactly `n * n` finite entries.
/// * `n` - Matrix dimension used to interpret both flat buffers.
/// * `l` - Mutable `n * n` output buffer overwritten with the lower-triangular
///   Cholesky factor in row-major order.
pub fn cholesky_decomposition_into(
    matrix: &[f64],
    n: usize,
    l: &mut [f64],
) -> std::result::Result<(), CholeskyError> {
    if n.checked_mul(n) != Some(matrix.len()) || matrix.len() != l.len() {
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

    let matrix_scale = (0..n)
        .map(|i| matrix[i * n + i].abs())
        .fold(0.0_f64, f64::max);
    let factor_scale = matrix_scale.sqrt();

    l.fill(0.0);
    if n > 0 && matrix_scale == 0.0 {
        return Err(CholeskyError::Singular {
            value: 0.0,
            row: 0,
            col: 0,
            threshold: 0.0,
        });
    }

    for i in 0..n {
        for j in 0..=i {
            let mut sum = 0.0;
            for k in 0..j {
                sum += l[i * n + k] * l[j * n + k];
            }

            if i == j {
                let diag = matrix[i * n + i] / matrix_scale - sum;
                if diag < 0.0 {
                    return Err(CholeskyError::NotPositiveDefinite {
                        diag: diag * matrix_scale,
                        row: i,
                    });
                }
                l[i * n + j] = diag.sqrt();
                if l[i * n + j] < SINGULAR_THRESHOLD {
                    return Err(CholeskyError::Singular {
                        value: l[i * n + j] * factor_scale,
                        row: i,
                        col: j,
                        threshold: SINGULAR_THRESHOLD * factor_scale,
                    });
                }
            } else {
                // This diagonal was checked when its row was factorized.
                l[i * n + j] = (matrix[i * n + j] / matrix_scale - sum) / l[j * n + j];
            }
        }
    }

    for value in l {
        *value *= factor_scale;
    }

    Ok(())
}
/// Apply a lower-triangular factor `L` to a vector `z`, returning `L z`.
///
/// This is the Cholesky *apply* step used to turn a vector of i.i.d. standard
/// normals into correlated normals: if `Sigma = L L^T` and `z ~ N(0, I)`, then
/// `L z ~ N(0, Sigma)`. Use [`CorrelationFactor::apply`] on hot Monte Carlo
/// paths where the allocation matters.
///
/// `l` is the same **row-major, `n * n`, lower-triangular** flat layout that
/// [`cholesky_decomposition`] returns — entry `(i, j)` lives at `l[i * n + j]`.
/// Only the lower triangle (`j <= i`) is read; the upper triangle is assumed
/// zero and is ignored rather than validated.
///
/// # Errors
///
/// Returns [`crate::error::InputError::DimensionMismatch`] if `l.len() != n * n`
/// or `z.len() != n`.
///
/// # Arguments
///
/// * `l` - Lower-triangular factor in row-major order with exactly `n * n`
///   entries, typically the output of [`cholesky_decomposition`].
/// * `n` - Dimension used to interpret both flat buffers (number of variables).
/// * `z` - Vector of length `n` to transform, typically i.i.d. standard normal
///   draws.
///
/// # Returns
///
/// A newly allocated vector of length `n` holding `L z`, in the same variable
/// order as `z`.
///
/// # Examples
///
/// ```
/// use finstack_quant_core::math::linalg::{apply_lower_triangular, cholesky_decomposition};
///
/// // Correlation matrix [[1.0, 0.5], [0.5, 1.0]].
/// let corr = vec![1.0, 0.5, 0.5, 1.0];
/// let l = cholesky_decomposition(&corr, 2).expect("Cholesky decomposition should succeed");
///
/// let correlated = apply_lower_triangular(&l, 2, &[1.0, 0.0]).expect("dimensions match");
/// assert!((correlated[0] - 1.0).abs() < 1e-12);
/// assert!((correlated[1] - 0.5).abs() < 1e-12);
/// ```
///
/// # Complexity
///
/// Time: O(n²), Space: O(n).
///
/// # References
///
/// - Glasserman, P. (2003). *Monte Carlo Methods in Financial Engineering*.
///   Springer. Section 2.3.3, pp. 71-73 (generating correlated normals from a
///   Cholesky factor). `docs/REFERENCES.md#glasserman-2004-monte-carlo`
/// - Golub, G. H., & Van Loan, C. F. (2013). *Matrix Computations* (4th ed.).
///   Johns Hopkins University Press. Section 3.1 (triangular matrix-vector
///   products). `docs/REFERENCES.md#golub-van-loan-matrix-computations`
///
/// # See Also
///
/// - [`CorrelationFactor::apply`] for the pivoted, rank-aware, allocation-free factor.
pub fn apply_lower_triangular(l: &[f64], n: usize, z: &[f64]) -> Result<Vec<f64>> {
    if n.checked_mul(n) != Some(l.len()) || z.len() != n {
        return Err(error::InputError::DimensionMismatch.into());
    }

    let mut out = vec![0.0; n];
    for (i, slot) in out.iter_mut().enumerate() {
        let mut sum = 0.0;
        for (j, &z_j) in z.iter().enumerate().take(i + 1) {
            sum += l[i * n + j] * z_j;
        }
        *slot = sum;
    }
    Ok(out)
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
///
/// The caller supplies the factor rather than the original matrix, so this
/// function does not re-check triangularity, symmetry, or factorization
/// accuracy. `x` is used as temporary storage for the forward-substitution
/// solution before it is overwritten with the final solution.
///
/// # Errors
///
/// Returns an error if `chol.len() != b.len()²`, `x.len() != b.len()`, or a
/// diagonal factor is too close to zero relative to the largest diagonal
/// magnitude. Rejects non-finite entries in the consumed lower triangle and
/// right-hand side, or a non-finite substitution result. The unused upper
/// triangle is ignored. On a numerical error, `x` may contain a partial result
/// and must not be used as a solution.
pub fn cholesky_solve(chol: &[f64], b: &[f64], x: &mut [f64]) -> Result<()> {
    let n = b.len();
    if n.checked_mul(n) != Some(chol.len()) || x.len() != n {
        return Err(crate::error::InputError::DimensionMismatch.into());
    }
    for &value in b {
        if !value.is_finite() {
            return Err(crate::InputError::NonFiniteValue {
                kind: crate::NonFiniteKind::classify(value),
            }
            .into());
        }
    }
    for i in 0..n {
        for &value in &chol[i * n..=i * n + i] {
            if !value.is_finite() {
                return Err(crate::InputError::NonFiniteValue {
                    kind: crate::NonFiniteKind::classify(value),
                }
                .into());
            }
        }
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
        if !x[i].is_finite() {
            return Err(crate::InputError::NonFiniteValue {
                kind: crate::NonFiniteKind::classify(x[i]),
            }
            .into());
        }
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
        if !x[i].is_finite() {
            return Err(crate::InputError::NonFiniteValue {
                kind: crate::NonFiniteKind::classify(x[i]),
            }
            .into());
        }
    }

    Ok(())
}
/// Validate that a matrix is a valid correlation matrix.
///
/// Checks:
/// 1. Diagonal elements are 1.0
/// 2. Off-diagonal elements are in [-1, 1]
/// 3. Matrix is symmetric
/// 4. Matrix is positive semi-definite (via Cholesky)
///
/// `matrix` is row-major and must contain exactly `n * n` elements. The
/// diagonal and symmetry checks use this module's numerical tolerances, rather
/// than requiring exact binary equality. Positive semidefiniteness is checked
/// through the correlation-specific pivoted Cholesky path, allowing nearly
/// rank-deficient but otherwise valid correlation matrices.
///
/// # Errors
///
/// Returns an error if the matrix shape is wrong; a diagonal differs from one
/// beyond tolerance; an off-diagonal entry is outside `[-1, 1]`; the matrix is
/// not symmetric within tolerance; or the Cholesky check finds it not positive
/// semidefinite.
///
/// # Example
///
/// ```
/// use finstack_quant_core::math::linalg::check_correlation_matrix;
///
/// let valid = vec![1.0, 0.5, 0.5, 1.0];
/// assert!(check_correlation_matrix(&valid, 2).is_ok());
///
/// let invalid = vec![1.0, 1.5, 1.5, 1.0]; // Correlation > 1
/// assert!(check_correlation_matrix(&invalid, 2).is_err());
/// ```
///
/// # Arguments
///
/// * `matrix` - Candidate correlation matrix in row-major order with exactly
///   `n * n` finite entries.
/// * `n` - Matrix dimension used to interpret the flat row-major buffer.
pub fn check_correlation_matrix(matrix: &[f64], n: usize) -> Result<()> {
    validate_correlation_matrix(matrix, n).map_err(|error| match error {
        CorrelationError::InvalidSize { .. } => error::InputError::DimensionMismatch.into(),
        _ => error::InputError::Invalid.into(),
    })
}

/// Validate a flattened row-major correlation matrix with located diagnostics.
///
/// Checks matrix size, unit diagonal, coefficient bounds, symmetry, and
/// positive semidefiniteness in that order. Numerical thresholds and Cholesky
/// semantics are identical to [`validate_correlation_matrix`].
///
/// # Arguments
///
/// * `matrix` - Correlation coefficients in row-major `n × n` order; each
///   entry at row `i`, column `j` is stored at `i * n + j`.
/// * `n` - Number of variables represented by each matrix dimension; `0`
///   accepts an empty matrix.
///
/// # Errors
///
/// Returns the first [`CorrelationError`] detected.
pub fn validate_correlation_matrix(
    matrix: &[f64],
    n: usize,
) -> std::result::Result<(), CorrelationError> {
    if n.checked_mul(n) != Some(matrix.len()) {
        return Err(CorrelationError::InvalidSize {
            expected: n,
            actual: matrix.len(),
        });
    }
    if n == 0 {
        return Ok(());
    }

    for i in 0..n {
        let value = matrix[i * n + i];
        if !value.is_finite() {
            return Err(CorrelationError::OutOfBounds { i, j: i, value });
        }
        if (value - 1.0).abs() > DIAGONAL_TOLERANCE {
            return Err(CorrelationError::DiagonalNotOne { index: i, value });
        }
    }

    for i in 0..n {
        for j in 0..n {
            if i == j {
                continue;
            }

            let value = matrix[i * n + j];
            if !(-1.0 - CORRELATION_BOUND_SLACK..=1.0 + CORRELATION_BOUND_SLACK).contains(&value) {
                return Err(CorrelationError::OutOfBounds { i, j, value });
            }
            if i < j {
                let diff = (matrix[i * n + j] - matrix[j * n + i]).abs();
                if diff > SYMMETRY_TOLERANCE {
                    return Err(CorrelationError::NotSymmetric { i, j, diff });
                }
            }
        }
    }

    if let Err(error) = cholesky_correlation(matrix, n) {
        let row = match error {
            CholeskyError::NotPositiveDefinite { row, .. } => row,
            _ => 0,
        };
        return Err(CorrelationError::NotPositiveSemiDefinite { row });
    }

    Ok(())
}

/// Result of Ledoit-Wolf covariance shrinkage.
///
/// See [`ledoit_wolf_shrinkage`].
#[derive(Debug, Clone, PartialEq)]
pub struct LedoitWolfResult {
    /// Row-major `n × n` shrunk covariance matrix `Σ* = δ*·μ·I + (1 − δ*)·S`.
    pub covariance: Vec<f64>,
    /// Optimal shrinkage intensity `δ* ∈ [0, 1]`.
    pub shrinkage: f64,
}

/// Ledoit-Wolf (2004) shrinkage of a sample covariance matrix toward a scaled
/// identity target, with the analytic optimal shrinkage intensity.
///
/// Given `t` observations of `n` variables (row-major `observations`, one
/// observation per row), columns are demeaned, the sample covariance
/// `S = XᵀX/T` is formed, and the estimator
///
/// ```text
/// Σ* = δ*·μ·I + (1 − δ*)·S
///
/// μ   = tr(S)/n                       (⟨S, I⟩ under ⟨A,B⟩ = tr(ABᵀ)/n)
/// d²  = ‖S − μI‖²                     (‖A‖² = tr(AAᵀ)/n)
/// b̄²  = (1/T²)·Σ_t ‖x_t x_tᵀ − S‖²
/// b²  = min(b̄², d²)
/// δ*  = b²/d²                          (δ* = 0 when d² = 0, i.e. S = μI)
/// ```
///
/// is returned. `Σ*` is a convex combination of the PSD `S` and the PSD `μI`,
/// hence always positive semi-definite and well-conditioned for `δ* > 0`.
/// Centered observations are normalized by a common magnitude before forming
/// second and fourth moments, then the covariance is restored to input units.
/// This preserves shrinkage under uniform rescaling without overflowing or
/// underflowing the fourth moments.
/// The computation is a deterministic serial fold: identical inputs produce
/// bit-identical outputs. Complexity: O(t·n²) time, O(t·n + n²) space.
///
/// # Arguments
///
/// * `observations` - Row-major `t × n` observation matrix (row = one date)
/// * `t` - Number of observations (rows); must be ≥ 2
/// * `n` - Number of variables (columns); must be ≥ 1
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] when `t < 2`, `n == 0`, any entry
/// is non-finite, or the centered observations or resulting covariance exceed
/// the finite `f64` range. Returns [`crate::InputError::DimensionMismatch`] when
/// `observations.len() != t * n` or the output dimensions overflow.
///
/// # Examples
///
/// ```
/// use finstack_quant_core::math::linalg::ledoit_wolf_shrinkage;
///
/// // 4 observations of 2 zero-mean variables (row-major).
/// let x = [1.0, 1.0, -1.0, -1.0, 2.0, -2.0, -2.0, 2.0];
/// let result = ledoit_wolf_shrinkage(&x, 4, 2).unwrap();
/// assert!((result.shrinkage - 17.0 / 18.0).abs() < 1e-14);
/// assert!((result.covariance[1] - (-1.0 / 12.0)).abs() < 1e-13);
/// ```
///
/// # References
///
/// - Ledoit, O., & Wolf, M. (2004). "A well-conditioned estimator for
///   large-dimensional covariance matrices." *Journal of Multivariate
///   Analysis*, 88(2), 365–411. Lemmas 3.2–3.4. `docs/REFERENCES.md#ledoitwolf2004`
pub fn ledoit_wolf_shrinkage(
    observations: &[f64],
    t: usize,
    n: usize,
) -> crate::Result<LedoitWolfResult> {
    if n == 0 {
        return Err(crate::Error::Validation(
            "ledoit_wolf_shrinkage: n must be >= 1".to_owned(),
        ));
    }
    if t < 2 {
        return Err(crate::Error::Validation(format!(
            "ledoit_wolf_shrinkage: need at least 2 observations, got {t}"
        )));
    }
    if t.checked_mul(n) != Some(observations.len()) || n.checked_mul(n).is_none() {
        return Err(crate::InputError::DimensionMismatch.into());
    }
    if let Some(bad) = observations.iter().find(|v| !v.is_finite()) {
        return Err(crate::Error::Validation(format!(
            "ledoit_wolf_shrinkage: non-finite observation {bad}"
        )));
    }

    let tf = t as f64;
    let nf = n as f64;

    // Compute each mean in that column's units to avoid overflowing a sum of
    // finite observations. Center before choosing the common moment scale so
    // a large constant column cannot erase variation in the other columns.
    let mut x = vec![0.0_f64; t * n];
    let mut scale = 0.0_f64;
    for col in 0..n {
        let column_scale = (0..t)
            .map(|row| observations[row * n + col].abs())
            .fold(0.0_f64, f64::max);
        let mean = if column_scale > 0.0 {
            crate::math::summation::neumaier_sum(
                (0..t).map(|row| observations[row * n + col] / column_scale),
            ) / tf
                * column_scale
        } else {
            0.0
        };
        for row in 0..t {
            let centered = observations[row * n + col] - mean;
            if !centered.is_finite() {
                return Err(crate::Error::Validation(
                    "ledoit_wolf_shrinkage: centered observations exceed finite f64 range"
                        .to_owned(),
                ));
            }
            x[row * n + col] = centered;
            scale = scale.max(centered.abs());
        }
    }
    if scale > 0.0 {
        for value in &mut x {
            *value /= scale;
        }
    }

    // S = XᵀX / T (row-major, symmetric by construction).
    let mut s = vec![0.0_f64; n * n];
    for i in 0..n {
        for j in i..n {
            let mut acc = 0.0_f64;
            for row in 0..t {
                acc += x[row * n + i] * x[row * n + j];
            }
            let value = acc / tf;
            s[i * n + j] = value;
            s[j * n + i] = value;
        }
    }

    // μ = tr(S)/n.
    let mu = (0..n).map(|i| s[i * n + i]).sum::<f64>() / nf;

    // d² = ‖S − μI‖² with ‖A‖² = Σ a_ij² / n.
    let mut d2 = 0.0_f64;
    for i in 0..n {
        for j in 0..n {
            let target = if i == j { mu } else { 0.0 };
            let dev = s[i * n + j] - target;
            d2 += dev * dev;
        }
    }
    d2 /= nf;

    // b̄² = (1/T²) Σ_t ‖x_t x_tᵀ − S‖².
    let mut b_bar2 = 0.0_f64;
    for row in 0..t {
        let xr = &x[row * n..(row + 1) * n];
        let mut norm = 0.0_f64;
        for i in 0..n {
            for j in 0..n {
                let dev = xr[i] * xr[j] - s[i * n + j];
                norm += dev * dev;
            }
        }
        b_bar2 += norm / nf;
    }
    b_bar2 /= tf * tf;

    let b2 = b_bar2.min(d2);
    let shrinkage = if d2 > 0.0 { b2 / d2 } else { 0.0 };

    let mut covariance = vec![0.0_f64; n * n];
    for i in 0..n {
        for j in 0..n {
            let target = if i == j { mu } else { 0.0 };
            let normalized = shrinkage * target + (1.0 - shrinkage) * s[i * n + j];
            // Restore units in two products: scale² can overflow even when
            // the final covariance entry is representable.
            let value = (normalized * scale) * scale;
            if !value.is_finite() {
                return Err(crate::Error::Validation(
                    "ledoit_wolf_shrinkage: covariance exceeds finite f64 range".to_owned(),
                ));
            }
            covariance[i * n + j] = value;
        }
    }

    Ok(LedoitWolfResult {
        covariance,
        shrinkage,
    })
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

    /// Reference dense multiply — the pre-optimization `apply` loop.
    fn dense_apply(factor: &CorrelationFactor, independent: &[f64], correlated: &mut [f64]) {
        let n = factor.n();
        for (i, out) in correlated.iter_mut().enumerate() {
            let mut sum = 0.0;
            for (j, &z_j) in independent.iter().enumerate() {
                sum += factor.factor_matrix()[i * n + j] * z_j;
            }
            *out = sum;
        }
    }

    /// Strictly lower-triangular factor with decreasing diagonals.
    fn lower_triangular_factor(n: usize) -> Vec<f64> {
        let mut factor = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..=i {
                factor[i * n + j] = if i == j {
                    (n - i) as f64
                } else {
                    0.1 * ((i - j) as f64)
                };
            }
        }
        factor
    }

    #[test]
    fn triangular_flag_reflects_pivot_reality() {
        // Unit-diagonal correlation inputs tie on every initial pivot, so
        // complete pivoting swaps and the unpermuted factor is generally NOT
        // triangular — the flag must stay off there.
        let n = 8;
        let rho = 0.5_f64;
        let corr: Vec<f64> = (0..n * n)
            .map(|k| {
                let (i, j) = (k / n, k % n);
                if i == j {
                    1.0
                } else {
                    rho
                }
            })
            .collect();
        let factor = cholesky_correlation(&corr, n).expect("valid");
        assert!(!factor.triangular);

        // A covariance-style input whose diagonals are already in pivot order
        // never swaps; its factor comes out exactly lower triangular and the
        // fast path engages.
        let cov = vec![100.0, 1.0, 1.0, 1.0];
        let factor = cholesky_correlation(&cov, 2).expect("valid");
        assert!(factor.triangular);
        let z = [0.3, -0.7];
        let mut fast = vec![0.0; 2];
        factor.apply(&z, &mut fast).expect("matching dimensions");
        let mut reference = vec![0.0; 2];
        dense_apply(&factor, &z, &mut reference);
        assert_eq!(fast, reference);
    }

    #[test]
    fn triangular_fast_path_matches_dense_multiply_bit_for_bit() {
        let n = 8usize;
        let l = lower_triangular_factor(n);
        let factor = CorrelationFactor::from_parts(l, n, n);
        assert!(
            factor.triangular,
            "n=8 lower-triangular factor must enter the half-matrix path"
        );

        let z: Vec<f64> = (0..n).map(|i| ((i % 13) as f64 * 0.07) - 0.4).collect();
        let mut fast = vec![0.0; n];
        factor.apply(&z, &mut fast).expect("matching dimensions");
        let mut reference = vec![0.0; n];
        dense_apply(&factor, &z, &mut reference);
        assert_eq!(fast, reference, "apply must be bit-identical at n={n}");

        // The same factor reconstructed by pivoted Cholesky of L L^T stays
        // triangular because the diagonals are already in pivot order.
        let cov = mat_mul_lt(factor.factor_matrix(), n);
        let reconstructed = cholesky_correlation(&cov, n).expect("SPD covariance");
        assert!(reconstructed.triangular);
        let mut via_chol = vec![0.0; n];
        reconstructed
            .apply(&z, &mut via_chol)
            .expect("matching dimensions");
        dense_apply(&reconstructed, &z, &mut reference);
        assert_eq!(via_chol, reference);
    }

    #[test]
    fn from_parts_factor_uses_the_dense_path() {
        // Callers may hand a full (non-triangular) matrix to `from_parts`;
        // it must take the dense path.
        let corr = vec![1.0, 0.5, 0.5, 1.0];
        let factor = CorrelationFactor::from_parts(corr.clone(), 2, 2);
        let z = [0.3, -0.7];

        let mut via_apply = vec![0.0; 2];
        factor
            .apply(&z, &mut via_apply)
            .expect("matching dimensions");
        assert!(!factor.triangular);
        // Dense multiply of the raw matrix, computed inline.
        assert_eq!(via_apply[0], corr[0] * z[0] + corr[1] * z[1]);
        assert_eq!(via_apply[1], corr[2] * z[0] + corr[3] * z[1]);
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
    fn symmetric_eigen_handles_empty_matrix() {
        let (values, vectors) = symmetric_eigen(&[], 0).expect("empty matrix should succeed");
        assert!(values.is_empty());
        assert!(vectors.is_empty());
    }

    #[test]
    fn symmetric_eigen_rejects_dimension_mismatch() {
        let err = symmetric_eigen(&[1.0, 0.0, 0.0], 2).expect_err("wrong length should fail");
        assert!(matches!(
            err,
            CholeskyError::DimensionMismatch {
                expected: 2,
                actual: 3
            }
        ));
    }

    #[test]
    fn symmetric_eigen_rejects_non_finite_input() {
        let err = symmetric_eigen(&[1.0, f64::NAN, f64::NAN, 1.0], 2)
            .expect_err("non-finite input should fail");
        assert!(matches!(
            err,
            CholeskyError::NonFiniteInput {
                value,
                row: 0,
                col: 1,
            } if value.is_nan()
        ));
    }

    #[test]
    fn symmetric_eigen_diagonal_matrix_returns_diagonal_values() {
        let matrix = vec![4.0, 0.0, 0.0, 0.0, 9.0, 0.0, 0.0, 0.0, 16.0];
        let (mut values, vectors) = symmetric_eigen(&matrix, 3).expect("diagonal matrix");
        values.sort_by(|a, b| a.partial_cmp(b).unwrap());

        assert_eq!(values, vec![4.0, 9.0, 16.0]);
        assert_eq!(vectors.len(), 9);
    }

    #[test]
    fn symmetric_eigen_vectors_satisfy_av_equals_lambda_v() {
        let matrix = vec![2.0, 1.0, 1.0, 2.0];
        let (values, vectors) = symmetric_eigen(&matrix, 2).expect("symmetric matrix");

        for (k, &lambda) in values.iter().enumerate() {
            let v = [vectors[k], vectors[2 + k]];
            let av = [
                matrix[0] * v[0] + matrix[1] * v[1],
                matrix[2] * v[0] + matrix[3] * v[1],
            ];

            assert!((av[0] - lambda * v[0]).abs() < 1e-10);
            assert!((av[1] - lambda * v[1]).abs() < 1e-10);
        }
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
        // Asymmetry below SYMMETRY_TOLERANCE (1e-10) is accepted: at that
        // scale it is rounding from forming the matrix, not a real defect.
        let almost_sym = vec![1.0, 0.5, 0.5 + 5.0e-11, 1.0];
        assert!(
            validate_correlation_matrix(&almost_sym, 2).is_ok(),
            "Near-symmetric matrix below tolerance should be accepted"
        );

        // Asymmetry just above SYMMETRY_TOLERANCE is rejected.
        let barely_asym = vec![1.0, 0.5, 0.5 + 2.0e-10, 1.0];
        assert!(
            validate_correlation_matrix(&barely_asym, 2).is_err(),
            "Asymmetry above tolerance should be rejected"
        );

        // Asymmetry that the previous 1e-6 tolerance admitted is now caught:
        // a 1e-7 discrepancy is orders of magnitude above rounding and
        // indicates a transposition or indexing bug upstream.
        let formerly_accepted = vec![1.0, 0.5, 0.5 + 1.0e-7, 1.0];
        assert!(
            validate_correlation_matrix(&formerly_accepted, 2).is_err(),
            "Asymmetry at 1e-7 should be rejected under the tightened tolerance"
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
    fn pivoted_cholesky_rejects_coupled_zero_residual_block() {
        let indefinite = [1.0, 1.0, 1.0, 1.0, 1.0, -1.0, 1.0, -1.0, 1.0];
        assert!(matches!(
            cholesky_correlation(&indefinite, 3),
            Err(CholeskyError::NotPositiveDefinite { .. })
        ));
        assert!(matches!(
            validate_correlation_matrix(&indefinite, 3),
            Err(CorrelationError::NotPositiveSemiDefinite { .. })
        ));

        let rank_one = [1.0; 9];
        let factor = cholesky_correlation(&rank_one, 3).expect("uncoupled zero residual is PSD");
        assert_eq!(factor.effective_rank(), 1);
        assert_eq!(mat_mul_lt(factor.factor_matrix(), 3), rank_one);
    }

    #[test]
    fn pivoted_cholesky_checks_all_residual_diagonals() {
        let indefinite = [0.0, 0.0, 0.0, -1.0];
        assert!(matches!(
            cholesky_correlation(&indefinite, 2),
            Err(CholeskyError::NotPositiveDefinite { row: 1, .. })
        ));
    }

    #[test]
    fn pivoted_cholesky_is_invariant_to_covariance_scale() {
        let covariance = [4.0, 1.0, 1.0, 2.0];
        for scale in [1e-16, 1.0, 1e16] {
            let scaled = covariance.map(|value| value * scale);
            let factor = cholesky_correlation(&scaled, 2).expect("scaled SPD covariance");
            assert_eq!(factor.effective_rank(), 2);
            for (actual, expected) in mat_mul_lt(factor.factor_matrix(), 2).iter().zip(covariance) {
                assert!((actual / scale - expected).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn linalg_rejects_overflowing_dimensions_without_panicking() {
        let n = usize::MAX;
        assert!(cholesky_correlation(&[], n).is_err());
        assert!(cholesky_decomposition(&[], n).is_err());
        assert!(cholesky_decomposition_into(&[], n, &mut []).is_err());
        assert!(symmetric_eigen(&[], n).is_err());
        let error = validate_correlation_matrix(&[], n).expect_err("overflowing square");
        assert!(error.to_string().contains("Invalid matrix size"));
        assert!(ledoit_wolf_shrinkage(&[], 2, n).is_err());
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
    fn cholesky_decomposition_into_matches_allocating_variant() {
        let a = vec![4.0_f64, 2.0, 2.0, 3.0];
        let expected = cholesky_decomposition(&a, 2).expect("positive definite");
        let mut out = vec![f64::NAN; 4];

        cholesky_decomposition_into(&a, 2, &mut out).expect("positive definite");

        assert_eq!(out, expected);
    }

    #[test]
    fn cholesky_decomposition_into_rejects_bad_output_len() {
        let a = vec![4.0_f64, 2.0, 2.0, 3.0];
        let mut out = vec![0.0; 3];

        match cholesky_decomposition_into(&a, 2, &mut out) {
            Err(CholeskyError::DimensionMismatch { expected, actual }) => {
                assert_eq!(expected, 2);
                assert_eq!(actual, 4);
            }
            other => panic!("expected DimensionMismatch, got {other:?}"),
        }
    }

    #[test]
    fn cholesky_decomposition_into_rejects_non_positive_definite() {
        let indefinite = vec![1.0_f64, 2.0, 2.0, 1.0];
        let mut out = vec![0.0; 4];

        assert!(matches!(
            cholesky_decomposition_into(&indefinite, 2, &mut out),
            Err(CholeskyError::NotPositiveDefinite { .. })
        ));
    }

    #[test]
    fn generic_cholesky_variants_reject_non_finite_entries() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(matches!(
                cholesky_decomposition(&[bad], 1),
                Err(CholeskyError::NonFiniteInput { .. })
            ));
            let mut out = [0.0];
            assert!(matches!(
                cholesky_decomposition_into(&[bad], 1, &mut out),
                Err(CholeskyError::NonFiniteInput { .. })
            ));
        }
    }

    #[test]
    fn cholesky_solve_rejects_dimension_mismatch_and_singular_diagonal() {
        let mut x = vec![0.0_f64; 2];
        assert!(cholesky_solve(&[1.0, 0.0, 0.0, 1.0], &[1.0], &mut x).is_err());

        let singular_chol = vec![1.0_f64, 0.0, 0.0, 0.0];
        let mut x = vec![0.0_f64; 2];
        assert!(cholesky_solve(&singular_chol, &[1.0, 1.0], &mut x).is_err());
    }

    #[test]
    fn cholesky_solve_uses_relative_diagonal_threshold_for_scaled_systems() {
        let chol = vec![1.0e-12_f64];
        let b = vec![2.0e-24_f64];
        let mut x = vec![0.0_f64];

        cholesky_solve(&chol, &b, &mut x).expect("scaled one-dimensional solve should succeed");

        assert!((x[0] - 2.0).abs() < 1e-12, "x={}", x[0]);
    }

    #[test]
    fn cholesky_factorization_and_solve_are_invariant_to_system_scale() {
        // Independent exact solution: [[4, 1], [1, 2]] * [2, -1] = [7, 0].
        let matrix = [4.0, 1.0, 1.0, 2.0];
        for scale in [1e-300, 1e-24, 1.0, 1e24, 1e300] {
            let scaled = matrix.map(|value| scale * value);
            let factor = cholesky_decomposition(&scaled, 2).expect("scaled SPD matrix");
            let mut buffer = [f64::NAN; 4];
            cholesky_decomposition_into(&scaled, 2, &mut buffer)
                .expect("scaled SPD matrix into supplied buffer");
            assert_eq!(factor, buffer);

            for (actual, expected) in mat_mul_lt(&factor, 2).iter().zip(matrix) {
                assert!((actual / scale - expected).abs() < 1e-12);
            }
            let mut solution = [0.0; 2];
            cholesky_solve(&factor, &[7.0 * scale, 0.0], &mut solution)
                .expect("factored scaled system");
            assert!((solution[0] - 2.0).abs() < 1e-12, "scale={scale}");
            assert!((solution[1] + 1.0).abs() < 1e-12, "scale={scale}");
        }
    }

    #[test]
    fn cholesky_factorization_rejects_singular_systems_at_every_scale() {
        for scale in [1e-300, 1.0, 1e300] {
            for matrix in [
                [0.0; 4],
                [scale, 0.0, 0.0, 0.0],
                [scale, scale, scale, scale],
                [scale, 0.0, 0.0, scale * 1e-22],
            ] {
                assert!(matches!(
                    cholesky_decomposition(&matrix, 2),
                    Err(CholeskyError::Singular { .. })
                ));
                assert!(matches!(
                    cholesky_decomposition_into(&matrix, 2, &mut [0.0; 4]),
                    Err(CholeskyError::Singular { .. })
                ));
            }
        }
    }

    #[test]
    fn cholesky_solve_rejects_nonfinite_inputs_before_writing_output() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for factor in [[bad, 0.0, 0.0, 1.0], [1.0, 0.0, bad, 1.0]] {
                let mut x = [7.0, 8.0];
                assert!(cholesky_solve(&factor, &[1.0, 1.0], &mut x).is_err());
                assert_eq!(x, [7.0, 8.0]);
            }
            let mut x = [7.0];
            assert!(cholesky_solve(&[1.0], &[bad], &mut x).is_err());
            assert_eq!(x, [7.0]);
        }
    }

    #[test]
    fn cholesky_solve_ignores_unused_upper_triangle() {
        let mut x = [0.0, 0.0];
        cholesky_solve(&[1.0, f64::NAN, 0.0, 2.0], &[2.0, 12.0], &mut x)
            .expect("only the lower triangle is consumed");
        assert_eq!(x, [2.0, 3.0]);
    }

    #[test]
    fn cholesky_solve_rejects_overflow_from_finite_inputs() {
        let mut x = [0.0];
        assert!(cholesky_solve(&[1.0e-160], &[1.0], &mut x).is_err());
    }
    #[test]
    fn apply_lower_triangular_reproduces_target_covariance() {
        // L L^T must equal the input matrix, i.e. applying L to each unit
        // vector and taking inner products recovers the correlations.
        let corr = vec![1.0, 0.5, 0.5, 1.0];
        let l = cholesky_decomposition(&corr, 2)
            .expect("Cholesky decomposition should succeed in test");

        let e0 = apply_lower_triangular(&l, 2, &[1.0, 0.0]).expect("dimensions match in test");
        let e1 = apply_lower_triangular(&l, 2, &[0.0, 1.0]).expect("dimensions match in test");

        // Cov(x_i, x_j) = sum_k L[i,k] L[j,k] = row_i . row_j.
        let cov_01 = l[0] * l[2] + l[1] * l[3];
        assert!((cov_01 - 0.5).abs() < 1e-12);
        // Upper triangle is ignored: the first output depends only on z[0].
        assert!((e0[0] - 1.0).abs() < 1e-12);
        assert!((e1[0] - 0.0).abs() < 1e-12);
    }

    #[test]
    fn apply_lower_triangular_rejects_dimension_mismatch() {
        let l = vec![1.0, 0.0, 0.5, 0.866];
        assert!(apply_lower_triangular(&l, 3, &[1.0, 0.0, 0.0]).is_err());
        assert!(apply_lower_triangular(&l, 2, &[1.0]).is_err());
    }

    #[test]
    fn apply_lower_triangular_handles_empty_input() {
        let out = apply_lower_triangular(&[], 0, &[]).expect("empty dimensions are consistent");
        assert!(out.is_empty());
    }
}

#[cfg(test)]
mod ledoit_wolf_tests {
    use super::ledoit_wolf_shrinkage;

    /// Hand-worked golden example (arithmetic reproduced in the rustdoc).
    ///
    /// T = 4 observations of N = 2 zero-mean factors, row-major:
    /// X = [(1, 1), (−1, −1), (2, −2), (−2, 2)]
    ///
    /// S = XᵀX/T = [[2.5, −1.5], [−1.5, 2.5]]
    /// μ = tr(S)/N = 2.5
    /// d² = ‖S − μI‖² = (0² + 1.5² + 1.5² + 0²)/2 = 2.25
    /// per-observation norms ‖x_t x_tᵀ − S‖²:
    /// t1/t2: outer = [[1,1],[1,1]], diff = [[−1.5,2.5],[2.5,−1.5]],
    /// Σ(entries²)/2 = (2·2.25 + 2·6.25)/2 = 8.5
    /// t3/t4: outer = [[4,−4],[−4,4]], diff = [[1.5,−2.5],[−2.5,1.5]],
    /// Σ(entries²)/2 = 8.5
    /// b̄² = (1/T²)·Σ_t ‖x_t x_tᵀ − S‖² = (4 · 8.5)/16 = 2.125
    /// b² = min(b̄², d²) = 2.125 ⇒ δ* = b²/d² = 2.125/2.25 = 17/18
    /// Σ* = δ*·μ·I + (1 − δ*)·S
    /// = [[2.5, −1.5/18], [−1.5/18, 2.5]] = [[2.5, −1/12], [−1/12, 2.5]]
    #[test]
    fn ledoit_wolf_matches_hand_worked_two_factor_example() {
        let observations = [1.0, 1.0, -1.0, -1.0, 2.0, -2.0, -2.0, 2.0];
        let result = ledoit_wolf_shrinkage(&observations, 4, 2).unwrap();
        assert!(
            (result.shrinkage - 17.0 / 18.0).abs() < 1e-14,
            "delta* must be 17/18, got {}",
            result.shrinkage
        );
        let expected = [2.5, -1.0 / 12.0, -1.0 / 12.0, 2.5];
        for (idx, (got, want)) in result.covariance.iter().zip(expected.iter()).enumerate() {
            assert!(
                (got - want).abs() < 1e-13,
                "covariance[{idx}]: expected {want}, got {got}"
            );
        }
    }

    #[test]
    fn ledoit_wolf_preserves_golden_estimate_under_extreme_rescaling() {
        let observations = [1.0, 1.0, -1.0, -1.0, 2.0, -2.0, -2.0, 2.0];
        for scale in [1e-150, 1e-100, 1.0, 1e100, 1e150] {
            let scaled = observations.map(|value| value * scale);
            let result = ledoit_wolf_shrinkage(&scaled, 4, 2).expect("finite covariance");
            assert!((result.shrinkage - 17.0 / 18.0).abs() < 1e-14);
            for (actual, expected) in
                result
                    .covariance
                    .iter()
                    .zip([2.5, -1.0 / 12.0, -1.0 / 12.0, 2.5])
            {
                assert!((actual / scale / scale - expected).abs() < 1e-13);
            }
        }
    }

    #[test]
    fn ledoit_wolf_centers_large_constants_without_erasing_other_columns() {
        let observations = [1e308, 1.0, 1e308, -1.0, 1e308, 2.0, 1e308, -2.0];
        let result = ledoit_wolf_shrinkage(&observations, 4, 2).expect("finite centered data");
        // S = diag(0, 2.5), mu = 1.25, d² = 25/16, b² = 9/32.
        assert!((result.shrinkage - 0.18).abs() < 1e-14);
        for (actual, expected) in result.covariance.iter().zip([0.225, 0.0, 0.0, 2.275]) {
            assert!((actual - expected).abs() < 1e-13);
        }
        let constant = ledoit_wolf_shrinkage(&[1e308; 8], 4, 2).expect("zero covariance");
        assert_eq!(constant.shrinkage, 0.0);
        assert_eq!(constant.covariance, [0.0; 4]);
    }

    #[test]
    fn ledoit_wolf_rejects_unrepresentable_covariance() {
        let result = ledoit_wolf_shrinkage(&[1e308, -1e308], 2, 1);
        assert!(matches!(result, Err(crate::Error::Validation(_))));
    }

    #[test]
    fn ledoit_wolf_restores_finite_covariance_when_scale_squared_overflows() {
        // E[X²] = (2 * (1.4e154)²) / 4 = 9.8e307 is finite, although
        // directly squaring the largest centered observation overflows.
        let result = ledoit_wolf_shrinkage(&[1.4e154, -1.4e154, 0.0, 0.0], 4, 1)
            .expect("representable variance");
        assert_eq!(result.shrinkage, 0.0);
        assert!((result.covariance[0] / 9.8e307 - 1.0).abs() < 1e-14);
    }

    /// Demeaning: adding a constant to a column must not change the estimate.
    #[test]
    fn ledoit_wolf_is_invariant_to_column_shifts() {
        let base = [1.0, 1.0, -1.0, -1.0, 2.0, -2.0, -2.0, 2.0];
        // Column 0 shifted by +10; column 1 unchanged.
        let shifted = [11.0, 1.0, 9.0, -1.0, 12.0, -2.0, 8.0, 2.0];
        let a = ledoit_wolf_shrinkage(&base, 4, 2).unwrap();
        let b = ledoit_wolf_shrinkage(&shifted, 4, 2).unwrap();
        assert!((a.shrinkage - b.shrinkage).abs() < 1e-12);
        for (x, y) in a.covariance.iter().zip(b.covariance.iter()) {
            assert!((x - y).abs() < 1e-12);
        }
    }

    /// Degenerate case d² = 0 (S is already a multiple of the identity):
    /// return S unchanged with shrinkage 0.
    ///
    /// X = [(1, 1), (1, −1), (−1, 1), (−1, −1)] → S = I, μ = 1, d² = 0.
    #[test]
    fn ledoit_wolf_degenerate_identity_sample_returns_s() {
        let observations = [1.0, 1.0, 1.0, -1.0, -1.0, 1.0, -1.0, -1.0];
        let result = ledoit_wolf_shrinkage(&observations, 4, 2).unwrap();
        assert!(result.shrinkage.abs() < 1e-14);
        let expected = [1.0, 0.0, 0.0, 1.0];
        for (got, want) in result.covariance.iter().zip(expected.iter()) {
            assert!((got - want).abs() < 1e-14);
        }
    }

    #[test]
    fn ledoit_wolf_rejects_bad_input() {
        // t < 2
        assert!(ledoit_wolf_shrinkage(&[1.0, 2.0], 1, 2).is_err());
        // n == 0
        assert!(ledoit_wolf_shrinkage(&[], 2, 0).is_err());
        // len mismatch
        assert!(ledoit_wolf_shrinkage(&[1.0, 2.0, 3.0], 2, 2).is_err());
        // non-finite
        assert!(ledoit_wolf_shrinkage(&[1.0, f64::NAN, 2.0, 3.0], 2, 2).is_err());
    }
}
