//! Interpolation tests.
//!
//! This module consolidates all interpolation tests including:
//! - Basic construction and value tests (via macros)
//! - Derivative/interp_prime tests
//! - Extrapolation policy tests
//! - Type-specific tests
//! - Trait tests
//! - Serialization tests

use super::common::{approx_eq, standard_dfs, standard_knots, two_point_dfs, two_point_knots};
use finstack_core::math::interp::*;

// Helper to create interpolator with default Strict validation policy
macro_rules! new_strict {
    ($ty:ty, $knots:expr, $dfs:expr, $extrap:expr) => {
        <$ty>::new($knots, $dfs, $extrap, ValidationPolicy::Strict)
    };
}

// ============================================================================
// Basic Tests (macro-generated for all interpolator types)
// ============================================================================

macro_rules! interp_basic_tests {
    ($mod_name:ident, $ty:ty) => {
        mod $mod_name {
            use super::*;

            #[test]
            fn construction_succeeds() {
                let result = new_strict!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::default()
                );
                assert!(result.is_ok(), "Construction should succeed");
            }

            #[test]
            fn exact_knot_values_preserved() {
                let interp = new_strict!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::default()
                )
                .unwrap();

                assert!(approx_eq(interp.interp(0.0), 1.0, 1e-12));
                assert!(approx_eq(interp.interp(1.0), 0.95, 1e-12));
                assert!(approx_eq(interp.interp(2.0), 0.9, 1e-12));
                assert!(approx_eq(interp.interp(3.0), 0.85, 1e-12));
            }

            #[test]
            fn interpolation_between_knots() {
                let interp = new_strict!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::default()
                )
                .unwrap();

                let mid = interp.interp(0.5);
                assert!(
                    mid > 0.95 && mid < 1.0,
                    "0.5 value {} not in (0.95, 1.0)",
                    mid
                );

                let mid2 = interp.interp(1.5);
                assert!(
                    mid2 > 0.9 && mid2 < 0.95,
                    "1.5 value {} not in (0.9, 0.95)",
                    mid2
                );
            }

            #[test]
            fn rejects_non_increasing_knots() {
                let bad_knots = vec![1.0, 0.0, 2.0, 3.0].into_boxed_slice();
                let result = new_strict!(
                    $ty,
                    bad_knots,
                    standard_dfs(),
                    ExtrapolationPolicy::default()
                );
                assert!(result.is_err(), "Should reject non-increasing knots");
            }

            #[test]
            fn rejects_non_positive_dfs() {
                let bad_dfs = vec![1.0, -0.95, 0.9, 0.85].into_boxed_slice();
                let result = new_strict!(
                    $ty,
                    standard_knots(),
                    bad_dfs,
                    ExtrapolationPolicy::default()
                );
                assert!(result.is_err(), "Should reject non-positive DFs");
            }

            #[test]
            fn two_point_case() {
                let interp = new_strict!(
                    $ty,
                    two_point_knots(),
                    two_point_dfs(),
                    ExtrapolationPolicy::default()
                )
                .unwrap();

                let mid = interp.interp(0.5);
                assert!(
                    mid > 0.95 && mid <= 1.0,
                    "Two-point midpoint {} invalid",
                    mid
                );
                assert!(mid.is_finite());
            }

            #[test]
            fn edge_values_are_finite() {
                let interp = new_strict!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::default()
                )
                .unwrap();

                assert!(interp.interp(0.001).is_finite());
                assert!(interp.interp(2.999).is_finite());
                assert!(interp.interp(0.001) > 0.0);
                assert!(interp.interp(2.999) > 0.0);
            }

            #[test]
            fn rejects_empty_input() {
                let result = new_strict!(
                    $ty,
                    vec![].into_boxed_slice(),
                    vec![].into_boxed_slice(),
                    ExtrapolationPolicy::default()
                );
                assert!(result.is_err(), "Should reject empty input");
            }

            #[test]
            fn rejects_mismatched_lengths() {
                let result = new_strict!(
                    $ty,
                    vec![0.0, 1.0, 2.0].into_boxed_slice(),
                    vec![1.0, 0.95].into_boxed_slice(),
                    ExtrapolationPolicy::default()
                );
                assert!(result.is_err(), "Should reject mismatched lengths");
            }

            #[test]
            fn single_point_rejected() {
                let result = new_strict!(
                    $ty,
                    vec![1.0].into_boxed_slice(),
                    vec![0.95].into_boxed_slice(),
                    ExtrapolationPolicy::default()
                );
                assert!(
                    result.is_err(),
                    "Interpolators require at least two points under Strict validation"
                );
            }
        }
    };
}

interp_basic_tests!(linear_df_basic, Interpolator<LinearStrategy>);
interp_basic_tests!(log_linear_df_basic, Interpolator<LogLinearStrategy>);
interp_basic_tests!(flat_fwd_basic, Interpolator<LogLinearStrategy>);
interp_basic_tests!(cubic_hermite_basic, Interpolator<CubicHermiteStrategy>);
interp_basic_tests!(monotone_convex_basic, Interpolator<MonotoneConvexStrategy>);
interp_basic_tests!(
    piecewise_quadratic_forward_basic,
    Interpolator<PiecewiseQuadraticForwardStrategy>
);

// ============================================================================
// InterpStyle::build Tests
// ============================================================================

mod interp_style_build {
    use super::*;

    #[test]
    fn build_linear() {
        let interp = InterpStyle::Linear
            .build(
                standard_knots(),
                standard_dfs(),
                ExtrapolationPolicy::FlatZero,
                ValidationPolicy::Strict,
            )
            .expect("Should build linear interpolator");

        assert!(approx_eq(interp.interp(0.0), 1.0, 1e-12));
        assert!(approx_eq(interp.interp(1.0), 0.95, 1e-12));
        assert!(approx_eq(interp.interp(2.0), 0.9, 1e-12));
        assert!(approx_eq(interp.interp(3.0), 0.85, 1e-12));
    }

    #[test]
    fn build_log_linear() {
        let interp = InterpStyle::LogLinear
            .build(
                standard_knots(),
                standard_dfs(),
                ExtrapolationPolicy::FlatZero,
                ValidationPolicy::Strict,
            )
            .expect("Should build log-linear interpolator");

        assert!(approx_eq(interp.interp(0.0), 1.0, 1e-12));
        assert!(approx_eq(interp.interp(1.0), 0.95, 1e-12));
    }

    #[test]
    fn build_monotone_convex() {
        let interp = InterpStyle::MonotoneConvex
            .build(
                standard_knots(),
                standard_dfs(),
                ExtrapolationPolicy::FlatZero,
                ValidationPolicy::Strict,
            )
            .expect("Should build monotone convex interpolator");

        assert!(approx_eq(interp.interp(0.0), 1.0, 1e-12));
        assert!(approx_eq(interp.interp(1.0), 0.95, 1e-12));
    }

    #[test]
    fn build_cubic_hermite() {
        let interp = InterpStyle::CubicHermite
            .build(
                standard_knots(),
                standard_dfs(),
                ExtrapolationPolicy::FlatZero,
                ValidationPolicy::Strict,
            )
            .expect("Should build cubic hermite interpolator");

        assert!(approx_eq(interp.interp(0.0), 1.0, 1e-12));
        assert!(approx_eq(interp.interp(1.0), 0.95, 1e-12));
    }

    #[test]
    fn build_piecewise_quadratic_forward() {
        let interp = InterpStyle::PiecewiseQuadraticForward
            .build(
                standard_knots(),
                standard_dfs(),
                ExtrapolationPolicy::FlatForward,
                ValidationPolicy::Strict,
            )
            .expect("Should build piecewise quadratic forward interpolator");

        assert!(approx_eq(interp.interp(0.0), 1.0, 1e-12));
        assert!(approx_eq(interp.interp(1.0), 0.95, 1e-12));
    }

    #[test]
    fn build_flat_fwd() {
        let interp = InterpStyle::LogLinear
            .build(
                standard_knots(),
                standard_dfs(),
                ExtrapolationPolicy::FlatZero,
                ValidationPolicy::Strict,
            )
            .expect("Should build flat forward interpolator");

        assert!(approx_eq(interp.interp(0.0), 1.0, 1e-12));
        assert!(approx_eq(interp.interp(1.0), 0.95, 1e-12));
    }

    #[test]
    fn build_all_with_flat_forward_extrapolation() {
        for style in [
            InterpStyle::Linear,
            InterpStyle::LogLinear,
            InterpStyle::MonotoneConvex,
            InterpStyle::CubicHermite,
            InterpStyle::LogLinear,
        ] {
            let interp = style
                .build(
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::FlatForward,
                    ValidationPolicy::Strict,
                )
                .unwrap_or_else(|e| panic!("Should build {:?}: {:?}", style, e));

            assert!(approx_eq(interp.interp(0.0), 1.0, 1e-12));
            assert!(approx_eq(interp.interp(3.0), 0.85, 1e-12));
        }
    }
}

// ============================================================================
// Extrapolation Policy Tests
// ============================================================================

macro_rules! extrapolation_tests {
    ($mod_name:ident, $ty:ty) => {
        mod $mod_name {
            use super::*;

            #[test]
            fn flat_zero_extrapolation_left() {
                let interp = new_strict!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::FlatZero
                )
                .unwrap();

                let left_extrap = interp.interp(-1.0);
                assert!(
                    approx_eq(left_extrap, 1.0, 1e-10),
                    "FlatZero left extrapolation should return first value, got {}",
                    left_extrap
                );
            }

            #[test]
            fn flat_zero_extrapolation_right() {
                let interp = new_strict!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::FlatZero
                )
                .unwrap();

                let right_extrap = interp.interp(5.0);
                assert!(
                    approx_eq(right_extrap, 0.85, 1e-10),
                    "FlatZero right extrapolation should return last value, got {}",
                    right_extrap
                );
            }

            #[test]
            fn flat_forward_extrapolation_left() {
                let interp = new_strict!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::FlatForward
                )
                .unwrap();

                let left_extrap = interp.interp(-0.5);
                assert!(
                    left_extrap > 1.0 && left_extrap.is_finite(),
                    "FlatForward left extrapolation {} should be > 1.0",
                    left_extrap
                );
            }

            #[test]
            fn flat_forward_extrapolation_right() {
                let interp = new_strict!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::FlatForward
                )
                .unwrap();

                let right_extrap = interp.interp(4.0);
                assert!(
                    right_extrap < 0.85 && right_extrap > 0.0 && right_extrap.is_finite(),
                    "FlatForward right extrapolation {} should be < 0.85 and > 0",
                    right_extrap
                );
            }

            #[test]
            fn extrapolation_far_left() {
                let interp = new_strict!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::FlatZero
                )
                .unwrap();

                let far_left = interp.interp(-100.0);
                assert!(far_left.is_finite());
                assert!(approx_eq(far_left, 1.0, 1e-10));
            }

            #[test]
            fn extrapolation_far_right() {
                let interp = new_strict!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::FlatZero
                )
                .unwrap();

                let far_right = interp.interp(100.0);
                assert!(far_right.is_finite());
                assert!(approx_eq(far_right, 0.85, 1e-10));
            }
        }
    };
}

extrapolation_tests!(extrap_linear, Interpolator<LinearStrategy>);
extrapolation_tests!(extrap_log_linear, Interpolator<LogLinearStrategy>);
extrapolation_tests!(extrap_flat_fwd, Interpolator<LogLinearStrategy>);
extrapolation_tests!(extrap_cubic_hermite, Interpolator<CubicHermiteStrategy>);
extrapolation_tests!(extrap_monotone_convex, Interpolator<MonotoneConvexStrategy>);

// ============================================================================
// Derivative (interp_prime) Tests
// ============================================================================

macro_rules! derivative_tests {
    ($mod_name:ident, $ty:ty) => {
        mod $mod_name {
            use super::*;

            #[test]
            fn derivative_is_finite_interior() {
                let interp = new_strict!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::FlatZero
                )
                .unwrap();

                for &x in &[0.5, 1.0, 1.5, 2.0, 2.5] {
                    let deriv = interp.interp_prime(x);
                    assert!(
                        deriv.is_finite(),
                        "Derivative at {} should be finite, got {}",
                        x,
                        deriv
                    );
                }
            }

            #[test]
            fn derivative_is_negative_for_decreasing_curve() {
                let interp = new_strict!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::FlatZero
                )
                .unwrap();

                for &x in &[0.5, 1.5, 2.5] {
                    let deriv = interp.interp_prime(x);
                    assert!(
                        deriv < 0.0,
                        "Derivative at {} should be negative for decreasing curve, got {}",
                        x,
                        deriv
                    );
                }
            }

            #[test]
            fn derivative_numerical_check() {
                let interp = new_strict!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::FlatZero
                )
                .unwrap();

                let h = 1e-6;
                for &x in &[0.5, 1.5, 2.5] {
                    let numerical = (interp.interp(x + h) - interp.interp(x - h)) / (2.0 * h);
                    let analytical = interp.interp_prime(x);

                    let error = (analytical - numerical).abs();
                    assert!(
                        error < 1e-5,
                        "Derivative mismatch at {}: analytical={}, numerical={}, error={}",
                        x,
                        analytical,
                        numerical,
                        error
                    );
                }
            }

            #[test]
            fn derivative_at_knots() {
                let interp = new_strict!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::FlatZero
                )
                .unwrap();

                for &x in &[0.0, 1.0, 2.0, 3.0] {
                    let deriv = interp.interp_prime(x);
                    assert!(
                        deriv.is_finite(),
                        "Derivative at knot {} should be finite",
                        x
                    );
                }
            }

            #[test]
            fn derivative_flat_zero_extrapolation() {
                let interp = new_strict!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::FlatZero
                )
                .unwrap();

                let left_deriv = interp.interp_prime(-1.0);
                let right_deriv = interp.interp_prime(5.0);

                assert!(
                    approx_eq(left_deriv, 0.0, 1e-10),
                    "FlatZero left derivative should be 0, got {}",
                    left_deriv
                );
                assert!(
                    approx_eq(right_deriv, 0.0, 1e-10),
                    "FlatZero right derivative should be 0, got {}",
                    right_deriv
                );
            }

            #[test]
            fn derivative_flat_forward_extrapolation() {
                let interp = new_strict!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::FlatForward
                )
                .unwrap();

                let left_deriv = interp.interp_prime(-1.0);
                let right_deriv = interp.interp_prime(5.0);

                assert!(
                    left_deriv.is_finite() && left_deriv != 0.0,
                    "FlatForward left derivative should be non-zero, got {}",
                    left_deriv
                );
                assert!(
                    right_deriv.is_finite() && right_deriv != 0.0,
                    "FlatForward right derivative should be non-zero, got {}",
                    right_deriv
                );
            }
        }
    };
}

derivative_tests!(deriv_linear, Interpolator<LinearStrategy>);
derivative_tests!(deriv_log_linear, Interpolator<LogLinearStrategy>);
derivative_tests!(deriv_flat_fwd, Interpolator<LogLinearStrategy>);
derivative_tests!(deriv_cubic_hermite, Interpolator<CubicHermiteStrategy>);
derivative_tests!(deriv_monotone_convex, Interpolator<MonotoneConvexStrategy>);

// ============================================================================
// Type-Specific Tests
// ============================================================================

mod linear_specific {
    use super::*;

    #[test]
    fn midpoint_matches_manual_formula() {
        let knots: Box<[f64]> = (0..=4)
            .map(|i| i as f64)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let dfs: Box<[f64]> = knots
            .iter()
            .map(|&t| (-0.02f64 * t).exp())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let interp = new_strict!(
            Interpolator<LinearStrategy>,
            knots.clone(),
            dfs.clone(),
            ExtrapolationPolicy::default()
        )
        .unwrap();

        for seg in 0..knots.len() - 1 {
            let t_mid = 0.5 * (knots[seg] + knots[seg + 1]);
            let expected = 0.5 * (dfs[seg] + dfs[seg + 1]);
            assert!((interp.interp(t_mid) - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn derivative_at_segment_boundaries() {
        let interp = new_strict!(
            Interpolator<LinearStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::FlatZero
        )
        .unwrap();

        let h = 1e-8;
        for &x in &[1.0, 2.0] {
            let deriv = interp.interp_prime(x);
            let numerical = (interp.interp(x + h) - interp.interp(x - h)) / (2.0 * h);
            assert!(
                approx_eq(deriv, numerical, 1e-4),
                "Derivative at {} should match numerical approximation",
                x
            );
        }
    }
}

mod log_linear_specific {
    use super::*;

    #[test]
    fn geometric_midpoint() {
        let knots: Vec<f64> = vec![0.0, 1.0, 2.0, 3.0];
        let zero_rate = 0.03f64;
        let dfs: Vec<f64> = knots.iter().map(|&t| (-zero_rate * t).exp()).collect();
        let knots = knots.into_boxed_slice();
        let dfs = dfs.into_boxed_slice();

        let interp = new_strict!(
            Interpolator<LogLinearStrategy>,
            knots.clone(),
            dfs.clone(),
            ExtrapolationPolicy::default()
        )
        .unwrap();
        for seg in 0..knots.len() - 1 {
            let t_mid = 0.5 * (knots[seg] + knots[seg + 1]);
            let expected = (dfs[seg].ln() * 0.5 + dfs[seg + 1].ln() * 0.5).exp();
            assert!((interp.interp(t_mid) - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn derivative_formula_consistency() {
        let interp = new_strict!(
            Interpolator<LogLinearStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::FlatZero
        )
        .unwrap();

        let h = 1e-7;
        for &x in &[0.5, 1.5, 2.5] {
            let analytical = interp.interp_prime(x);
            let numerical = (interp.interp(x + h) - interp.interp(x - h)) / (2.0 * h);

            let rel_error = (analytical - numerical).abs() / numerical.abs();
            assert!(
                rel_error < 1e-5,
                "Log-linear derivative at {}: analytical={}, numerical={}, rel_error={}",
                x,
                analytical,
                numerical,
                rel_error
            );
        }
    }
}

mod flat_fwd_specific {
    use super::*;

    #[test]
    fn matches_log_linear_exactly() {
        let flat = new_strict!(
            Interpolator<LogLinearStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::default()
        )
        .unwrap();
        let log = new_strict!(
            Interpolator<LogLinearStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::default()
        )
        .unwrap();

        for t in [0.1, 0.25, 0.5, 1.5, 2.5] {
            assert!(
                approx_eq(flat.interp(t), log.interp(t), 1e-15),
                "LogLinear != LogLinear at t={}",
                t
            );
        }
    }

    #[test]
    fn constant_forward_rate_property() {
        let interp = new_strict!(
            Interpolator<LogLinearStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::default()
        )
        .unwrap();

        let t1 = 0.25;
        let t2 = 0.75;
        let df1 = interp.interp(t1);
        let df2 = interp.interp(t2);

        let implied_rate = -(df2.ln() - df1.ln()) / (t2 - t1);

        let t3 = 0.1;
        let t4 = 0.9;
        let df3 = interp.interp(t3);
        let df4 = interp.interp(t4);
        let implied_rate2 = -(df4.ln() - df3.ln()) / (t4 - t3);

        assert!(approx_eq(implied_rate, implied_rate2, 1e-10));
    }
}

mod cubic_hermite_specific {
    use super::*;

    #[test]
    fn derivative_numerical_consistency() {
        let interp = new_strict!(
            Interpolator<CubicHermiteStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::default()
        )
        .unwrap();

        let h = 6e-6;
        let x = 1.5;
        let numerical = (interp.interp(x + h) - interp.interp(x - h)) / (2.0 * h);
        let analytical = interp.interp_prime(x);

        let relative_error = (analytical - numerical).abs() / numerical.abs();
        assert!(
            relative_error < 1e-8,
            "Derivative error {} too large",
            relative_error
        );
    }

    #[test]
    fn derivative_monotonicity_preserved() {
        let interp = new_strict!(
            Interpolator<CubicHermiteStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::default()
        )
        .unwrap();

        for &x in &[0.5, 1.5, 2.5] {
            let deriv = interp.interp_prime(x);
            assert!(
                deriv < 0.0,
                "Derivative at {} should be negative, got {}",
                x,
                deriv
            );
            assert!(deriv.is_finite());
        }
    }

    #[test]
    fn derivative_at_knots_returns_precomputed_slopes() {
        let interp = new_strict!(
            Interpolator<CubicHermiteStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::default()
        )
        .unwrap();

        let derivative_at_knots = vec![
            interp.interp_prime(0.0),
            interp.interp_prime(1.0),
            interp.interp_prime(2.0),
            interp.interp_prime(3.0),
        ];

        for &deriv in &derivative_at_knots {
            assert!(deriv.is_finite());
            assert!(deriv <= 0.0);
        }
    }

    #[test]
    fn extrapolation_uses_boundary_slopes() {
        let interp = new_strict!(
            Interpolator<CubicHermiteStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::FlatForward
        )
        .unwrap();

        let left_deriv = interp.interp_prime(-1.0);
        let first_deriv = interp.interp_prime(0.0);
        assert!(
            approx_eq(left_deriv, first_deriv, 1e-10),
            "Left extrapolation slope should match boundary slope"
        );

        let right_deriv = interp.interp_prime(5.0);
        let last_deriv = interp.interp_prime(3.0);
        assert!(
            approx_eq(right_deriv, last_deriv, 1e-10),
            "Right extrapolation slope should match boundary slope"
        );
    }

    #[test]
    fn derivative_continuity_across_knots() {
        let interp = new_strict!(
            Interpolator<CubicHermiteStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::FlatZero
        )
        .unwrap();

        let h = 1e-8;
        for &x in &[1.0, 2.0] {
            let left_deriv = interp.interp_prime(x - h);
            let right_deriv = interp.interp_prime(x + h);
            let knot_deriv = interp.interp_prime(x);

            assert!(
                approx_eq(left_deriv, knot_deriv, 1e-4),
                "Left derivative at {} differs from knot derivative",
                x
            );
            assert!(
                approx_eq(right_deriv, knot_deriv, 1e-4),
                "Right derivative at {} differs from knot derivative",
                x
            );
        }
    }
}

mod monotone_convex_specific {
    use super::*;

    /// Increasing DFs (negative forward rates) are accepted: MonotoneConvex
    /// must support negative-rate curves
    /// (EUR/CHF/JPY) where DF > 1 is legitimate.
    #[test]
    fn accepts_increasing_dfs_negative_rates() {
        let increasing_dfs = vec![0.9, 0.95, 1.0, 1.05].into_boxed_slice();
        let interp = new_strict!(
            Interpolator<MonotoneConvexStrategy>,
            standard_knots(),
            increasing_dfs,
            ExtrapolationPolicy::default()
        )
        .expect("increasing DFs (negative rates) should build");
        let mid = interp.interp(0.5);
        assert!(mid.is_finite() && mid > 0.0);
    }

    #[test]
    fn near_flat_curve_handled() {
        let near_flat = vec![1.0, 0.999999, 0.999998, 0.999997].into_boxed_slice();
        let interp = new_strict!(
            Interpolator<MonotoneConvexStrategy>,
            standard_knots(),
            near_flat,
            ExtrapolationPolicy::default()
        )
        .unwrap();

        let mid = interp.interp(0.5);
        assert!(mid.is_finite());
        assert!(mid > 0.999997 && mid <= 1.0);
    }

    #[test]
    fn shape_preservation() {
        let interp = new_strict!(
            Interpolator<MonotoneConvexStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::default()
        )
        .unwrap();

        let test_points = [0.25, 0.5, 0.75, 1.25, 1.5, 1.75, 2.25, 2.5, 2.75];
        let mut values = Vec::new();

        for &t in &test_points {
            let df = interp.interp(t);
            values.push(df);

            assert!(
                df > 0.0 && df.is_finite(),
                "Invalid value at t={}: {}",
                t,
                df
            );
        }

        for i in 1..values.len() {
            assert!(
                values[i] <= values[i - 1] + 1e-10,
                "Monotonicity violated between {} and {}: {} > {}",
                test_points[i - 1],
                test_points[i],
                values[i - 1],
                values[i]
            );
        }
    }

    #[test]
    fn derivative_at_all_knot_points() {
        let interp = new_strict!(
            Interpolator<MonotoneConvexStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::FlatZero
        )
        .unwrap();

        for &x in &[0.0, 1.0, 2.0, 3.0] {
            let deriv = interp.interp_prime(x);
            assert!(deriv.is_finite());
            assert!(deriv <= 0.0);
        }
    }

    #[test]
    fn derivative_extrapolation_flat_forward_left() {
        let interp = new_strict!(
            Interpolator<MonotoneConvexStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::FlatForward
        )
        .unwrap();

        for &x in &[-0.5, -1.0, -2.0] {
            let deriv = interp.interp_prime(x);
            assert!(
                deriv.is_finite(),
                "Left extrapolation derivative at {} should be finite",
                x
            );
            assert!(
                deriv < 0.0,
                "Left extrapolation derivative should be negative for decreasing curve"
            );
        }
    }

    #[test]
    fn derivative_extrapolation_flat_forward_right() {
        let interp = new_strict!(
            Interpolator<MonotoneConvexStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::FlatForward
        )
        .unwrap();

        for &x in &[4.0, 5.0, 10.0] {
            let deriv = interp.interp_prime(x);
            assert!(
                deriv.is_finite(),
                "Right extrapolation derivative at {} should be finite",
                x
            );
            assert!(
                deriv < 0.0,
                "Right extrapolation derivative should be negative for decreasing curve"
            );
        }
    }

    #[test]
    fn interp_at_exact_knots_returns_exact_values() {
        let interp = new_strict!(
            Interpolator<MonotoneConvexStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::FlatZero
        )
        .unwrap();

        assert!(approx_eq(interp.interp(0.0), 1.0, 1e-15));
        assert!(approx_eq(interp.interp(1.0), 0.95, 1e-15));
        assert!(approx_eq(interp.interp(2.0), 0.9, 1e-15));
        assert!(approx_eq(interp.interp(3.0), 0.85, 1e-15));
    }

    #[test]
    fn extrapolation_flat_forward_values_left() {
        let interp = new_strict!(
            Interpolator<MonotoneConvexStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::FlatForward
        )
        .unwrap();

        let left_val = interp.interp(-0.5);
        assert!(left_val > 1.0, "Left extrapolated value should be > 1.0");
        assert!(left_val.is_finite());
    }

    #[test]
    fn extrapolation_flat_forward_values_right() {
        let interp = new_strict!(
            Interpolator<MonotoneConvexStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::FlatForward
        )
        .unwrap();

        let right_val = interp.interp(4.0);
        assert!(
            right_val < 0.85,
            "Right extrapolated value should be < 0.85"
        );
        assert!(right_val > 0.0, "Right extrapolated value should be > 0");
        assert!(right_val.is_finite());
    }

    #[test]
    fn convexity_constraint_with_steep_curve() {
        let knots = vec![0.0, 1.0, 2.0, 3.0].into_boxed_slice();
        let steep_dfs = vec![1.0, 0.8, 0.6, 0.4].into_boxed_slice();

        let interp = new_strict!(
            Interpolator<MonotoneConvexStrategy>,
            knots,
            steep_dfs,
            ExtrapolationPolicy::FlatZero
        )
        .unwrap();

        for x in (0..30).map(|i| i as f64 * 0.1) {
            let val = interp.interp(x);
            assert!(val > 0.0 && val.is_finite(), "Value at {} = {}", x, val);
        }
    }

    #[test]
    fn near_zero_slope_handling() {
        let knots = vec![0.0, 1.0, 2.0, 3.0].into_boxed_slice();
        let near_flat = vec![1.0, 0.9999, 0.9998, 0.9997].into_boxed_slice();

        let interp = new_strict!(
            Interpolator<MonotoneConvexStrategy>,
            knots,
            near_flat,
            ExtrapolationPolicy::FlatZero
        )
        .unwrap();

        for x in (0..30).map(|i| i as f64 * 0.1) {
            let val = interp.interp(x);
            assert!(
                val > 0.0 && val.is_finite(),
                "Near-flat curve value at {} = {}",
                x,
                val
            );
        }
    }

    /// Non-monotone DFs (sign-changing forwards) now build successfully and
    /// round-trip the knot values exactly.
    #[test]
    fn sign_change_in_slopes() {
        let knots = vec![0.0, 1.0, 2.0, 3.0, 4.0].into_boxed_slice();
        let non_monotone = vec![1.0, 0.9, 0.85, 0.86, 0.84];

        let interp = new_strict!(
            Interpolator<MonotoneConvexStrategy>,
            knots,
            non_monotone.clone().into_boxed_slice(),
            ExtrapolationPolicy::FlatZero
        )
        .expect("non-monotone (mixed-sign forward) input should build");

        for (i, &df) in non_monotone.iter().enumerate() {
            assert!(
                approx_eq(interp.interp(i as f64), df, 1e-12),
                "knot {i} DF should round-trip"
            );
        }
    }

    #[test]
    fn two_point_curve() {
        let knots = vec![0.0, 1.0].into_boxed_slice();
        let dfs = vec![1.0, 0.95].into_boxed_slice();

        let interp = new_strict!(
            Interpolator<MonotoneConvexStrategy>,
            knots,
            dfs,
            ExtrapolationPolicy::FlatZero
        )
        .unwrap();

        assert!(approx_eq(interp.interp(0.0), 1.0, 1e-12));
        assert!(approx_eq(interp.interp(1.0), 0.95, 1e-12));

        let mid = interp.interp(0.5);
        assert!(mid > 0.95 && mid < 1.0, "Midpoint should be between values");
    }

    #[test]
    fn five_point_curve() {
        let knots = vec![0.0, 0.5, 1.0, 2.0, 3.0].into_boxed_slice();
        let dfs = vec![1.0, 0.98, 0.95, 0.9, 0.85].into_boxed_slice();

        let interp = new_strict!(
            Interpolator<MonotoneConvexStrategy>,
            knots,
            dfs,
            ExtrapolationPolicy::FlatZero
        )
        .unwrap();

        assert!(approx_eq(interp.interp(0.0), 1.0, 1e-12));
        assert!(approx_eq(interp.interp(0.5), 0.98, 1e-12));
        assert!(approx_eq(interp.interp(1.0), 0.95, 1e-12));
        assert!(approx_eq(interp.interp(2.0), 0.9, 1e-12));
        assert!(approx_eq(interp.interp(3.0), 0.85, 1e-12));
    }

    // ========================================================================
    // Hagan-West monotonicity projection
    // ========================================================================
    //
    // Regression tests for the Hagan-West 2006 Figure 6 projection. The
    // original implementation collapsed same-sign α, β scalings to
    //     scale = fd_i.abs() / max(|α|, |β|)
    // which over-flattens whenever fd_i is small (ZIRP) because the scale
    // is driven by fd_i magnitude alone, not by the amount of negative
    // excursion in f(x) = fd_i + g(x) on [0, 1].
    //
    // The corrected projection scales (α, β) by a single factor
    //     η = fd_i / (fd_i - min_f)
    // where min_f < 0 is the pre-adjustment minimum of f on the segment.
    // This places (α', β') on the ray from origin through (α, β) at the
    // exact boundary min(f) = 0, preserving the ratio α:β and therefore
    // the shape of the segment forward.
    //
    // Reference: Hagan, P. S., & West, G. (2006). "Interpolation Methods
    // for Curve Construction." Applied Mathematical Finance, 13(2), §6
    // and Figure 6.

    /// At a short-end knot with fd_i ≈ 10 bp and an opposite-sign deviation
    /// pattern (α < 0, β > 0, |α| = |β|), the old code forced both knot
    /// forwards to fd_i (loss of all slope information). The fixed code
    /// preserves the slope: f[0] = 0, f[1] = 2·fd_i.
    #[test]
    fn zirp_preserves_curvature_at_left_boundary() {
        // Construct a ZIRP-like humped short-end forward curve.
        // Discrete forwards: fd = [10bp, 50bp, 10bp] on unit-tenor knots.
        //   DF[0] = 1
        //   DF[1] = exp(-fd[0])
        //   DF[2] = DF[1] · exp(-fd[1])
        //   DF[3] = DF[2] · exp(-fd[2])
        let fd0 = 0.001_f64;
        let fd1 = 0.005_f64;
        let fd2 = 0.001_f64;
        let df0 = 1.0_f64;
        let df1 = df0 * (-fd0).exp();
        let df2 = df1 * (-fd1).exp();
        let df3 = df2 * (-fd2).exp();

        let knots = vec![0.0, 1.0, 2.0, 3.0].into_boxed_slice();
        let dfs = vec![df0, df1, df2, df3].into_boxed_slice();
        let interp = new_strict!(
            Interpolator<MonotoneConvexStrategy>,
            knots,
            dfs,
            ExtrapolationPolicy::FlatZero
        )
        .unwrap();

        // Probe the instantaneous forward at the left boundary via
        //     f(0) = -DF'(0) / DF(0) = -interp_prime(0) / interp(0).
        // With fd[0] = 10 bp and boundary extrapolation producing a negative
        // raw f[0] = 1.5·fd[0] - 0.5·fd[1] = -10 bp, after projection:
        //   Old code: forces f[0] = fd[0] = 10 bp (flat, curvature lost).
        //   New code: eta = 0.5 so f[0] = 0 bp.
        let df_at_0 = interp.interp(0.0);
        let dfp_at_0 = interp.interp_prime(0.0);
        let fwd_at_0 = -dfp_at_0 / df_at_0;

        // Must be non-negative (the Hagan-West invariant).
        assert!(
            fwd_at_0 >= -1e-12,
            "Forward at left boundary must be non-negative, got {}",
            fwd_at_0
        );

        // Old buggy code returned 10 bp (fd[0]). Fixed code returns ~0 bp.
        // Assert strictly less than 5 bp to catch the collapse-to-flat bug.
        assert!(
            fwd_at_0 < 0.0005,
            "ZIRP curvature collapsed: f(0) = {:.6}, expected < 5bp. \
             The old Hagan-West projection forced f[0] = fd[0] = 10 bp \
             instead of projecting (α, β) to the feasibility boundary.",
            fwd_at_0
        );
    }

    /// Mirror of the left-boundary ZIRP test: at the right boundary the
    /// same collapse-to-flat bug applied symmetrically. The fix preserves
    /// the descending slope at the right end.
    #[test]
    fn zirp_preserves_curvature_at_right_boundary() {
        let fd0 = 0.001_f64;
        let fd1 = 0.005_f64;
        let fd2 = 0.001_f64;
        let df0 = 1.0_f64;
        let df1 = df0 * (-fd0).exp();
        let df2 = df1 * (-fd1).exp();
        let df3 = df2 * (-fd2).exp();

        let knots = vec![0.0, 1.0, 2.0, 3.0].into_boxed_slice();
        let dfs = vec![df0, df1, df2, df3].into_boxed_slice();
        let interp = new_strict!(
            Interpolator<MonotoneConvexStrategy>,
            knots,
            dfs,
            ExtrapolationPolicy::FlatZero
        )
        .unwrap();

        let df_at_3 = interp.interp(3.0);
        let dfp_at_3 = interp.interp_prime(3.0);
        let fwd_at_3 = -dfp_at_3 / df_at_3;

        assert!(
            fwd_at_3 >= -1e-12,
            "Forward at right boundary must be non-negative, got {}",
            fwd_at_3
        );
        assert!(
            fwd_at_3 < 0.0005,
            "ZIRP curvature collapsed at right end: f(3) = {:.6}, expected < 5bp.",
            fwd_at_3
        );
    }

    /// The Hagan-West non-negativity invariant: for any arbitrage-free
    /// discount curve, every interpolated forward on every segment must be
    /// non-negative. This is the *reason* the monotonicity constraint
    /// exists in the first place. A property-style test over a handful of
    /// stressed curves.
    #[test]
    fn forward_rates_are_non_negative_on_stressed_curves() {
        // Build several curves that have historically caused adjustment:
        //   (a) ZIRP humped short end
        //   (b) ZIRP inverted short end
        //   (c) Steep front-loaded (2y at 8%, rest at 50bp)
        //   (d) Near-flat micro-rate (all forwards ~5 bp)
        let curves: [(&str, Vec<f64>, Vec<f64>); 4] = [
            (
                "zirp_humped",
                vec![0.0, 1.0, 2.0, 3.0, 5.0],
                forwards_to_dfs(&[0.001, 0.005, 0.001, 0.002], &[1.0, 1.0, 1.0, 2.0]),
            ),
            (
                "zirp_inverted",
                vec![0.0, 0.5, 1.0, 2.0, 5.0],
                forwards_to_dfs(&[0.002, 0.001, 0.0005, 0.0003], &[0.5, 0.5, 1.0, 3.0]),
            ),
            (
                "steep_front",
                vec![0.0, 1.0, 2.0, 3.0, 5.0],
                forwards_to_dfs(&[0.08, 0.01, 0.005, 0.005], &[1.0, 1.0, 1.0, 2.0]),
            ),
            (
                "micro_rate",
                vec![0.0, 1.0, 2.0, 5.0, 10.0],
                forwards_to_dfs(&[0.0005, 0.0005, 0.0004, 0.0003], &[1.0, 1.0, 3.0, 5.0]),
            ),
        ];

        for (name, knots, dfs) in &curves {
            let interp = new_strict!(
                Interpolator<MonotoneConvexStrategy>,
                knots.clone().into_boxed_slice(),
                dfs.clone().into_boxed_slice(),
                ExtrapolationPolicy::FlatZero
            )
            .unwrap_or_else(|e| panic!("Curve {name} failed to build: {e:?}"));

            // Sample 41 points across the domain and compute forwards.
            let first = *knots.first().unwrap();
            let last = *knots.last().unwrap();
            let steps = 41;
            for i in 0..steps {
                let t = first + (last - first) * (i as f64) / ((steps - 1) as f64);
                let df = interp.interp(t);
                let dfp = interp.interp_prime(t);
                assert!(
                    df > 0.0,
                    "Curve {name}: DF({t}) = {df} not positive (arbitrage violation)"
                );
                let fwd = -dfp / df;
                assert!(
                    fwd >= -1e-10,
                    "Curve {name}: forward at t={t} is negative ({fwd}); \
                     Hagan-West non-negativity invariant violated"
                );
            }
        }
    }

    /// Helper: build monotone-decreasing DFs from piecewise-constant forwards
    /// over segments of the given widths. DF[0] = 1.
    fn forwards_to_dfs(forwards: &[f64], widths: &[f64]) -> Vec<f64> {
        assert_eq!(forwards.len(), widths.len());
        let mut dfs = Vec::with_capacity(forwards.len() + 1);
        dfs.push(1.0);
        let mut df = 1.0;
        for (f, w) in forwards.iter().zip(widths.iter()) {
            df *= (-(*f) * (*w)).exp();
            dfs.push(df);
        }
        dfs
    }
}

// ============================================================================
// Trait Tests
// ============================================================================

mod traits {
    use super::*;

    #[derive(Debug)]
    struct TestInterp {
        value: f64,
    }

    impl InterpFn for TestInterp {
        fn interp(&self, _x: f64) -> f64 {
            self.value
        }
    }

    #[test]
    fn interp_fn_default_derivative_uses_finite_differences() {
        let interp = TestInterp { value: 42.0 };

        let deriv = interp.interp_prime(1.0);
        assert!(deriv.abs() < 1e-6);
    }

    #[test]
    fn interp_fn_default_derivative_for_linear_function() {
        let knots = vec![0.0, 1.0, 2.0].into_boxed_slice();
        let values = vec![1.0, 0.5, 0.1].into_boxed_slice();
        let interp = new_strict!(
            Interpolator<LinearStrategy>,
            knots,
            values,
            ExtrapolationPolicy::FlatZero
        )
        .unwrap();

        let deriv_0_5 = interp.interp_prime(0.5);
        let deriv_1_5 = interp.interp_prime(1.5);

        assert!((deriv_0_5 - (-0.5)).abs() < 1e-6);
        assert!((deriv_1_5 - (-0.4)).abs() < 1e-6);
    }

    #[test]
    fn interp_fn_derivative_for_exponential_function() {
        let knots = vec![0.0, 1.0, 2.0].into_boxed_slice();
        let dfs = vec![1.0, 0.95, 0.9].into_boxed_slice();
        let interp = new_strict!(
            Interpolator<LogLinearStrategy>,
            knots,
            dfs,
            ExtrapolationPolicy::FlatZero
        )
        .unwrap();

        let deriv_0_5 = interp.interp_prime(0.5);
        let deriv_1_5 = interp.interp_prime(1.5);

        assert!(deriv_0_5 < 0.0);
        assert!(deriv_1_5 < 0.0);
        assert!(deriv_0_5.is_finite());
        assert!(deriv_1_5.is_finite());
    }

    #[test]
    fn interp_fn_derivative_consistency() {
        let knots = vec![0.0, 1.0, 2.0, 3.0].into_boxed_slice();
        let values = vec![1.0, 0.8, 0.6, 0.4].into_boxed_slice();
        let interp = new_strict!(
            Interpolator<LinearStrategy>,
            knots,
            values,
            ExtrapolationPolicy::FlatZero
        )
        .unwrap();

        let x = 1.5;
        let h = 6e-6;
        let numerical = (interp.interp(x + h) - interp.interp(x - h)) / (2.0 * h);
        let analytical = interp.interp_prime(x);

        assert!((numerical - analytical).abs() < 1e-8);
    }

    #[test]
    fn interp_fn_send_sync_bounds() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Box<dyn InterpFn>>();
    }

    #[test]
    fn interp_fn_derivative_at_boundaries() {
        let knots = vec![0.0, 1.0, 2.0].into_boxed_slice();
        let values = vec![1.0, 0.5, 0.25].into_boxed_slice();
        let interp = new_strict!(
            Interpolator<LinearStrategy>,
            knots,
            values,
            ExtrapolationPolicy::FlatZero
        )
        .unwrap();

        let deriv_0 = interp.interp_prime(0.0);
        let deriv_1 = interp.interp_prime(1.0);
        let deriv_2 = interp.interp_prime(2.0);

        assert!(deriv_0.is_finite());
        assert!(deriv_1.is_finite());
        assert!(deriv_2.is_finite());

        assert!(deriv_0 <= 0.0);
        assert!(deriv_1 <= 0.0);
        assert!(deriv_2 <= 0.0);
    }

    #[test]
    fn interp_fn_derivative_extrapolation_flat_zero() {
        let knots = vec![0.0, 1.0].into_boxed_slice();
        let values = vec![1.0, 0.5].into_boxed_slice();
        let interp = new_strict!(
            Interpolator<LinearStrategy>,
            knots,
            values,
            ExtrapolationPolicy::FlatZero
        )
        .unwrap();

        let deriv_below = interp.interp_prime(-0.5);
        let deriv_above = interp.interp_prime(2.0);

        assert_eq!(deriv_below, 0.0);
        assert_eq!(deriv_above, 0.0);
    }

    #[test]
    fn interp_fn_derivative_extrapolation_flat_forward() {
        let knots = vec![0.0, 1.0].into_boxed_slice();
        let values = vec![1.0, 0.5].into_boxed_slice();
        let interp = new_strict!(
            Interpolator<LinearStrategy>,
            knots,
            values,
            ExtrapolationPolicy::FlatForward
        )
        .unwrap();

        let deriv_interior = interp.interp_prime(0.5);
        let deriv_below = interp.interp_prime(-0.5);
        let deriv_above = interp.interp_prime(2.0);

        assert!((deriv_interior - deriv_below).abs() < 1e-6);
        assert!((deriv_interior - deriv_above).abs() < 1e-6);
    }
}

// ============================================================================
// Serialization Tests
// ============================================================================

mod serde_tests {
    use super::*;

    #[test]
    fn linear_df_roundtrip() {
        let knots = vec![0.0, 1.0, 2.0, 3.0, 5.0].into_boxed_slice();
        let dfs = vec![1.0, 0.98, 0.95, 0.92, 0.87].into_boxed_slice();

        let linear = new_strict!(
            Interpolator<LinearStrategy>,
            knots,
            dfs,
            ExtrapolationPolicy::FlatZero
        )
        .unwrap();
        let json = serde_json::to_string_pretty(&linear).unwrap();
        let deserialized: Interpolator<LinearStrategy> = serde_json::from_str(&json).unwrap();
        assert_eq!(linear.interp(1.5), deserialized.interp(1.5));
    }

    #[test]
    fn log_linear_df_roundtrip() {
        let knots = vec![0.0, 1.0, 2.0, 3.0, 5.0].into_boxed_slice();
        let dfs = vec![1.0, 0.98, 0.95, 0.92, 0.87].into_boxed_slice();

        let log_linear = new_strict!(
            Interpolator<LogLinearStrategy>,
            knots,
            dfs,
            ExtrapolationPolicy::FlatZero
        )
        .unwrap();
        let json = serde_json::to_string_pretty(&log_linear).unwrap();
        let deserialized: Interpolator<LogLinearStrategy> = serde_json::from_str(&json).unwrap();
        assert!((log_linear.interp(1.5) - deserialized.interp(1.5)).abs() < 1e-10);
    }

    #[test]
    fn monotone_convex_roundtrip() {
        let knots = vec![0.0, 1.0, 2.0, 3.0, 5.0].into_boxed_slice();
        let dfs = vec![1.0, 0.98, 0.95, 0.92, 0.87].into_boxed_slice();

        let monotone = new_strict!(
            Interpolator<MonotoneConvexStrategy>,
            knots,
            dfs,
            ExtrapolationPolicy::FlatZero
        )
        .unwrap();
        let json = serde_json::to_string_pretty(&monotone).unwrap();
        let deserialized: Interpolator<MonotoneConvexStrategy> =
            serde_json::from_str(&json).unwrap();
        assert!((monotone.interp(1.5) - deserialized.interp(1.5)).abs() < 1e-10);
    }

    #[test]
    fn cubic_hermite_roundtrip() {
        let knots = vec![0.0, 1.0, 2.0, 3.0, 5.0].into_boxed_slice();
        let dfs = vec![1.0, 0.98, 0.95, 0.92, 0.87].into_boxed_slice();

        let cubic = new_strict!(
            Interpolator<CubicHermiteStrategy>,
            knots,
            dfs,
            ExtrapolationPolicy::FlatZero
        )
        .unwrap();
        let json = serde_json::to_string_pretty(&cubic).unwrap();
        let deserialized: Interpolator<CubicHermiteStrategy> = serde_json::from_str(&json).unwrap();
        assert!((cubic.interp(1.5) - deserialized.interp(1.5)).abs() < 1e-10);
    }

    #[test]
    fn flat_fwd_roundtrip() {
        let knots = vec![0.0, 1.0, 2.0, 3.0, 5.0].into_boxed_slice();
        let dfs = vec![1.0, 0.98, 0.95, 0.92, 0.87].into_boxed_slice();

        let flat_fwd = new_strict!(
            Interpolator<LogLinearStrategy>,
            knots,
            dfs,
            ExtrapolationPolicy::FlatZero
        )
        .unwrap();
        let json = serde_json::to_string_pretty(&flat_fwd).unwrap();
        let deserialized: Interpolator<LogLinearStrategy> = serde_json::from_str(&json).unwrap();
        assert!((flat_fwd.interp(1.5) - deserialized.interp(1.5)).abs() < 1e-10);
    }
}

// ============================================================================
// ExtrapolationPolicy Tests
// ============================================================================

#[cfg(test)]
mod extrapolation_policy_tests {
    use super::*;

    #[test]
    fn default_is_flat_zero() {
        let default = ExtrapolationPolicy::default();
        // FlatZero is the default
        match default {
            ExtrapolationPolicy::FlatZero => {}
            _ => panic!("Default should be FlatZero"),
        }
    }

    #[test]
    fn flat_zero_behavior() {
        let interp = InterpStyle::Linear
            .build(
                standard_knots(),
                standard_dfs(),
                ExtrapolationPolicy::FlatZero,
                ValidationPolicy::Strict,
            )
            .unwrap();

        // Below minimum knot (0.0), should return boundary value
        let val = interp.interp(-1.0);
        assert!(approx_eq(val, 1.0, 1e-10), "Should use boundary value");

        // Above maximum knot (3.0), should return boundary value
        let val_high = interp.interp(10.0);
        assert!(
            approx_eq(val_high, 0.85, 1e-10),
            "Should use boundary value"
        );
    }

    #[test]
    fn flat_forward_behavior() {
        let interp = InterpStyle::Linear
            .build(
                standard_knots(),
                standard_dfs(),
                ExtrapolationPolicy::FlatForward,
                ValidationPolicy::Strict,
            )
            .unwrap();

        // Test extrapolation (exact behavior depends on implementation)
        let _ = interp.interp(-1.0);
        let _ = interp.interp(10.0);
        // Just verify it doesn't panic with FlatForward
    }

    #[test]
    fn extrapolation_policy_serialization() {
        // Test FlatZero
        let flat_zero = ExtrapolationPolicy::FlatZero;
        let json = serde_json::to_string(&flat_zero).unwrap();
        let deserialized: ExtrapolationPolicy = serde_json::from_str(&json).unwrap();
        match deserialized {
            ExtrapolationPolicy::FlatZero => {}
            _ => panic!("Should deserialize to FlatZero"),
        }

        // Test FlatForward
        let flat_fwd = ExtrapolationPolicy::FlatForward;
        let json = serde_json::to_string(&flat_fwd).unwrap();
        let deserialized: ExtrapolationPolicy = serde_json::from_str(&json).unwrap();
        match deserialized {
            ExtrapolationPolicy::FlatForward => {}
            _ => panic!("Should deserialize to FlatForward"),
        }
    }
}

// ============================================================================
// DERIVATIVE_EPSILON Constant Tests
// ============================================================================

#[cfg(test)]
mod derivative_epsilon_tests {
    use super::*;

    #[test]
    fn derivative_epsilon_defined() {
        // Verify constant is accessible and has expected value
        assert_eq!(DERIVATIVE_EPSILON, 1e-6);
    }

    #[test]
    fn derivative_epsilon_usage() {
        // Test that DERIVATIVE_EPSILON is reasonable for finite differences
        let epsilon = DERIVATIVE_EPSILON;

        // Simple function: f(x) = x^2, f'(x) = 2x
        let f = |x: f64| x * x;
        let x = 2.0;
        let true_deriv = 2.0 * x; // = 4.0

        // Finite difference approximation
        let approx_deriv = (f(x + epsilon) - f(x)) / epsilon;

        // Should be close with this epsilon
        assert!(
            (approx_deriv - true_deriv).abs() < 1e-5,
            "DERIVATIVE_EPSILON should give reasonable approximations"
        );
    }
}

// ============================================================================
// InterpStyle PartialEq Tests
// ============================================================================

#[cfg(test)]
mod interp_style_eq_tests {
    use super::*;

    #[test]
    fn interp_style_equality() {
        assert_eq!(InterpStyle::Linear, InterpStyle::Linear);
        assert_eq!(InterpStyle::LogLinear, InterpStyle::LogLinear);
        assert_eq!(InterpStyle::MonotoneConvex, InterpStyle::MonotoneConvex);
        assert_eq!(InterpStyle::CubicHermite, InterpStyle::CubicHermite);
        assert_eq!(
            InterpStyle::PiecewiseQuadraticForward,
            InterpStyle::PiecewiseQuadraticForward
        );
    }

    #[test]
    fn interp_style_inequality() {
        assert_ne!(InterpStyle::Linear, InterpStyle::LogLinear);
        assert_ne!(InterpStyle::LogLinear, InterpStyle::MonotoneConvex);
        assert_ne!(InterpStyle::MonotoneConvex, InterpStyle::CubicHermite);
    }
}

// ============================================================================
// Hagan-West forward non-negativity property tests (seeded)
// ============================================================================

mod monotone_convex_positivity {
    use super::*;
    use finstack_core::math::random::{Pcg64Rng, RandomNumberGenerator};

    /// Randomized-but-seeded property test: monotone-convex forwards must be
    /// non-negative on stressed curves where steep segments sit next to
    /// near-zero segments. Regression test for the sequential Hagan-West
    /// positivity projection re-violating earlier segments (fixed by the
    /// bounded fixpoint sweep in `apply_monotonicity_constraints`).
    #[test]
    fn forwards_non_negative_on_stressed_curves() {
        // Fixed seeds per testing standards: deterministic and reproducible.
        for seed in [7_u64, 42, 1234, 987_654] {
            let mut rng = Pcg64Rng::new(seed);

            for curve_idx in 0..50 {
                // 4..=10 knots with random spacing.
                let n = 4 + (rng.uniform() * 7.0) as usize;
                let mut knots = Vec::with_capacity(n);
                let mut t = 0.0;
                knots.push(t);
                for _ in 1..n {
                    t += 0.05 + 1.95 * rng.uniform();
                    knots.push(t);
                }

                // Stressed discrete forwards: alternate steep and near-zero
                // regimes so the boundary/interior forward estimates overshoot.
                let mut dfs = Vec::with_capacity(n);
                dfs.push(1.0_f64);
                for i in 1..n {
                    let fd = if rng.uniform() < 0.5 {
                        0.0005 + 0.002 * rng.uniform() // near-ZIRP segment
                    } else {
                        0.08 + 0.12 * rng.uniform() // steep segment
                    };
                    let dt = knots[i] - knots[i - 1];
                    dfs.push(dfs[i - 1] * (-fd * dt).exp());
                }

                let interp = new_strict!(
                    Interpolator<MonotoneConvexStrategy>,
                    knots.clone().into(),
                    dfs.into(),
                    ExtrapolationPolicy::FlatForward
                )
                .expect("stressed curve should build");

                // Dense sampling of the instantaneous forward
                // f(t) = -DF'(t)/DF(t) across the interpolation domain.
                let t_max = knots[n - 1];
                let samples = 400;
                for s in 0..=samples {
                    let x = t_max * (s as f64) / (samples as f64);
                    let df = interp.interp(x);
                    let df_prime = interp.interp_prime(x);
                    let fwd = -df_prime / df;
                    assert!(
                        fwd >= -1e-10,
                        "negative forward {fwd} at t={x} (seed={seed}, curve={curve_idx}, knots={knots:?})"
                    );
                }
            }
        }
    }
}

// ============================================================================
// Negative-rate curve support
// ============================================================================
//
// Per Hagan & West (2006), forward-positivity enforcement is an optional
// amelioration valid only for positive curves. MonotoneConvex auto-detects
// negative discrete forwards and skips the projection so negative-rate
// curves (EUR/CHF/JPY, DF > 1) interpolate faithfully.

mod monotone_convex_negative_rates {
    use super::*;
    use finstack_core::math::random::{Pcg64Rng, RandomNumberGenerator};

    /// Sample the instantaneous forward f(t) = -DF'(t)/DF(t).
    fn fwd_at(interp: &Interpolator<MonotoneConvexStrategy>, t: f64) -> f64 {
        -interp.interp_prime(t) / interp.interp(t)
    }

    /// Flat -1% continuous curve: DF(t) = e^{+0.01 t} > 1, increasing.
    /// Builds, round-trips the flat -1% zero rate, and the interpolated
    /// forwards are ~ -1% everywhere (no zero-clamping).
    #[test]
    fn flat_negative_one_percent_curve() {
        let rate = -0.01_f64;
        let knot_times = [0.0, 1.0, 2.0, 3.0, 5.0, 10.0];
        let dfs: Vec<f64> = knot_times.iter().map(|&t| (-rate * t).exp()).collect();

        let interp = new_strict!(
            Interpolator<MonotoneConvexStrategy>,
            knot_times.to_vec().into_boxed_slice(),
            dfs.clone().into_boxed_slice(),
            ExtrapolationPolicy::FlatForward
        )
        .expect("flat -1% curve (DF > 1) should build");

        // Knot DFs round-trip exactly (tolerance 1e-10).
        for (&t, &df) in knot_times.iter().zip(dfs.iter()) {
            if t > 0.0 {
                assert!(df > 1.0, "DF({t}) must exceed 1");
            }
            assert!(
                approx_eq(interp.interp(t), df, 1e-10),
                "DF at knot t={t} should round-trip"
            );
        }

        // Between knots: zero rate and forwards are both ~ -1%.
        for i in 0..80 {
            let t = 0.05 + 10.0 * (i as f64) / 80.0;
            if t > 10.0 {
                break;
            }
            let df = interp.interp(t);
            let zero = -df.ln() / t;
            assert!(
                approx_eq(zero, rate, 1e-9),
                "zero rate at t={t} should be -1%, got {zero}"
            );
            let fwd = fwd_at(&interp, t);
            assert!(
                approx_eq(fwd, rate, 1e-9),
                "forward at t={t} should be -1% (no zero-clamping), got {fwd}"
            );
        }
    }

    /// Mixed curve crossing zero: discrete forwards +50bp -> -30bp -> +20bp.
    /// Builds, segment-average forwards reproduce the discrete forwards, and
    /// the negative segment's forwards are genuinely negative (no clamping).
    #[test]
    fn mixed_sign_forwards_not_clamped() {
        let fds = [0.005_f64, -0.003, 0.002];
        let knot_times = [0.0_f64, 1.0, 2.0, 3.0];
        let mut dfs = vec![1.0_f64];
        for (i, fd) in fds.iter().enumerate() {
            let dt = knot_times[i + 1] - knot_times[i];
            dfs.push(dfs[i] * (-fd * dt).exp());
        }

        let interp = new_strict!(
            Interpolator<MonotoneConvexStrategy>,
            knot_times.to_vec().into_boxed_slice(),
            dfs.clone().into_boxed_slice(),
            ExtrapolationPolicy::FlatForward
        )
        .expect("mixed-sign forward curve should build");

        // Knot DFs round-trip.
        for (&t, &df) in knot_times.iter().zip(dfs.iter()) {
            assert!(
                approx_eq(interp.interp(t), df, 1e-12),
                "DF at knot t={t} should round-trip"
            );
        }

        // Segment-average forwards reproduce the discrete forwards: the
        // monotone-convex interpolant integrates g(x) to zero per segment.
        for (i, fd) in fds.iter().enumerate() {
            let (t0, t1) = (knot_times[i], knot_times[i + 1]);
            let avg = (interp.interp(t0) / interp.interp(t1)).ln() / (t1 - t0);
            assert!(
                approx_eq(avg, *fd, 1e-10),
                "segment {i} average forward {avg} should equal discrete {fd}"
            );
        }

        // The negative segment [1, 2] must produce negative forwards at its
        // midpoint — the positivity projection must NOT clamp them to zero.
        let fwd_mid = fwd_at(&interp, 1.5);
        assert!(
            fwd_mid < -0.003,
            "midpoint forward of -30bp segment should be negative and below \
             the discrete forward (interior dip), got {fwd_mid}"
        );
    }

    /// Seeded property companion to `forwards_non_negative_on_stressed_curves`:
    /// all-negative-forward curves (rates in [-1.5%, -5bp]) build, round-trip
    /// knot DFs, produce finite forwards, and retain negative forwards
    /// (projection skipped — no clamping toward zero).
    #[test]
    fn negative_rate_curves_interpolate_faithfully() {
        for seed in [7_u64, 42, 1234, 987_654] {
            let mut rng = Pcg64Rng::new(seed);

            for curve_idx in 0..50 {
                let n = 4 + (rng.uniform() * 7.0) as usize;
                let mut knots = Vec::with_capacity(n);
                let mut t = 0.0;
                knots.push(t);
                for _ in 1..n {
                    t += 0.05 + 1.95 * rng.uniform();
                    knots.push(t);
                }

                // All-negative discrete forwards in [-1.5%, -5bp].
                let mut dfs = vec![1.0_f64];
                for i in 1..n {
                    let fd = -(0.0005 + 0.0145 * rng.uniform());
                    let dt = knots[i] - knots[i - 1];
                    dfs.push(dfs[i - 1] * (-fd * dt).exp());
                }

                let interp = new_strict!(
                    Interpolator<MonotoneConvexStrategy>,
                    knots.clone().into(),
                    dfs.clone().into(),
                    ExtrapolationPolicy::FlatForward
                )
                .expect("negative-rate curve should build");

                for (i, &df) in dfs.iter().enumerate() {
                    assert!(
                        approx_eq(interp.interp(knots[i]), df, 1e-10),
                        "knot DF round-trip (seed={seed}, curve={curve_idx})"
                    );
                }

                let t_max = knots[n - 1];
                let mut min_fwd = f64::INFINITY;
                for s in 0..=200 {
                    let x = t_max * (s as f64) / 200.0;
                    let df = interp.interp(x);
                    let fwd = -interp.interp_prime(x) / df;
                    assert!(
                        df.is_finite() && df > 0.0 && fwd.is_finite(),
                        "non-finite DF/forward at t={x} (seed={seed}, curve={curve_idx})"
                    );
                    min_fwd = min_fwd.min(fwd);
                }
                // Discrete forwards are all <= -5bp, so the interpolated
                // forward must be genuinely negative somewhere: clamping
                // toward zero would violate this.
                assert!(
                    min_fwd < -0.0005,
                    "forwards were clamped (min {min_fwd}; seed={seed}, curve={curve_idx})"
                );
            }
        }
    }
}
