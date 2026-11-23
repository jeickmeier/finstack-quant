//! Symmetric eigendecomposition, Cholesky decomposition/solve, and triangular matrix helpers.

use super::errors::{
    CholeskyError, CorrelationError, DIAGONAL_TOLERANCE, SINGULAR_THRESHOLD, SYMMETRY_TOLERANCE,
    check_square_finite, check_symmetric, square_len, square_len_label,
};
use super::factor::cholesky_correlation;
use crate::Result;

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
/// Returns [`CholeskyError::DimensionMismatch`] if `matrix.len() != n * n`,
/// [`CholeskyError::NonFiniteInput`] if `matrix` contains NaN or infinity, or
/// [`CholeskyError::NotSymmetric`] if `matrix` violates symmetry beyond
/// relative tolerance.
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
/// # Arguments
///
/// * `matrix` - Symmetric square matrix in row-major order with exactly `n * n`
///   entries.
/// * `n` - Matrix dimension used to interpret the flat row-major buffer.
pub fn symmetric_eigen(
    matrix: &[f64],
    n: usize,
) -> std::result::Result<(Vec<f64>, Vec<f64>), CholeskyError> {
    check_square_finite(matrix, n)?;
    if n == 0 {
        return Ok((Vec::new(), Vec::new()));
    }
    check_symmetric(matrix, n)?;

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
/// * `flat` - Row-major square matrix storage; `flat.len()` must equal `n * n`.
/// * `n` - Matrix dimension (rows == columns); `0` yields no rows.
///
/// # Errors
///
/// Returns [`CholeskyError::DimensionMismatch`] when `flat.len() != n * n`
/// (including when `n * n` overflows `usize`).
///
/// # Examples
///
/// ```
/// use finstack_quant_core::math::linalg::unflatten_square;
/// let rows = unflatten_square(&[1.0, 2.0, 3.0, 4.0], 2)?;
/// assert_eq!(rows, vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
/// assert!(unflatten_square(&[1.0, 2.0, 3.0], 2).is_err());
/// # Ok::<(), finstack_quant_core::math::linalg::CholeskyError>(())
/// ```
pub fn unflatten_square(
    flat: &[f64],
    n: usize,
) -> std::result::Result<Vec<Vec<f64>>, CholeskyError> {
    if square_len(n) != Some(flat.len()) {
        return Err(CholeskyError::DimensionMismatch {
            expected: n,
            actual: flat.len(),
        });
    }
    if n == 0 {
        return Ok(Vec::new());
    }
    Ok(flat.chunks_exact(n).map(<[f64]>::to_vec).collect())
}

/// Flatten a row-major nested matrix after validating squareness against `n`.
///
/// The inverse of [`unflatten_square`]: host bindings use it to turn a
/// nested `n x n` list into the row-major buffer the matrix kernels take.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] when the matrix has the
/// wrong number of rows or any row has the wrong number of columns.
///
/// # Arguments
///
/// * `matrix` - Row-major nested vector with one inner vector per row.
/// * `n` - Expected square dimension.
/// * `label` - Caller-provided label included in validation messages.
///
/// # Examples
///
/// ```
/// use finstack_quant_core::math::linalg::flatten_square_matrix;
/// let flat = flatten_square_matrix(vec![vec![1.0, 2.0], vec![3.0, 4.0]], 2, "cov")?;
/// assert_eq!(flat, vec![1.0, 2.0, 3.0, 4.0]);
/// assert!(flatten_square_matrix(vec![vec![1.0, 2.0]], 2, "cov").is_err());
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn flatten_square_matrix(
    matrix: Vec<Vec<f64>>,
    n: usize,
    label: &str,
) -> crate::Result<Vec<f64>> {
    if matrix.len() != n {
        return Err(crate::Error::Validation(format!(
            "{label} must have {n} rows, got {}",
            matrix.len()
        )));
    }
    let mut flat = Vec::with_capacity(n * n);
    for (index, row) in matrix.into_iter().enumerate() {
        if row.len() != n {
            return Err(crate::Error::Validation(format!(
                "{label} row {index} must have {n} columns, got {}",
                row.len()
            )));
        }
        flat.extend(row);
    }
    Ok(flat)
}

/// Copy a dense matrix into nested rows (`rows[i][j] == matrix[(i, j)]`).
///
/// The row-major nested form is the shape the host bindings exchange and the
/// shape `to_rows` accessors on matrix-backed model types return.
///
/// # Arguments
///
/// * `matrix` - Dense matrix of any shape; a matrix with zero rows yields an
///   empty vector, one with zero columns yields that many empty rows.
///
/// # Examples
///
/// ```
/// use finstack_quant_core::math::linalg::rows_of;
/// let m = nalgebra::DMatrix::from_row_slice(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
/// assert_eq!(rows_of(&m), vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]]);
/// ```
#[must_use]
pub fn rows_of(matrix: &nalgebra::DMatrix<f64>) -> Vec<Vec<f64>> {
    matrix
        .row_iter()
        .map(|row| row.iter().copied().collect())
        .collect()
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
/// - Matrix is not symmetric within relative tolerance (`NotSymmetric`)
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
    check_square_finite(matrix, n)?;
    check_symmetric(matrix, n)?;

    let matrix_scale = (0..n)
        .map(|i| matrix[i * n + i].abs())
        .fold(0.0_f64, f64::max);
    let factor_scale = matrix_scale.sqrt();

    if n > 0 && matrix_scale == 0.0 {
        return Err(CholeskyError::Singular {
            value: 0.0,
            row: 0,
            col: 0,
            threshold: 0.0,
        });
    }

    let mut l = vec![0.0; matrix.len()];
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

    for value in &mut l {
        *value *= factor_scale;
    }

    Ok(l)
}
/// Apply a lower-triangular factor `L` to a vector `z`, returning `L z`.
///
/// This is the Cholesky *apply* step used to turn a vector of i.i.d. standard
/// normals into correlated normals: if `Sigma = L L^T` and `z ~ N(0, I)`, then
/// `L z ~ N(0, Sigma)`. Use [`CorrelationFactor::apply`](super::CorrelationFactor::apply) on hot Monte Carlo
/// paths where the allocation matters.
///
/// `l` is the same **row-major, `n * n`, lower-triangular** flat layout that
/// [`cholesky_decomposition`] returns — entry `(i, j)` lives at `l[i * n + j]`.
/// Only the lower triangle (`j <= i`) is read; the upper triangle is assumed
/// zero and is ignored rather than validated.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`], naming both lengths, if `l` does not
/// hold exactly `n * n` entries (including when `n * n` overflows `usize`) or
/// `z.len() != n`.
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
/// - [`CorrelationFactor::apply`](super::CorrelationFactor::apply) for the pivoted, rank-aware, allocation-free factor.
pub fn apply_lower_triangular(l: &[f64], n: usize, z: &[f64]) -> Result<Vec<f64>> {
    validate_lower_factor(l, n)?;
    if z.len() != n {
        return Err(crate::Error::Validation(format!(
            "vector has length {} but the factor is {n}x{n}",
            z.len()
        )));
    }
    let mut out = vec![0.0; n];
    apply_lower_row(l, n, z, &mut out)?;
    Ok(out)
}

/// Apply one lower-triangular factor to consecutive rows of observations.
///
/// # Arguments
/// * `l` - Square row-major factor with `n * n` elements, typically a Cholesky factor.
///   The upper triangle is ignored.
/// * `n` - Number of variables per row and factor dimension.
/// * `z` - Row-major independent observations with `n` finite values per row.
///
/// # Errors
/// Rejects incompatible dimensions, non-finite input, or multiplication overflow.
pub fn apply_lower_triangular_many(l: &[f64], n: usize, z: &[f64]) -> Result<Vec<f64>> {
    validate_lower_factor(l, n)?;
    if n == 0 && z.is_empty() {
        return Ok(Vec::new());
    }
    if n == 0 || !z.len().is_multiple_of(n) {
        return Err(crate::Error::Validation(format!(
            "each observation row must have {n} entries"
        )));
    }
    let mut out = vec![0.0; z.len()];
    for (row, transformed) in z.chunks_exact(n).zip(out.chunks_exact_mut(n)) {
        apply_lower_row(l, n, row, transformed)?;
    }
    Ok(out)
}

pub(super) fn validate_lower_factor(l: &[f64], n: usize) -> Result<()> {
    if square_len(n) != Some(l.len()) {
        return Err(crate::Error::Validation(format!(
            "lower-triangular factor has {} entries but a {n}x{n} factor needs {}",
            l.len(),
            square_len_label(n)
        )));
    }
    for i in 0..n {
        let row = i * n;
        for &value in &l[row..=row + i] {
            if !value.is_finite() {
                return Err(crate::Error::Input(
                    crate::error::InputError::NonFiniteValue {
                        kind: crate::error::NonFiniteKind::classify(value),
                    },
                ));
            }
        }
    }

    Ok(())
}

pub(super) fn apply_lower_row(l: &[f64], n: usize, z: &[f64], out: &mut [f64]) -> Result<()> {
    for &value in z {
        if !value.is_finite() {
            return Err(crate::Error::Input(
                crate::error::InputError::NonFiniteValue {
                    kind: crate::error::NonFiniteKind::classify(value),
                },
            ));
        }
    }
    lower_triangular_mul(l, n, z, out).map_err(|(row, value)| {
        crate::Error::Internal(format!(
            "lower-triangular product produced non-finite value at row {row}: {value}"
        ))
    })?;
    Ok(())
}

/// `out = L z` for a row-major `n × n` lower-triangular `l`, reading only the
/// lower half. Callers have checked `l.len() == n * n` and `z.len() == out.len() == n`.
pub(super) fn lower_triangular_mul(
    l: &[f64],
    n: usize,
    z: &[f64],
    out: &mut [f64],
) -> std::result::Result<(), (usize, f64)> {
    for (i, slot) in out.iter_mut().enumerate() {
        let row = i * n;
        let mut sum = 0.0;
        for (&l_j, &z_j) in l[row..=row + i].iter().zip(z[..=i].iter()) {
            sum += l_j * z_j;
        }
        if !sum.is_finite() {
            return Err((i, sum));
        }
        *slot = sum;
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
    if square_len(n) != Some(chol.len()) {
        return Err(crate::Error::Validation(format!(
            "Cholesky factor has {} entries but a right-hand side of length {n} needs {}",
            chol.len(),
            square_len_label(n)
        )));
    }
    if x.len() != n {
        return Err(crate::Error::Validation(format!(
            "solution buffer has length {} but the right-hand side has length {n}",
            x.len()
        )));
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
            return Err(crate::Error::Validation(
                "Cholesky factor has a near-zero diagonal; the matrix is singular".to_string(),
            ));
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
/// Validate a flattened row-major correlation matrix with located diagnostics.
///
/// Checks matrix size, unit diagonal, coefficient bounds, symmetry, and
/// positive semidefiniteness in that order. The diagonal check uses `1e-10`
/// and the symmetry check uses [`SYMMETRY_TOLERANCE`]; positive
/// semidefiniteness is tested with [`cholesky_correlation`].
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
    if square_len(n) != Some(matrix.len()) {
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
            if !(-1.0..=1.0).contains(&value) {
                return Err(CorrelationError::OutOfBounds { i, j, value });
            }
        }
    }

    for i in 0..n {
        for j in (i + 1)..n {
            let diff = (matrix[i * n + j] - matrix[j * n + i]).abs();
            if diff > SYMMETRY_TOLERANCE {
                return Err(CorrelationError::NotSymmetric { i, j, diff });
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
