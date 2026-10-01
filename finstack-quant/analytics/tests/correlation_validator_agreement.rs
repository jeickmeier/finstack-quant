//! Pins that analytics' detailed correlation diagnostics and core's coarse
//! validator agree.
//!
//! The two used to disagree in *both* directions: analytics applied a 1e-10
//! diagonal/symmetry tolerance against core's 1e-6, while core enforced a
//! strict `[-1, 1]` bound against analytics' 1e-10 slack. A caller could get
//! opposite answers from `core.math.linalg.validate_correlation_matrix` and
//! `valuations.correlation.validate_correlation_matrix` on the same matrix.
//!
//! Core owns the canonical detailed validation and maps those diagnostics onto
//! its existing coarse `InputError`. Analytics re-exports the detailed error
//! type and delegates validation to core.

use finstack_quant_analytics::correlation::{
    validate_correlation_matrix as analytics_validate, Error,
};
use finstack_quant_core::math::linalg::{
    check_correlation_matrix as core_validate,
    validate_correlation_matrix as core_validate_detailed, CorrelationError,
};
use serde_json::json;

fn assert_detailed_rejection(matrix: &[f64], n: usize, expected: impl FnOnce(&Error) -> bool) {
    let analytics_error = analytics_validate(matrix, n).expect_err("analytics should reject input");
    assert!(
        expected(&analytics_error),
        "unexpected analytics error: {analytics_error:?}"
    );

    let core_error = core_validate_detailed(matrix, n).expect_err("core should reject input");
    assert_eq!(analytics_error, core_error);
    assert!(core_validate(matrix, n).is_err());
}

/// Cases spanning both former divergence directions plus the boundaries.
fn agreement_cases() -> Vec<(&'static str, Vec<f64>, usize, bool)> {
    vec![
        ("clean 2x2", vec![1.0, 0.5, 0.5, 1.0], 2, true),
        // Formerly: passed core (1e-6), failed analytics (1e-10).
        (
            "diagonal noise 1e-8",
            vec![1.0 + 1e-8, 0.5, 0.5, 1.0],
            2,
            false,
        ),
        // Formerly: passed analytics (1e-10 slack), failed core (strict bound).
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
fn analytics_and_core_validators_agree() {
    for (label, matrix, n, expect_ok) in agreement_cases() {
        let core_ok = core_validate(&matrix, n).is_ok();
        let analytics_ok = analytics_validate(&matrix, n).is_ok();
        assert_eq!(
            core_ok, analytics_ok,
            "validators disagree on '{label}': core_ok={core_ok}, analytics_ok={analytics_ok}"
        );
        assert_eq!(
            core_ok, expect_ok,
            "'{label}' should validate as {expect_ok}, got {core_ok}"
        );
    }
}

#[test]
fn diagonal_noise_above_tolerance_is_rejected_by_both() {
    // 1e-8 sits above the shared 1e-10 diagonal tolerance; it signals a
    // mis-normalized covariance matrix rather than rounding.
    let m = vec![1.0 + 1e-8, 0.5, 0.5, 1.0];
    assert!(core_validate(&m, 2).is_err());
    assert!(analytics_validate(&m, 2).is_err());
}

#[test]
fn rounding_scale_overshoot_is_accepted_by_both() {
    // A perfectly collinear pair (duplicated series, or genuinely identical
    // instruments) computes to a few ulp above 1. Rejecting it would reject a
    // valid input, so both validators allow rounding-scale slack.
    let over = 1.0 + f64::EPSILON;
    let m = vec![1.0, over, over, 1.0];
    assert!(core_validate(&m, 2).is_ok());
    assert!(analytics_validate(&m, 2).is_ok());
}

#[test]
fn diagonal_within_tolerance_is_accepted_by_both() {
    // This exceeds the tighter off-diagonal coefficient-bound slack. Diagonal
    // entries must still be governed solely by DIAGONAL_TOLERANCE.
    let matrix = [1.0 + 5e-11];
    assert!(core_validate(&matrix, 1).is_ok());
    assert!(analytics_validate(&matrix, 1).is_ok());
}

#[test]
fn diagonal_nan_reports_out_of_bounds_from_both_detailed_validators() {
    let matrix = [f64::NAN];
    let analytics_error = analytics_validate(&matrix, 1).expect_err("analytics should reject NaN");
    let core_error = core_validate_detailed(&matrix, 1).expect_err("core should reject NaN");

    for error in [&analytics_error, &core_error] {
        assert!(matches!(
            error,
            Error::OutOfBounds { i: 0, j: 0, value } if value.is_nan()
        ));
    }
    assert!(core_validate(&matrix, 1).is_err());
}

#[test]
fn wrong_size_reports_detailed_error_and_coarse_core_rejection() {
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
fn overflowing_dimensions_report_errors_from_all_validators() {
    for n in [usize::MAX, 1usize << (usize::BITS / 2)] {
        assert_detailed_rejection(
            &[],
            n,
            |error| matches!(error, Error::InvalidSize { expected, actual: 0 } if *expected == n),
        );
        assert!(analytics_validate(&[], n)
            .expect_err("overflowing dimensions must be invalid")
            .to_string()
            .contains("Invalid matrix size"));
    }
}

#[test]
fn invalid_diagonal_reports_detailed_error_and_coarse_core_rejection() {
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
fn asymmetry_reports_detailed_error_and_coarse_core_rejection() {
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
fn out_of_bounds_reports_detailed_error_and_coarse_core_rejection() {
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
fn non_psd_reports_detailed_error_and_coarse_core_rejection() {
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
fn analytics_error_is_core_correlation_error_reexport() {
    let error = analytics_validate(&[1.0, 0.5, 0.5], 2).expect_err("wrong size should fail");
    let _: CorrelationError = error;
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
