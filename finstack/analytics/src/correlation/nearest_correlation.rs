//! Higham (2002) nearest correlation matrix projection.
//!
//! Given a symmetric matrix `A` that is *approximately* a correlation matrix
//! (e.g. a sample correlation corrupted by estimation noise or a user-supplied
//! matrix with small PSD violations), this module finds a nearby valid
//! correlation matrix:
//!
//! ```text
//! X s.t. X = Xᵀ, diag(X) = 1, X ⪰ 0,   ‖X − A‖_F small.
//! ```
//!
//! Higham's alternating-projections algorithm iteratively projects onto the
//! PSD cone and the "unit diagonal" hyperplane until convergence. This
//! implementation carries a Dykstra correction term only for the PSD-cone
//! projection; the unit-diagonal projection step needs no correction term
//! because the unit-diagonal set is *affine*, and Dykstra increments are
//! provably unnecessary for affine sets (Boyle & Dykstra 1986). This is
//! exactly Higham (2002) Algorithm 3.3, which converges to the
//! Frobenius-nearest valid correlation matrix (up to the configured
//! tolerance). It is the standard remedy for real-world correlation matrices
//! that fail Cholesky by a small margin.
//!
//! # References
//!
//! - Higham, N. J. (2002). "Computing the nearest correlation matrix — A
//!   problem from finance." *IMA Journal of Numerical Analysis*, 22(3), 329–343.
//!   Algorithm 3.3.
//! - Boyle, J. P., & Dykstra, R. L. (1986). "A method for finding projections
//!   onto the intersection of convex sets in Hilbert spaces." *Advances in
//!   Order Restricted Statistical Inference*, Lecture Notes in Statistics 37,
//!   28–47.
//!
//! # When to use
//!
//! Use `nearest_correlation_matrix` when an upstream pipeline produces a matrix
//! that *should* be a correlation matrix but has small numerical defects
//! (typical causes: thresholded sample estimates, shrinkage, missing-data
//! imputation, user-edited blocks). When the input is wildly off — e.g.
//! asymmetric by more than rounding, diagonal very far from 1, or the target
//! application rejects any silent modification — validate first and let the
//! caller decide.

use super::error::{Error, Result};

/// Convergence parameters for [`nearest_correlation_matrix`].
///
/// The defaults (`max_iter = 200`, `tol = 1e-10`) are a conservative balance
/// between runtime and accuracy for correlation matrices up to ~50×50 that
/// typically arise in credit and rates applications.
#[derive(Debug, Clone, Copy)]
pub struct NearestCorrelationOpts {
    /// Maximum number of alternating projection iterations.
    pub max_iter: usize,
    /// Frobenius-norm tolerance on the change between successive iterates.
    pub tol: f64,
}

impl Default for NearestCorrelationOpts {
    fn default() -> Self {
        Self {
            max_iter: 200,
            tol: 1e-10,
        }
    }
}

/// Compute the nearest valid correlation matrix to `input` using Higham's
/// alternating-projection algorithm (Higham 2002, Algorithm 3.3).
///
/// The returned matrix is symmetric, has unit diagonal, and is positive
/// semidefinite. The iteration converges to the Frobenius-nearest valid
/// correlation matrix up to the configured tolerance: the Dykstra correction
/// is carried on the PSD-cone projection only, which suffices because the
/// unit-diagonal set is affine (Boyle & Dykstra 1986).
///
/// The input is expected to be nearly symmetric with a diagonal close to 1.
/// Gross violations (asymmetry beyond 1e-6, diagonal entries far from 1, etc.)
/// are rejected with an error rather than silently "fixed", because those
/// usually indicate upstream data corruption rather than numerical noise.
///
/// # Arguments
///
/// * `input` — Flattened `n × n` matrix in row-major order.
/// * `n`     — Matrix dimension.
/// * `opts`  — Convergence settings (use `NearestCorrelationOpts::default()`).
///
/// # Returns
///
/// A new `n × n` matrix (row-major) that is symmetric, has unit diagonal, and
/// is positive semidefinite.
///
/// # Errors
///
/// * [`Error::InvalidSize`] if `input.len() != n * n`.
/// * [`Error::NotSymmetric`] if the input deviates from symmetry by more than
///   `1e-6` at any off-diagonal entry.
/// * [`Error::DiagonalNotOne`] if any diagonal entry is further than `1e-3`
///   from `1.0`.
/// * [`Error::DidNotConverge`] if the algorithm fails to converge within
///   `opts.max_iter` iterations.
pub fn nearest_correlation_matrix(
    input: &[f64],
    n: usize,
    opts: NearestCorrelationOpts,
) -> Result<Vec<f64>> {
    if input.len() != n * n {
        return Err(Error::InvalidSize {
            expected: n,
            actual: input.len(),
        });
    }
    if n == 0 {
        return Ok(Vec::new());
    }

    // Sanity gates: reject pathological inputs. Small defects are fine —
    // those are precisely what the projection is designed to repair — but a
    // diagonal of 0.5 or a blatantly asymmetric entry is almost certainly a
    // data bug that must be surfaced, not silently reshaped.
    const SYMMETRY_GATE: f64 = 1e-6;
    const DIAGONAL_GATE: f64 = 1e-3;
    for i in 0..n {
        let diag = input[i * n + i];
        if (diag - 1.0).abs() > DIAGONAL_GATE {
            return Err(Error::DiagonalNotOne {
                index: i,
                value: diag,
            });
        }
        for j in (i + 1)..n {
            let diff = (input[i * n + j] - input[j * n + i]).abs();
            if diff > SYMMETRY_GATE {
                return Err(Error::NotSymmetric { i, j, diff });
            }
        }
    }

    // Symmetrize to machine precision before starting the projection so that
    // the eigensolver gets a perfectly symmetric iterate, then rescale to an
    // exact unit diagonal (x_ij / sqrt(d_i * d_j)). The diagonal gate above
    // only bounds the defect to 1e-3; the rescale repairs it so every return
    // path — including the early exit below — satisfies the unit-diagonal
    // postcondition and the strict 1e-10 tolerance used by
    // `validate_correlation_matrix`.
    let mut y = symmetrize(input, n);
    rescale_to_unit_diagonal(&mut y, n);

    // Early-exit on already-PSD inputs: a symmetric, unit-diagonal matrix
    // that passes Cholesky is positive definite, so it is its own
    // nearest-correlation projection. Common in daily recalibration where
    // the input only drifts within tolerance.
    if finstack_core::math::linalg::cholesky_correlation(&y, n).is_ok() {
        return Ok(y);
    }

    // Dykstra-projection iteration. Pre-allocate `prev`, `next`, `r`, `s`
    // outside the loop; swap `prev` and `next` per iteration to avoid the
    // per-iteration `clone()`.
    let mut s = vec![0.0_f64; n * n];
    let mut r = vec![0.0_f64; n * n];
    let mut prev = y;
    let mut next = vec![0.0_f64; n * n];
    for _ in 0..opts.max_iter {
        // Dykstra correction: r = prev - s
        for k in 0..(n * n) {
            r[k] = prev[k] - s[k];
        }

        let x = project_psd(&r, n);

        for k in 0..(n * n) {
            s[k] = x[k] - r[k];
        }

        // Project onto unit diagonal into `next`.
        next.copy_from_slice(&x);
        for i in 0..n {
            next[i * n + i] = 1.0;
        }

        if frobenius_diff(&next, &prev) < opts.tol {
            // The converged iterate is the *diagonal*-projected matrix, whose
            // smallest eigenvalue is only bounded by ≈ `opts.tol` from below.
            // One final PSD projection followed by a unit-diagonal rescale
            // (a congruence transform, which preserves PSD exactly) makes the
            // postcondition unconditional even for loose user tolerances.
            let mut out = project_psd(&next, n);
            rescale_to_unit_diagonal(&mut out, n);
            return Ok(out);
        }
        std::mem::swap(&mut prev, &mut next);
    }

    Err(Error::DidNotConverge {
        max_iter: opts.max_iter,
        tol: opts.tol,
    })
}

/// Rescale a symmetric matrix to an exact unit diagonal in place:
/// `x_ij ← x_ij / sqrt(d_i · d_j)` with the diagonal set to exactly `1.0`.
///
/// For a PSD input this is a congruence transform `D^{-1/2} X D^{-1/2}`, so
/// positive semidefiniteness is preserved. Diagonal entries are clamped to a
/// small positive floor before the square root as a defensive guard; inputs
/// reaching this helper always have diagonals near 1.
fn rescale_to_unit_diagonal(matrix: &mut [f64], n: usize) {
    const MIN_DIAGONAL: f64 = 1e-12;
    let inv_sqrt: Vec<f64> = (0..n)
        .map(|i| 1.0 / matrix[i * n + i].max(MIN_DIAGONAL).sqrt())
        .collect();
    for i in 0..n {
        for j in 0..n {
            matrix[i * n + j] *= inv_sqrt[i] * inv_sqrt[j];
        }
        matrix[i * n + i] = 1.0;
    }
}

/// Project a symmetric matrix onto the PSD cone by zeroing negative
/// eigenvalues (the Higham projection step).
///
/// Spectral decomposition delegates to
/// [`finstack_core::math::linalg::symmetric_eigen`] (divide-and-conquer
/// tridiagonal QR, `O(n³)`), which scales to portfolio-size correlation
/// matrices without the `O(n⁵)` worst case of a hand-rolled Jacobi
/// sweep.
fn project_psd(matrix: &[f64], n: usize) -> Vec<f64> {
    if n == 0 {
        return Vec::new();
    }

    // Defensive: `symmetric_eigen` only fails on shape mismatch; we
    // already know `matrix.len() == n * n` here, but fall through to a
    // zero matrix if the invariant is ever broken so callers observe a
    // degenerate projection instead of a panic.
    let Ok((eigenvalues, eigenvectors)) = finstack_core::math::linalg::symmetric_eigen(matrix, n)
    else {
        return vec![0.0; n * n];
    };

    // Reconstruct using only non-negative eigenvalues: X = V · diag(max(λ, 0)) · Vᵀ.
    // `eigenvectors[i * n + k]` is the `i`-th component of the `k`-th
    // eigenvector, matching the previous Jacobi layout.
    let mut out = vec![0.0_f64; n * n];
    for i in 0..n {
        for j in i..n {
            let mut sum = 0.0_f64;
            for k in 0..n {
                let lambda = eigenvalues[k].max(0.0);
                if lambda == 0.0 {
                    continue;
                }
                sum += lambda * eigenvectors[i * n + k] * eigenvectors[j * n + k];
            }
            out[i * n + j] = sum;
            out[j * n + i] = sum;
        }
    }
    out
}

fn symmetrize(matrix: &[f64], n: usize) -> Vec<f64> {
    let mut out = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            out[i * n + j] = 0.5 * (matrix[i * n + j] + matrix[j * n + i]);
        }
    }
    out
}

fn frobenius_diff(a: &[f64], b: &[f64]) -> f64 {
    debug_assert_eq!(a.len(), b.len());
    let mut acc = 0.0;
    for (x, y) in a.iter().zip(b.iter()) {
        let d = x - y;
        acc += d * d;
    }
    acc.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::correlation::validate_correlation_matrix;

    fn max_abs_diff(a: &[f64], b: &[f64]) -> f64 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).abs())
            .fold(0.0_f64, f64::max)
    }

    #[test]
    fn identity_is_fixed_point() {
        let input = vec![1.0, 0.0, 0.0, 1.0];
        let repaired =
            nearest_correlation_matrix(&input, 2, NearestCorrelationOpts::default()).expect("ok");
        assert!(max_abs_diff(&input, &repaired) < 1e-12);
    }

    #[test]
    fn already_valid_matrix_is_unchanged() {
        let input = vec![1.0, 0.5, 0.3, 0.5, 1.0, 0.4, 0.3, 0.4, 1.0];
        let repaired =
            nearest_correlation_matrix(&input, 3, NearestCorrelationOpts::default()).expect("ok");
        // Valid PSD correlation matrices are a fixed point of the projection.
        assert!(max_abs_diff(&input, &repaired) < 1e-8);
        validate_correlation_matrix(&repaired, 3).expect("repaired matrix is valid");
    }

    #[test]
    fn non_psd_matrix_is_projected_to_valid_correlation() {
        // Higham (2002) canonical counter-example: symmetric, unit diagonal,
        // but not PSD (smallest eigenvalue is negative).
        let input = vec![
            1.0, -0.55, -0.55, //
            -0.55, 1.0, -0.55, //
            -0.55, -0.55, 1.0,
        ];
        assert!(validate_correlation_matrix(&input, 3).is_err());

        let repaired =
            nearest_correlation_matrix(&input, 3, NearestCorrelationOpts::default()).expect("ok");
        validate_correlation_matrix(&repaired, 3).expect("repaired matrix is valid");

        for i in 0..3 {
            assert!((repaired[i * 3 + i] - 1.0).abs() < 1e-10);
            for j in (i + 1)..3 {
                let diff = (repaired[i * 3 + j] - repaired[j * 3 + i]).abs();
                assert!(diff < 1e-10);
            }
        }
    }

    #[test]
    fn rejects_wrong_size() {
        let input = vec![1.0, 0.5, 0.5, 1.0];
        let err = nearest_correlation_matrix(&input, 3, NearestCorrelationOpts::default())
            .expect_err("size mismatch");
        assert!(matches!(err, Error::InvalidSize { .. }));
    }

    #[test]
    fn rejects_diagonal_far_from_one() {
        let input = vec![0.5, 0.1, 0.1, 1.0];
        let err = nearest_correlation_matrix(&input, 2, NearestCorrelationOpts::default())
            .expect_err("diagonal guard");
        assert!(matches!(err, Error::DiagonalNotOne { .. }));
    }

    #[test]
    fn rejects_gross_asymmetry() {
        let input = vec![1.0, 0.5, 0.3, 1.0];
        let err = nearest_correlation_matrix(&input, 2, NearestCorrelationOpts::default())
            .expect_err("symmetry guard");
        assert!(matches!(err, Error::NotSymmetric { .. }));
    }

    /// The algorithm is Higham (2002) Algorithm 3.3: the Dykstra correction is
    /// carried on the PSD-cone projection only, which suffices for optimality
    /// because the unit-diagonal set is affine (Boyle & Dykstra 1986). It
    /// therefore converges to the Frobenius-nearest valid correlation matrix.
    ///
    /// For the 3×3 equicorrelation counter-example at ρ = −0.55, positive
    /// semidefiniteness requires ρ ≥ −1/2, and the unique nearest correlation
    /// matrix (which inherits the input's permutation symmetry) has all
    /// off-diagonals = −0.5 — Frobenius distance √(6·0.05²) ≈ 0.1225. This
    /// test locks in convergence to that analytic optimum.
    #[test]
    fn converges_to_frobenius_nearest_matrix() {
        // Higham (2002) canonical counter-example: symmetric, unit diagonal,
        // but not PSD (smallest eigenvalue ≈ −0.165).
        let input = vec![
            1.0, -0.55, -0.55, //
            -0.55, 1.0, -0.55, //
            -0.55, -0.55, 1.0,
        ];
        let repaired =
            nearest_correlation_matrix(&input, 3, NearestCorrelationOpts::default()).expect("ok");

        // The repaired matrix must be a valid correlation matrix.
        validate_correlation_matrix(&repaired, 3).expect("repaired is a valid correlation matrix");

        // It must be "nearby" — the Frobenius distance must be strictly less
        // than the input's own Frobenius norm (i.e. not a wild extrapolation).
        let frob_dist = frobenius_diff(&input, &repaired);
        let frob_input = frobenius_diff(&input, &[0.0; 9]);
        assert!(
            frob_dist < frob_input,
            "Frobenius distance to repaired ({frob_dist:.6}) should be less than input norm ({frob_input:.6})"
        );

        // All off-diagonals converge to the analytic Frobenius-nearest
        // optimum of −0.5 (the PSD boundary for 3×3 equicorrelation).
        let nearest_offdiag = -0.5_f64;
        for i in 0..3 {
            for j in 0..3 {
                let expected = if i == j { 1.0 } else { nearest_offdiag };
                let diff = (repaired[i * 3 + j] - expected).abs();
                assert!(
                    diff < 1e-6,
                    "nearest-correlation regression: repaired[{i},{j}]={:.6} expected {expected:.6} (diff={diff:.2e})",
                    repaired[i * 3 + j]
                );
            }
        }
    }

    /// The early-exit path must repair a within-gate diagonal defect: a PSD
    /// input with diagonals up to 1e-3 off must still come back with an exact
    /// unit diagonal that passes the strict validator.
    #[test]
    fn early_exit_repairs_within_gate_diagonal_defect() {
        let input = vec![
            1.0005, 0.2, 0.1, //
            0.2, 0.9995, 0.3, //
            0.1, 0.3, 1.0002,
        ];
        let repaired =
            nearest_correlation_matrix(&input, 3, NearestCorrelationOpts::default()).expect("ok");
        for i in 0..3 {
            assert!(
                (repaired[i * 3 + i] - 1.0).abs() < 1e-15,
                "diag[{i}] = {}",
                repaired[i * 3 + i]
            );
        }
        validate_correlation_matrix(&repaired, 3).expect("repaired matrix passes strict validator");
    }

    /// Iteration-budget exhaustion must surface as `DidNotConverge`, not as a
    /// (misleading) PSD-validation failure.
    #[test]
    fn iteration_exhaustion_reports_did_not_converge() {
        let input = vec![
            1.0, -0.55, -0.55, //
            -0.55, 1.0, -0.55, //
            -0.55, -0.55, 1.0,
        ];
        let opts = NearestCorrelationOpts {
            max_iter: 1,
            tol: 1e-16,
        };
        let err = nearest_correlation_matrix(&input, 3, opts).expect_err("must not converge");
        assert!(matches!(err, Error::DidNotConverge { max_iter: 1, .. }));
    }

    /// With a loose user tolerance the converged iterate alone is only PSD up
    /// to ~tol; the final projection + rescale must still produce a matrix
    /// that passes the strict validator.
    #[test]
    fn loose_tolerance_output_still_passes_strict_validation() {
        let input = vec![
            1.0, -0.55, -0.55, //
            -0.55, 1.0, -0.55, //
            -0.55, -0.55, 1.0,
        ];
        let opts = NearestCorrelationOpts {
            max_iter: 200,
            tol: 1e-4,
        };
        let repaired = nearest_correlation_matrix(&input, 3, opts).expect("converges");
        validate_correlation_matrix(&repaired, 3).expect("valid despite loose tolerance");
    }

    /// Smoke test for the `n > 40` regime: the divide-and-conquer
    /// `symmetric_eigen` path must produce a valid correlation matrix
    /// in fractions of a second.
    #[test]
    fn nearest_corr_scales_past_forty_dimensions() {
        let n = 60;
        // Construct a "near-correlation" matrix: identity plus a low-rank
        // perturbation that makes some eigenvalues mildly negative.
        let mut input = vec![0.0; n * n];
        for i in 0..n {
            input[i * n + i] = 1.0;
            for j in (i + 1)..n {
                let rho = 0.2 + 0.6 * ((i as f64 + 1.0) / (j as f64 + 1.0)); // off-diag > 1 sometimes
                let rho = rho.clamp(-0.9, 0.9);
                input[i * n + j] = rho;
                input[j * n + i] = rho;
            }
        }

        let opts = NearestCorrelationOpts {
            max_iter: 400,
            tol: 1e-9,
        };
        let repaired = nearest_correlation_matrix(&input, n, opts).expect("converges");

        // Unit diagonal, symmetry, and PSD — the three invariants the
        // projection must restore.
        validate_correlation_matrix(&repaired, n).expect("valid correlation matrix");
        for i in 0..n {
            assert!(
                (repaired[i * n + i] - 1.0).abs() < 1e-9,
                "diag[{i}] = {}",
                repaired[i * n + i]
            );
            for j in (i + 1)..n {
                let d = (repaired[i * n + j] - repaired[j * n + i]).abs();
                assert!(d < 1e-10, "asym at ({i},{j}): {d}");
            }
        }
    }
}
