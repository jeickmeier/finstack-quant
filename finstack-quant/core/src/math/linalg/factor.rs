//! Correlation factor representation and correlation Cholesky factorization.

use super::cholesky::lower_triangular_mul;
use super::errors::{
    CholeskyError, PIVOT_TOLERANCE_RELATIVE, check_square_finite, check_symmetric,
};

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
    pub(super) factor: Vec<f64>,
    /// Matrix dimension.
    pub(super) n: usize,
    /// Whether [`Self::factor`] is exactly lower triangular (all entries
    /// above the diagonal are `+0.0`).
    ///
    /// Certified by inspection of the stored matrix, independent of whether
    /// pivoting occurred. Enables a half-matrix hot path in [`Self::apply`].
    /// The dense and triangular loops agree bit-for-bit on finite shocks
    /// because the skipped terms are `0.0 * z` additions that cannot change
    /// the running sum.
    pub(super) triangular: bool,
}

impl CorrelationFactor {
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
    /// Returns [`CholeskyError::DimensionMismatch`] if either slice length differs from `n`,
    /// [`CholeskyError::NonFiniteInput`] if `independent` contains a non-finite entry (leaving
    /// `correlated` untouched), or [`CholeskyError::NonFiniteOutput`] if any computed shock
    /// overflows to a non-finite value.
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
        for (i, &z_i) in independent.iter().enumerate() {
            if !z_i.is_finite() {
                return Err(CholeskyError::NonFiniteInput {
                    row: i,
                    col: 0,
                    value: z_i,
                });
            }
        }
        let n = self.n;
        if self.triangular {
            // Half-matrix path: entries above the diagonal are exactly zero,
            // so skipping them cannot change the running sum on finite input.
            lower_triangular_mul(&self.factor, n, independent, correlated)
                .map_err(|(row, value)| CholeskyError::NonFiniteOutput { value, row })?;
        } else {
            for (i, out) in correlated.iter_mut().enumerate() {
                let mut sum = 0.0;
                for (j, &z_j) in independent.iter().enumerate() {
                    sum += self.factor[i * n + j] * z_j;
                }
                if !sum.is_finite() {
                    return Err(CholeskyError::NonFiniteOutput { value: sum, row: i });
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
    /// * `factor` - Finite row-major `n × n` factor; length must be exactly `n * n`.
    /// * `n` - Matrix dimension; zero accepts only an empty factor.
    ///
    /// # Errors
    ///
    /// Returns [`CholeskyError::DimensionMismatch`] if `n * n` overflows or
    /// differs from the buffer length, or [`CholeskyError::NonFiniteInput`]
    /// for the first NaN or infinite entry. Dense finite matrices are valid.
    pub fn from_parts(factor: Vec<f64>, n: usize) -> std::result::Result<Self, CholeskyError> {
        check_square_finite(&factor, n)?;
        let triangular = is_exactly_lower_triangular(&factor, n);
        Ok(Self {
            factor,
            n,
            triangular,
        })
    }
}

/// True when every strictly-upper-triangular entry is exactly `+0.0`.
pub(super) fn is_exactly_lower_triangular(factor: &[f64], n: usize) -> bool {
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
/// factor models, copulas). For solver normal equations use [`cholesky_decomposition`](super::cholesky_decomposition)
/// and [`cholesky_solve`](super::cholesky_solve) instead.
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
/// [`CorrelationFactor`] holding the factor in the original variable order. It
/// satisfies `L Lᵀ = matrix` but is generally not lower triangular once pivoting
/// reorders variables; apply it with [`CorrelationFactor::apply`], not
/// [`apply_lower_triangular`](super::apply_lower_triangular).
///
/// # Errors
///
/// - [`CholeskyError::DimensionMismatch`] if `matrix.len() != n * n`
/// - [`CholeskyError::NonFiniteInput`] if any entry is NaN or infinite
/// - [`CholeskyError::NotSymmetric`] if the matrix violates symmetry beyond relative tolerance
/// - [`CholeskyError::NotPositiveDefinite`] if a diagonal or pivot is significantly negative
///
/// # Example
///
/// ```
/// use finstack_quant_core::math::linalg::cholesky_correlation;
///
/// // Well-conditioned 2×2
/// let corr = vec![1.0, 0.5, 0.5, 1.0];
/// let f = cholesky_correlation(&corr, 2).unwrap();
/// assert_eq!(f.factor_matrix().len(), 4);
///
/// // Near-singular (rho ≈ 1): pivoted Cholesky handles it gracefully
/// let near_singular = vec![1.0, 0.9999999, 0.9999999, 1.0];
/// assert!(cholesky_correlation(&near_singular, 2).is_ok());
/// ```
pub fn cholesky_correlation(
    matrix: &[f64],
    n: usize,
) -> std::result::Result<CorrelationFactor, CholeskyError> {
    // Reject non-finite inputs up front: NaN otherwise propagates through the
    // pivot selection (`total_cmp` orders NaN above all numbers) and would be
    // absorbed into an Ok(NaN factor).
    check_square_finite(matrix, n)?;
    if n == 0 {
        return CorrelationFactor::from_parts(vec![], 0);
    }
    check_symmetric(matrix, n)?;

    // Maximum diagonal magnitude — used to set scale-relative tolerance.
    let max_diag = (0..n)
        .map(|i| matrix[i * n + i].abs())
        .fold(0.0_f64, f64::max);
    let tol = PIVOT_TOLERANCE_RELATIVE * max_diag;

    for row in 0..n {
        let diag = matrix[row * n + row];
        if diag < -tol {
            return Err(CholeskyError::NotPositiveDefinite { diag, row });
        }
    }

    // Work copy of the matrix that we reduce in-place (Schur complement updates).
    let mut a: Vec<f64> = matrix.to_vec();

    // perm[k] holds the original variable index placed at pivot position k.
    let mut perm: Vec<usize> = (0..n).collect();

    // Lower-triangular factor in pivoted order (unpermuted before return).
    let mut l_piv = vec![0.0_f64; n * n];

    let mut effective_rank = 0usize;

    for step in 0..n {
        // Complete diagonal pivoting: find largest remaining diagonal, breaking
        // equal-pivot ties toward the smallest index so identity/equicorrelation
        // inputs preserve index order `[0..n]`.
        let pivot_idx = (step..n)
            .max_by(|&ai, &bi| {
                a[ai * n + ai]
                    .total_cmp(&a[bi * n + bi])
                    .then_with(|| bi.cmp(&ai))
            })
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

    // `from_parts` inspects the unpermuted factor. Equal-diagonal correlation
    // inputs preserve index order at step 0, whereas larger later pivots place
    // non-zero entries above the diagonal.
    CorrelationFactor::from_parts(l_orig, n)
}

// ─── Symmetric eigendecomposition (shared helper) ──────────────────────────────
