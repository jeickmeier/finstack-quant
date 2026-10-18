//! Pins the detailed correlation-matrix diagnostics of
//! `core::math::linalg::validate_correlation_matrix`, its tolerance
//! boundaries, the `CorrelationError` serde shape, and the Higham
//! `nearest_correlation` repair.
//!
//! The validator used to exist twice with opposite tolerances (a 1e-10
//! diagonal/symmetry tolerance against 1e-6, a strict `[-1, 1]` bound against
//! 1e-10 slack); the cases below span both former divergence directions.

use finstack_quant_core::math::linalg::{
    nearest_correlation, validate_correlation_matrix as validate, CorrelationError as Error,
    NearestCorrelationOpts,
};
use serde_json::json;

fn assert_detailed_rejection(matrix: &[f64], n: usize, expected: impl FnOnce(&Error) -> bool) {
    let error = validate(matrix, n).expect_err("validator should reject input");
    assert!(expected(&error), "unexpected error: {error:?}");
}

/// Cases spanning both former divergence directions plus the boundaries.
fn boundary_cases() -> Vec<(&'static str, Vec<f64>, usize, bool)> {
    vec![
        ("clean 2x2", vec![1.0, 0.5, 0.5, 1.0], 2, true),
        // Formerly passed the 1e-6 validator and failed the 1e-10 one.
        (
            "diagonal noise 1e-8",
            vec![1.0 + 1e-8, 0.5, 0.5, 1.0],
            2,
            false,
        ),
        // Formerly passed the 1e-10-slack validator and failed the strict-bound one.
        (
            "off-diagonal at 1 + 1e-10",
            vec![1.0, 1.0 + 1e-10, 1.0 + 1e-10, 1.0],
            2,
            false,
        ),
        // Genuinely out of range.
        ("off-diagonal at 1.5", vec![1.0, 1.5, 1.5, 1.0], 2, false),
        // Asymmetry well above noise scale.
        (
            "asymmetric by 1e-8",
            vec![1.0, 0.5, 0.5 + 1e-8, 1.0],
            2,
            false,
        ),
        // Asymmetry at rounding scale is tolerated.
        (
            "asymmetric by 1e-16",
            vec![1.0, 0.5, 0.5 + 1e-16, 1.0],
            2,
            true,
        ),
    ]
}

#[test]
fn validator_tolerance_boundaries() {
    for (label, matrix, n, expect_ok) in boundary_cases() {
        let ok = validate(&matrix, n).is_ok();
        assert_eq!(
            ok, expect_ok,
            "'{label}' should validate as {expect_ok}, got {ok}"
        );
    }
}

#[test]
fn diagonal_noise_above_tolerance_is_rejected() {
    // 1e-8 sits above the shared 1e-10 diagonal tolerance; it signals a
    // mis-normalized covariance matrix rather than rounding.
    let m = vec![1.0 + 1e-8, 0.5, 0.5, 1.0];
    assert!(validate(&m, 2).is_err());
}

#[test]
fn rounding_scale_overshoot_is_accepted() {
    // A perfectly collinear pair (duplicated series, or genuinely identical
    // instruments) computes to a few ulp above 1. Rejecting it would reject a
    // valid input, so both validators allow rounding-scale slack.
    let over = 1.0 + f64::EPSILON;
    let m = vec![1.0, over, over, 1.0];
    assert!(validate(&m, 2).is_ok());
}

#[test]
fn diagonal_within_tolerance_is_accepted() {
    // This exceeds the tighter off-diagonal coefficient-bound slack. Diagonal
    // entries must still be governed solely by DIAGONAL_TOLERANCE.
    let matrix = [1.0 + 5e-11];
    assert!(validate(&matrix, 1).is_ok());
}

#[test]
fn diagonal_nan_reports_out_of_bounds() {
    let error = validate(&[f64::NAN], 1).expect_err("NaN should be rejected");
    assert!(matches!(
        error,
        Error::OutOfBounds { i: 0, j: 0, value } if value.is_nan()
    ));
}

#[test]
fn wrong_size_reports_detailed_error() {
    assert_detailed_rejection(&[1.0, 0.5, 0.5], 2, |error| {
        matches!(
            error,
            Error::InvalidSize {
                expected: 2,
                actual: 3
            }
        )
    });
}

#[test]
fn overflowing_dimensions_report_invalid_size() {
    for n in [usize::MAX, 1usize << (usize::BITS / 2)] {
        assert_detailed_rejection(
            &[],
            n,
            |error| matches!(error, Error::InvalidSize { expected, actual: 0 } if *expected == n),
        );
        assert!(validate(&[], n)
            .expect_err("overflowing dimensions must be invalid")
            .to_string()
            .contains("Invalid matrix size"));
    }
}

#[test]
fn invalid_diagonal_reports_detailed_error() {
    assert_detailed_rejection(&[0.9, 0.5, 0.5, 1.0], 2, |error| {
        matches!(
            error,
            Error::DiagonalNotOne {
                index: 0,
                value: 0.9
            }
        )
    });
}

#[test]
fn asymmetry_reports_detailed_error() {
    assert_detailed_rejection(&[1.0, 0.5, 0.3, 1.0], 2, |error| {
        matches!(
            error,
            Error::NotSymmetric {
                i: 0,
                j: 1,
                diff
            } if (*diff - 0.2).abs() < f64::EPSILON
        )
    });
}

#[test]
fn out_of_bounds_reports_detailed_error() {
    assert_detailed_rejection(&[1.0, 1.5, 1.5, 1.0], 2, |error| {
        matches!(
            error,
            Error::OutOfBounds {
                i: 0,
                j: 1,
                value: 1.5
            }
        )
    });
}

#[test]
fn non_psd_reports_detailed_error() {
    let matrix = [
        1.0, -0.75, -0.75, //
        -0.75, 1.0, -0.75, //
        -0.75, -0.75, 1.0,
    ];
    assert_detailed_rejection(&matrix, 3, |error| {
        matches!(error, Error::NotPositiveSemiDefinite { .. })
    });
}

#[test]
fn correlation_error_serde_shape_is_stable() {
    let cases = [
        (
            Error::InvalidSize {
                expected: 2,
                actual: 3,
            },
            json!({"invalid_size": {"expected": 2, "actual": 3}}),
        ),
        (
            Error::DiagonalNotOne {
                index: 1,
                value: 0.9,
            },
            json!({"diagonal_not_one": {"index": 1, "value": 0.9}}),
        ),
        (
            Error::NotSymmetric {
                i: 0,
                j: 1,
                diff: 0.2,
            },
            json!({"not_symmetric": {"i": 0, "j": 1, "diff": 0.2}}),
        ),
        (
            Error::NotPositiveSemiDefinite { row: 2 },
            json!({"not_positive_semi_definite": {"row": 2}}),
        ),
        (
            Error::OutOfBounds {
                i: 0,
                j: 1,
                value: 1.5,
            },
            json!({"out_of_bounds": {"i": 0, "j": 1, "value": 1.5}}),
        ),
        (
            Error::DidNotConverge {
                max_iter: 100,
                tol: 1e-8,
            },
            json!({"did_not_converge": {"max_iter": 100, "tol": 1e-8}}),
        ),
        (
            Error::EigenDecompositionFailed,
            json!("eigen_decomposition_failed"),
        ),
    ];

    for (error, expected) in cases {
        assert_eq!(
            serde_json::to_value(&error).expect("error should serialize"),
            expected
        );
        assert_eq!(
            serde_json::from_value::<Error>(expected).expect("error should deserialize"),
            error
        );
    }
}

#[test]
fn nearest_correlation_preserves_already_feasible_off_diagonal() {
    let output = nearest_correlation(
        &[1.0005, 0.5, 0.5, 1.0005],
        2,
        NearestCorrelationOpts::default(),
    )
    .expect("near-feasible matrix should repair");
    assert_eq!(output, vec![1.0, 0.5, 0.5, 1.0]);
}
