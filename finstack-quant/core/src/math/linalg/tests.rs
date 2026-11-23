use super::cholesky::*;
use super::errors::*;
use super::factor::*;
use super::*;

mod unit_tests {
    use super::*;

    #[test]
    fn flatten_square_matrix_validates_shape() {
        let flat = flatten_square_matrix(vec![vec![1.0, 2.0], vec![3.0, 4.0]], 2, "cov")
            .expect("valid matrix");
        assert_eq!(flat, vec![1.0, 2.0, 3.0, 4.0]);
        assert!(flatten_square_matrix(vec![vec![1.0, 2.0]], 2, "cov").is_err());
        assert!(
            flatten_square_matrix(vec![vec![1.0, 2.0, 3.0], vec![1.0, 2.0]], 2, "cov").is_err()
        );
    }

    /// Numerical rank of a pivoted factor: the retained pivots are exactly
    /// the columns holding a non-zero entry.
    fn rank(factor: &CorrelationFactor) -> usize {
        let n = factor.n;
        (0..n)
            .filter(|&j| (0..n).any(|i| factor.factor[i * n + j] != 0.0))
            .count()
    }

    /// A dimension whose square overflows `usize` on this target, so an
    /// unchecked `n * n` wraps (to 0 for `2^(BITS/2)`) and a wrong-length
    /// buffer would pass the size check.
    const WRAPPING_N: [usize; 2] = [1 << (usize::BITS / 2), usize::MAX];

    #[test]
    fn size_checks_reject_a_square_that_overflows() {
        for n in WRAPPING_N {
            assert!(cholesky_decomposition(&[], n).is_err(), "n = {n}");
            assert!(cholesky_correlation(&[], n).is_err(), "n = {n}");
            assert!(symmetric_eigen(&[], n).is_err(), "n = {n}");
            assert!(apply_lower_triangular(&[], n, &[]).is_err(), "n = {n}");
            assert!(ledoit_wolf_shrinkage(&[], 2, n).is_err(), "n = {n}");
            let size_error = validate_correlation_matrix(&[], n).expect_err("wrapping n");
            assert!(matches!(size_error, CorrelationError::InvalidSize { .. }));
            // Display must not multiply the dimension either.
            assert!(size_error.to_string().contains("entries, got 0"));
        }
    }

    #[test]
    fn cholesky_solve_names_both_lengths() {
        let mut x = [0.0; 3];
        let error = cholesky_solve(&[2.0, 0.0, 1.0, 1.0], &[2.0, 1.0, 0.0], &mut x)
            .expect_err("a 2x2 factor cannot solve a length-3 system");
        assert_eq!(error.kind(), crate::ErrorKind::Validation);
        assert!(
            error
                .to_string()
                .contains("4 entries but a right-hand side of length 3 needs 9"),
            "{error}"
        );
        let mut x = [0.0; 2];
        cholesky_solve(&[2.0, 0.0, 1.0, 1.0], &[2.0, 1.0], &mut x).expect("2x2 solve");
        assert_eq!(x, [0.5, 0.0]);
    }

    #[test]
    fn correlation_error_kind_survives_the_core_fold() {
        use crate::ErrorKind;
        let cases = [
            (
                CorrelationError::DidNotConverge {
                    max_iter: 3,
                    tol: 1e-9,
                },
                ErrorKind::Computation,
            ),
            (
                CorrelationError::EigenDecompositionFailed,
                ErrorKind::Computation,
            ),
            (
                CorrelationError::InvalidSize {
                    expected: 2,
                    actual: 3,
                },
                ErrorKind::Validation,
            ),
            (
                CorrelationError::DiagonalNotOne {
                    index: 0,
                    value: 2.0,
                },
                ErrorKind::Validation,
            ),
        ];
        for (error, kind) in cases {
            let message = error.to_string();
            let core = crate::Error::from(error);
            assert_eq!(core.kind(), kind);
            assert!(core.to_string().contains(&message), "{core}");
        }
    }

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
        let n = factor.n;
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
        // Equal-pivot tie-breaking selects the smallest index, so identity and
        // equicorrelation matrices preserve index order `[0..n]` and stay lower
        // triangular.
        let n = 8;
        let identity: Vec<f64> = (0..n * n)
            .map(|k| if k / n == k % n { 1.0 } else { 0.0 })
            .collect();
        let id_factor = cholesky_correlation(&identity, n).expect("valid");
        assert!(id_factor.triangular);
        assert_eq!(id_factor.factor_matrix(), identity.as_slice());

        let rho = 0.5_f64;
        let corr: Vec<f64> = (0..n * n)
            .map(|k| {
                let (i, j) = (k / n, k % n);
                if i == j { 1.0 } else { rho }
            })
            .collect();
        let factor = cholesky_correlation(&corr, n).expect("valid");
        assert!(factor.triangular);

        // When Schur-complement diagonals reorder variables (step 1 has
        // `a[1,1] = 0.19 < a[2,2] = 1.0`), the unpermuted factor is NOT lower
        // triangular and the flag must turn off while `apply` stays exact.
        let reordered = vec![
            1.0, 0.9, 0.0, //
            0.9, 1.0, 0.2, //
            0.0, 0.2, 1.0,
        ];
        let reordered_factor = cholesky_correlation(&reordered, 3).expect("valid");
        assert!(!reordered_factor.triangular);
        let z3 = [0.4, -0.6, 0.2];
        let mut fast3 = vec![0.0; 3];
        reordered_factor.apply(&z3, &mut fast3).expect("matching");
        let mut ref3 = vec![0.0; 3];
        dense_apply(&reordered_factor, &z3, &mut ref3);
        assert_eq!(fast3, ref3);

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
        let factor = CorrelationFactor::from_parts(l, n).expect("finite square factor");
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
    fn from_parts_validates_in_every_build_profile() {
        for (factor, n) in [
            (vec![], 1),
            (vec![1.0; 3], 2),
            (vec![1.0; 5], 2),
            (vec![], usize::MAX),
            (vec![1.0], 0),
        ] {
            assert!(matches!(
                CorrelationFactor::from_parts(factor, n),
                Err(CholeskyError::DimensionMismatch { .. })
            ));
        }
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(matches!(
                CorrelationFactor::from_parts(vec![1.0, value, 0.0, 1.0], 2),
                Err(CholeskyError::NonFiniteInput { row: 0, col: 1, .. })
            ));
        }
        let empty = CorrelationFactor::from_parts(vec![], 0).expect("empty factor");
        assert!(empty.factor_matrix().is_empty());
        empty.apply(&[], &mut []).expect("empty application");
        let dense =
            CorrelationFactor::from_parts(vec![1.0, 0.5, 0.5, 1.0], 2).expect("dense factor");
        assert!(matches!(
            dense.apply(&[1.0], &mut [0.0; 2]),
            Err(CholeskyError::DimensionMismatch { .. })
        ));
        assert!(matches!(
            dense.apply(&[1.0; 2], &mut [0.0]),
            Err(CholeskyError::DimensionMismatch { .. })
        ));
    }

    #[test]
    fn from_parts_factor_uses_the_dense_path() {
        // Callers may hand a full (non-triangular) matrix to `from_parts`;
        // it must take the dense path.
        let corr = vec![1.0, 0.5, 0.5, 1.0];
        let factor = CorrelationFactor::from_parts(corr.clone(), 2).expect("finite dense factor");
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
        assert_eq!(rank(&f), f.n);
        assert_eq!(rank(&f), 2);

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
        // Equal-diagonal correlation preserves original order `[0, 1]` -> lower triangular.
        let corr = vec![1.0, 0.5, 0.5, 1.0];
        let f_corr = cholesky_correlation(&corr, 2).expect("should succeed");
        let mut z_corr = vec![0.0; 2];
        f_corr
            .apply(&[1.0, 0.0], &mut z_corr)
            .expect("dimension match");
        assert!((z_corr[0] - 1.0).abs() < 1e-12, "z_corr[0] = {}", z_corr[0]);
        assert!((z_corr[1] - 0.5).abs() < 1e-12, "z_corr[1] = {}", z_corr[1]);

        // Unequal diagonal `[1.0, 4.0]` pivots variable 1 first, placing `0.5` above the
        // diagonal in original variable order: `factor = [[sqrt(0.75), 0.5], [0.0, 2.0]]`.
        let cov = vec![1.0, 1.0, 1.0, 4.0];
        let f_piv = cholesky_correlation(&cov, 2).expect("should succeed");
        f_piv
            .apply(&[0.0, 1.0], &mut z_corr)
            .expect("dimension match");
        assert!((z_corr[0] - 0.5).abs() < 1e-12, "z_corr[0] = {}", z_corr[0]);
        assert!((z_corr[1] - 2.0).abs() < 1e-12, "z_corr[1] = {}", z_corr[1]);
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
        assert_eq!(rank(&f), f.n);
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
        assert_eq!(rank(&f), 3);
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
        assert!(rank(&f) <= 2);
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
            rank(&f),
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
        assert_eq!(rank(&factor), 1);
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
            assert_eq!(rank(&factor), 2);
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
        assert_eq!(rank(&f), f.n);

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
    fn cholesky_decomposition_rejects_non_positive_definite() {
        let indefinite = vec![1.0_f64, 2.0, 2.0, 1.0];

        assert!(matches!(
            cholesky_decomposition(&indefinite, 2),
            Err(CholeskyError::NotPositiveDefinite { .. })
        ));
    }

    #[test]
    fn cholesky_decomposition_rejects_non_finite_entries() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(matches!(
                cholesky_decomposition(&[bad], 1),
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
    fn apply_lower_triangular_many_keeps_consecutive_observation_rows() {
        let factor = [2.0, f64::NAN, 1.0, 3.0];
        let rows = [1.0, 2.0, -1.0, 1.0, 0.5, 0.0];
        assert_eq!(
            apply_lower_triangular_many(&factor, 2, &rows).expect("three valid draws"),
            [2.0, 7.0, -2.0, 2.0, 1.0, 0.5],
        );
        assert!(apply_lower_triangular_many(&factor, 2, &rows[..5]).is_err());
        assert!(apply_lower_triangular_many(&factor, 2, &[1.0, f64::INFINITY]).is_err());
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
        assert!(matches!(result, Err(crate::Error::Internal(_))));
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
    fn ledoit_wolf_retains_local_offsets_and_mixed_units() {
        let adjacent = ledoit_wolf_shrinkage(&[1e16, 1e16 + 2.0], 2, 1).unwrap();
        assert_eq!(adjacent.covariance, [1.0]);
        let (huge, tiny) = (2.0_f64.powi(500), 2.0_f64.powi(-500));
        let mixed = ledoit_wolf_shrinkage(&[huge, tiny, -huge, -tiny], 2, 2).unwrap();
        assert_eq!(mixed.shrinkage, 0.0);
        assert_eq!(mixed.covariance, [huge * huge, 1.0, 1.0, tiny * tiny]);
    }

    #[test]
    fn ledoit_wolf_retains_small_positive_target_distance() {
        let e = 2.0_f64.powi(-600);
        let got = ledoit_wolf_shrinkage(&[1.0, 0.0, -1.0, 0.0, e, 1.0, -e, -1.0], 4, 2).unwrap();
        assert_eq!(got.shrinkage, 1.0);
        assert_eq!(got.covariance, [0.5, 0.0, 0.0, 0.5]);
    }

    #[test]
    fn rounded_unit_intensity_preserves_the_small_sample_contribution() {
        let b = 8_700_286_382_685_973.0 / 2.0_f64.powi(52);
        let near_cap = ledoit_wolf_shrinkage(&[1.0, 1.0, -1.0, -1.0, b, -b, -b, b], 4, 2).unwrap();
        assert_eq!(near_cap.shrinkage, 1.0);
        assert!((near_cap.covariance[1] / -6.23737371524e-17 - 1.0).abs() < 1e-10);
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

    #[test]
    fn asymmetric_matrices_are_rejected_across_factorizations() {
        use crate::math::linalg::{
            CholeskyError, cholesky_correlation, cholesky_decomposition, symmetric_eigen,
        };
        let asym = vec![1.0, 0.5, 0.4, 1.0];
        assert!(matches!(
            cholesky_decomposition(&asym, 2),
            Err(CholeskyError::NotSymmetric { row: 0, col: 1, .. })
        ));
        assert!(matches!(
            cholesky_correlation(&asym, 2),
            Err(CholeskyError::NotSymmetric { row: 0, col: 1, .. })
        ));
        assert!(matches!(
            symmetric_eigen(&asym, 2),
            Err(CholeskyError::NotSymmetric { row: 0, col: 1, .. })
        ));

        // Up-front negative diagonal check reports the exact offending row.
        let neg_diag = vec![1.0, 0.0, 0.0, -0.25];
        assert!(matches!(
            cholesky_correlation(&neg_diag, 2),
            Err(CholeskyError::NotPositiveDefinite { row: 1, diag }) if (diag - -0.25).abs() < 1e-15
        ));
    }

    #[test]
    fn cholesky_error_kind_separates_validation_and_computation() {
        use crate::ErrorKind;
        use crate::math::linalg::CholeskyError;

        assert_eq!(
            CholeskyError::DimensionMismatch {
                expected: 2,
                actual: 3
            }
            .kind(),
            ErrorKind::Validation
        );
        assert_eq!(
            CholeskyError::NonFiniteInput {
                value: f64::NAN,
                row: 0,
                col: 0
            }
            .kind(),
            ErrorKind::Validation
        );
        assert_eq!(
            CholeskyError::NotSymmetric {
                row: 0,
                col: 1,
                diff: 0.1,
                tolerance: 1e-12
            }
            .kind(),
            ErrorKind::Validation
        );
        assert_eq!(
            CholeskyError::NotPositiveDefinite { diag: -0.1, row: 1 }.kind(),
            ErrorKind::Computation
        );
        assert_eq!(
            CholeskyError::Singular {
                value: 0.0,
                row: 1,
                col: 1,
                threshold: 1e-12
            }
            .kind(),
            ErrorKind::Computation
        );
        assert_eq!(
            CholeskyError::NonFiniteOutput {
                value: f64::INFINITY,
                row: 0,
            }
            .kind(),
            ErrorKind::Computation
        );
    }

    #[test]
    fn correlation_factor_apply_rejects_non_finite_input_without_mutating_output() {
        use crate::math::linalg::{CholeskyError, cholesky_correlation};

        let matrix = [1.0, 0.5, 0.5, 1.0];
        let factor = cholesky_correlation(&matrix, 2).expect("valid factor");
        let mut correlated = [42.0, -7.0];
        let err = factor
            .apply(&[1.0, f64::NAN], &mut correlated)
            .expect_err("non-finite independent sample must fail");
        assert!(matches!(err, CholeskyError::NonFiniteInput { .. }));
        assert_eq!(correlated, [42.0, -7.0]);
    }

    #[test]
    fn correlation_factor_apply_rejects_non_finite_output() {
        use crate::math::linalg::{CholeskyError, cholesky_correlation};

        let matrix = [1.0, 0.5, 0.5, 1.0];
        let factor = cholesky_correlation(&matrix, 2).expect("valid factor");
        let mut correlated = [0.0; 2];
        let err = factor
            .apply(&[f64::MAX, f64::MAX], &mut correlated)
            .expect_err("overflowing product must fail");
        assert!(matches!(err, CholeskyError::NonFiniteOutput { .. }));
    }

    #[test]
    fn apply_lower_triangular_rejects_non_finite_inputs_and_overflow() {
        use crate::math::linalg::apply_lower_triangular;

        let valid_l = [1.0, 0.0, 0.5, 1.0];
        assert!(matches!(
            apply_lower_triangular(&valid_l, 2, &[1.0, f64::INFINITY]),
            Err(crate::Error::Input(
                crate::error::InputError::NonFiniteValue { .. }
            ))
        ));
        assert!(matches!(
            apply_lower_triangular(&[1.0, 0.0, f64::NAN, 1.0], 2, &[1.0, 1.0]),
            Err(crate::Error::Input(
                crate::error::InputError::NonFiniteValue { .. }
            ))
        ));
        assert!(matches!(
            apply_lower_triangular(&valid_l, 2, &[f64::MAX, f64::MAX]),
            Err(crate::Error::Internal(_))
        ));
    }

    #[test]
    fn validate_correlation_matrix_enforces_exact_unit_bounds() {
        use crate::math::linalg::{CorrelationError, validate_correlation_matrix};

        let over = [1.0, 1.0 + 1e-13, 1.0 + 1e-13, 1.0];
        assert!(matches!(
            validate_correlation_matrix(&over, 2),
            Err(CorrelationError::OutOfBounds { i: 0, j: 1, .. })
        ));
        let under = [1.0, -1.0 - 1e-13, -1.0 - 1e-13, 1.0];
        assert!(matches!(
            validate_correlation_matrix(&under, 2),
            Err(CorrelationError::OutOfBounds { i: 0, j: 1, .. })
        ));
    }
}
