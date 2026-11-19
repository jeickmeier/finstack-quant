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
use finstack_quant_core::math::interp::*;

// Helper to create interpolator
macro_rules! new_interp {
    ($ty:ty, $knots:expr_2021, $dfs:expr_2021, $extrap:expr_2021) => {
        <$ty>::new($knots, $dfs, $extrap)
    };
}

// Basic Tests (macro-generated for all interpolator types)

macro_rules! interp_basic_tests {
    ($mod_name:ident, $ty:ty, $requires_positive:expr_2021) => {
        mod $mod_name {
            use super::*;

            #[test]
            fn construction_succeeds() {
                let result = new_interp!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::Flat
                );
                assert!(result.is_ok(), "Construction should succeed");
            }

            #[test]
            fn exact_knot_values_preserved() {
                let interp = new_interp!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::Flat
                )
                .unwrap();

                assert_eq!(interp.interp(0.0).unwrap().to_bits(), 1.0_f64.to_bits());
                assert_eq!(interp.interp(1.0).unwrap().to_bits(), 0.95_f64.to_bits());
                assert_eq!(interp.interp(2.0).unwrap().to_bits(), 0.9_f64.to_bits());
                assert_eq!(interp.interp(3.0).unwrap().to_bits(), 0.85_f64.to_bits());
            }

            #[test]
            fn interpolation_between_knots() {
                let interp = new_interp!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::Flat
                )
                .unwrap();

                let mid = interp.interp(0.5).unwrap();
                assert!(
                    mid > 0.95 && mid < 1.0,
                    "0.5 value {} not in (0.95, 1.0)",
                    mid
                );

                let mid2 = interp.interp(1.5).unwrap();
                assert!(
                    mid2 > 0.9 && mid2 < 0.95,
                    "1.5 value {} not in (0.9, 0.95)",
                    mid2
                );
            }

            #[test]
            fn rejects_non_increasing_knots() {
                let bad_knots = vec![1.0, 0.0, 2.0, 3.0].into_boxed_slice();
                let result = new_interp!($ty, bad_knots, standard_dfs(), ExtrapolationPolicy::Flat);
                assert!(result.is_err(), "Should reject non-increasing knots");
            }

            #[test]
            fn accepts_tightly_spaced_distinct_knots() {
                let tight_knots = vec![0.0, 1.0e-12, 1.0, 2.0].into_boxed_slice();
                let result =
                    new_interp!($ty, tight_knots, standard_dfs(), ExtrapolationPolicy::Flat);
                assert!(
                    result.is_ok(),
                    "Should accept strictly increasing distinct knots regardless of relative gap"
                );
            }

            #[test]
            fn signed_value_domain_matches_strategy() {
                let signed_dfs = vec![1.0, -0.95, 0.9, 0.85].into_boxed_slice();
                let result =
                    new_interp!($ty, standard_knots(), signed_dfs, ExtrapolationPolicy::Flat);
                if $requires_positive {
                    assert!(result.is_err(), "Positive-domain strategy must reject <= 0");
                } else {
                    assert!(
                        result.is_ok(),
                        "Signed strategy should accept finite signed values"
                    );
                }
            }

            #[test]
            fn two_point_case() {
                let interp = new_interp!(
                    $ty,
                    two_point_knots(),
                    two_point_dfs(),
                    ExtrapolationPolicy::Flat
                )
                .unwrap();

                let mid = interp.interp(0.5).unwrap();
                assert!(
                    mid > 0.95 && mid <= 1.0,
                    "Two-point midpoint {} invalid",
                    mid
                );
                assert!(mid.is_finite());
            }

            #[test]
            fn edge_values_are_finite() {
                let interp = new_interp!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::Flat
                )
                .unwrap();

                let v1 = interp.interp(0.001).unwrap();
                let v2 = interp.interp(2.999).unwrap();
                assert!(v1.is_finite() && v1 > 0.0);
                assert!(v2.is_finite() && v2 > 0.0);
            }

            #[test]
            fn rejects_empty_input() {
                let result = new_interp!(
                    $ty,
                    vec![].into_boxed_slice(),
                    vec![].into_boxed_slice(),
                    ExtrapolationPolicy::Flat
                );
                assert!(result.is_err(), "Should reject empty input");
            }

            #[test]
            fn rejects_mismatched_lengths() {
                let result = new_interp!(
                    $ty,
                    vec![0.0, 1.0, 2.0].into_boxed_slice(),
                    vec![1.0, 0.95].into_boxed_slice(),
                    ExtrapolationPolicy::Flat
                );
                assert!(result.is_err(), "Should reject mismatched lengths");
            }

            #[test]
            fn single_point_rejected() {
                let result = new_interp!(
                    $ty,
                    vec![1.0].into_boxed_slice(),
                    vec![0.95].into_boxed_slice(),
                    ExtrapolationPolicy::Flat
                );
                assert!(result.is_err(), "Interpolators require at least two points");
            }
        }
    };
}

interp_basic_tests!(linear_df_basic, Interpolator<LinearStrategy>, false);
interp_basic_tests!(log_linear_df_basic, Interpolator<LogLinearStrategy>, true);
interp_basic_tests!(
    cubic_hermite_basic,
    Interpolator<CubicHermiteStrategy>,
    false
);
interp_basic_tests!(
    monotone_convex_basic,
    Interpolator<MonotoneConvexStrategy>,
    true
);
interp_basic_tests!(
    piecewise_quadratic_forward_basic,
    Interpolator<PiecewiseQuadraticForwardStrategy>,
    true
);

#[test]
fn piecewise_quadratic_forward_matches_natural_cubic_and_is_continuous() {
    // For x = [0, 1, 3], y = -ln(DF) = [0, .02, .12], the natural
    // spline has second derivatives [0, .03, 0]. Its two segments are
    // y(x) = .015*x + .005*x^3 and
    // y(1+s) = .02 + .03*s + .015*s^2 - .0025*s^3.
    let interp = new_interp!(
        Interpolator<PiecewiseQuadraticForwardStrategy>,
        vec![0.0, 1.0, 3.0].into_boxed_slice(),
        vec![1.0, (-0.02_f64).exp(), (-0.12_f64).exp()].into_boxed_slice(),
        ExtrapolationPolicy::Extend
    )
    .expect("valid natural cubic spline");

    for (time, log_df, forward) in [(0.5, 0.008125_f64, 0.01875), (2.0, 0.0625, 0.0525)] {
        assert!(approx_eq(
            interp.interp(time).unwrap(),
            (-log_df).exp(),
            1e-14
        ));
        let actual_forward = -interp.interp_prime(time).unwrap() / interp.interp(time).unwrap();
        assert!(approx_eq(actual_forward, forward, 1e-14));
    }

    let epsilon = 1e-7;
    let forward_left =
        -interp.interp_prime(1.0 - epsilon).unwrap() / interp.interp(1.0 - epsilon).unwrap();
    let forward_right =
        -interp.interp_prime(1.0 + epsilon).unwrap() / interp.interp(1.0 + epsilon).unwrap();
    assert!(approx_eq(forward_left, 0.03, 1e-8));
    assert!(approx_eq(forward_right, 0.03, 1e-8));
    assert!(approx_eq(forward_left, forward_right, 1e-8));
}

// Extrapolation Policy Tests

macro_rules! extrapolation_tests {
    ($mod_name:ident, $ty:ty) => {
        mod $mod_name {
            use super::*;

            #[test]
            fn flat_extrapolation_left() {
                let interp = new_interp!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::Flat
                )
                .unwrap();

                let left_extrap = interp.interp(-1.0).unwrap();
                assert!(
                    approx_eq(left_extrap, 1.0, 1e-10),
                    "Flat left extrapolation should return first value, got {}",
                    left_extrap
                );
            }

            #[test]
            fn flat_extrapolation_right() {
                let interp = new_interp!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::Flat
                )
                .unwrap();

                let right_extrap = interp.interp(5.0).unwrap();
                assert!(
                    approx_eq(right_extrap, 0.85, 1e-10),
                    "Flat right extrapolation should return last value, got {}",
                    right_extrap
                );
            }

            #[test]
            fn extend_extrapolation_left() {
                let interp = new_interp!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::Extend
                )
                .unwrap();

                let left_extrap = interp.interp(-0.5).unwrap();
                assert!(
                    left_extrap > 1.0 && left_extrap.is_finite(),
                    "Extend left extrapolation {} should be > 1.0",
                    left_extrap
                );
            }

            #[test]
            fn extend_extrapolation_right() {
                let interp = new_interp!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::Extend
                )
                .unwrap();

                let right_extrap = interp.interp(4.0).unwrap();
                assert!(
                    right_extrap < 0.85 && right_extrap > 0.0 && right_extrap.is_finite(),
                    "Extend right extrapolation {} should be < 0.85 and > 0",
                    right_extrap
                );
            }

            #[test]
            fn extrapolation_far_left() {
                let interp = new_interp!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::Flat
                )
                .unwrap();

                let far_left = interp.interp(-100.0).unwrap();
                assert!(far_left.is_finite());
                assert!(approx_eq(far_left, 1.0, 1e-10));
            }

            #[test]
            fn extrapolation_far_right() {
                let interp = new_interp!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::Flat
                )
                .unwrap();

                let far_right = interp.interp(100.0).unwrap();
                assert!(far_right.is_finite());
                assert!(approx_eq(far_right, 0.85, 1e-10));
            }
        }
    };
}

extrapolation_tests!(extrap_linear, Interpolator<LinearStrategy>);
extrapolation_tests!(extrap_log_linear, Interpolator<LogLinearStrategy>);
extrapolation_tests!(extrap_cubic_hermite, Interpolator<CubicHermiteStrategy>);
extrapolation_tests!(extrap_monotone_convex, Interpolator<MonotoneConvexStrategy>);
extrapolation_tests!(
    extrap_piecewise_quadratic_forward,
    Interpolator<PiecewiseQuadraticForwardStrategy>
);

macro_rules! error_extrapolation_tests {
    ($mod_name:ident, $ty:ty) => {
        mod $mod_name {
            use super::*;

            #[test]
            fn out_of_bounds_value_and_derivative_error() {
                let interp = new_interp!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::Error
                )
                .unwrap();

                for x in [-0.25, 3.25] {
                    assert!(interp.interp(x).is_err(), "value at {x} should error");
                    assert!(
                        interp.interp_prime(x).is_err(),
                        "derivative at {x} should error"
                    );
                }

                assert!(interp.interp(1.5).unwrap().is_finite());
                assert!(interp.interp_prime(1.5).unwrap().is_finite());
            }
        }
    };
}

error_extrapolation_tests!(none_linear, Interpolator<LinearStrategy>);
error_extrapolation_tests!(none_log_linear, Interpolator<LogLinearStrategy>);
error_extrapolation_tests!(none_cubic_hermite, Interpolator<CubicHermiteStrategy>);
error_extrapolation_tests!(none_monotone_convex, Interpolator<MonotoneConvexStrategy>);
error_extrapolation_tests!(
    none_piecewise_quadratic_forward,
    Interpolator<PiecewiseQuadraticForwardStrategy>
);

macro_rules! non_finite_query_tests {
    ($mod_name:ident, $ty:ty) => {
        mod $mod_name {
            use super::*;

            #[test]
            fn non_finite_queries_error_under_every_policy() {
                for policy in [
                    ExtrapolationPolicy::Flat,
                    ExtrapolationPolicy::Extend,
                    ExtrapolationPolicy::Error,
                ] {
                    let interp =
                        new_interp!($ty, standard_knots(), standard_dfs(), policy).unwrap();
                    for x in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                        assert!(
                            interp.interp(x).is_err(),
                            "value at {x} under {policy:?} should error"
                        );
                        assert!(
                            interp.interp_prime(x).is_err(),
                            "derivative at {x} under {policy:?} should error"
                        );
                    }
                }
            }
        }
    };
}

non_finite_query_tests!(non_finite_linear, Interpolator<LinearStrategy>);
non_finite_query_tests!(non_finite_log_linear, Interpolator<LogLinearStrategy>);
non_finite_query_tests!(non_finite_cubic_hermite, Interpolator<CubicHermiteStrategy>);
non_finite_query_tests!(
    non_finite_monotone_convex,
    Interpolator<MonotoneConvexStrategy>
);
non_finite_query_tests!(
    non_finite_piecewise_quadratic_forward,
    Interpolator<PiecewiseQuadraticForwardStrategy>
);

// Derivative (interp_prime) Tests

macro_rules! derivative_tests {
    ($mod_name:ident, $ty:ty) => {
        mod $mod_name {
            use super::*;

            #[test]
            fn derivative_is_finite_interior() {
                let interp = new_interp!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::Flat
                )
                .unwrap();

                for &x in &[0.5, 1.0, 1.5, 2.0, 2.5] {
                    let deriv = interp.interp_prime(x).unwrap();
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
                let interp = new_interp!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::Flat
                )
                .unwrap();

                for &x in &[0.5, 1.5, 2.5] {
                    let deriv = interp.interp_prime(x).unwrap();
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
                let interp = new_interp!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::Flat
                )
                .unwrap();

                let h = 1e-6;
                for &x in &[0.5, 1.5, 2.5] {
                    let numerical =
                        (interp.interp(x + h).unwrap() - interp.interp(x - h).unwrap()) / (2.0 * h);
                    let analytical = interp.interp_prime(x).unwrap();

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
                let interp = new_interp!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::Flat
                )
                .unwrap();

                for &x in &[0.0, 1.0, 2.0, 3.0] {
                    let deriv = interp.interp_prime(x).unwrap();
                    assert!(
                        deriv.is_finite(),
                        "Derivative at knot {} should be finite",
                        x
                    );
                }
            }

            #[test]
            fn derivative_flat_extrapolation() {
                let interp = new_interp!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::Flat
                )
                .unwrap();

                let left_deriv = interp.interp_prime(-1.0).unwrap();
                let right_deriv = interp.interp_prime(5.0).unwrap();

                assert!(
                    approx_eq(left_deriv, 0.0, 1e-10),
                    "Flat left derivative should be 0, got {}",
                    left_deriv
                );
                assert!(
                    approx_eq(right_deriv, 0.0, 1e-10),
                    "Flat right derivative should be 0, got {}",
                    right_deriv
                );
            }

            #[test]
            fn derivative_extend_extrapolation() {
                let interp = new_interp!(
                    $ty,
                    standard_knots(),
                    standard_dfs(),
                    ExtrapolationPolicy::Extend
                )
                .unwrap();

                let left_deriv = interp.interp_prime(-1.0).unwrap();
                let right_deriv = interp.interp_prime(5.0).unwrap();

                assert!(
                    left_deriv.is_finite() && left_deriv != 0.0,
                    "Extend left derivative should be non-zero, got {}",
                    left_deriv
                );
                assert!(
                    right_deriv.is_finite() && right_deriv != 0.0,
                    "Extend right derivative should be non-zero, got {}",
                    right_deriv
                );
            }
        }
    };
}

derivative_tests!(deriv_linear, Interpolator<LinearStrategy>);
derivative_tests!(deriv_log_linear, Interpolator<LogLinearStrategy>);
derivative_tests!(deriv_cubic_hermite, Interpolator<CubicHermiteStrategy>);
derivative_tests!(deriv_monotone_convex, Interpolator<MonotoneConvexStrategy>);
derivative_tests!(
    deriv_piecewise_quadratic_forward,
    Interpolator<PiecewiseQuadraticForwardStrategy>
);

// Type-Specific Tests

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
        let interp = new_interp!(
            Interpolator<LinearStrategy>,
            knots.clone(),
            dfs.clone(),
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        for seg in 0..knots.len() - 1 {
            let t_mid = 0.5 * (knots[seg] + knots[seg + 1]);
            let expected = 0.5 * (dfs[seg] + dfs[seg + 1]);
            assert!((interp.interp(t_mid).unwrap() - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn derivative_at_segment_boundaries() {
        let interp = new_interp!(
            Interpolator<LinearStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        let h = 1e-8;
        for &x in &[1.0, 2.0] {
            let deriv = interp.interp_prime(x).unwrap();
            let left = (interp.interp(x).unwrap() - interp.interp(x - h).unwrap()) / h;
            assert!(
                approx_eq(deriv, left, 1e-4),
                "Left derivative at {} should match analytical segment derivative",
                x
            );
        }
    }
}

mod log_linear_forward_rate_specific {
    use super::*;

    #[test]
    fn geometric_midpoint() {
        let knots: Vec<f64> = vec![0.0, 1.0, 2.0, 3.0];
        let zero_rate = 0.03f64;
        let dfs: Vec<f64> = knots.iter().map(|&t| (-zero_rate * t).exp()).collect();
        let knots = knots.into_boxed_slice();
        let dfs = dfs.into_boxed_slice();

        let interp = new_interp!(
            Interpolator<LogLinearStrategy>,
            knots.clone(),
            dfs.clone(),
            ExtrapolationPolicy::Flat
        )
        .unwrap();
        for seg in 0..knots.len() - 1 {
            let t_mid = 0.5 * (knots[seg] + knots[seg + 1]);
            let expected = (dfs[seg].ln() * 0.5 + dfs[seg + 1].ln() * 0.5).exp();
            assert!((interp.interp(t_mid).unwrap() - expected).abs() < 1e-12);
        }
    }

    #[test]
    fn derivative_formula_consistency() {
        let interp = new_interp!(
            Interpolator<LogLinearStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        let h = 1e-7;
        for &x in &[0.5, 1.5, 2.5] {
            let analytical = interp.interp_prime(x).unwrap();
            let numerical =
                (interp.interp(x + h).unwrap() - interp.interp(x - h).unwrap()) / (2.0 * h);

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

mod log_linear_specific {
    use super::*;

    #[test]
    fn constant_forward_rate_property() {
        let interp = new_interp!(
            Interpolator<LogLinearStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        let t1 = 0.25;
        let t2 = 0.75;
        let df1 = interp.interp(t1).unwrap();
        let df2 = interp.interp(t2).unwrap();

        let implied_rate = -(df2.ln() - df1.ln()) / (t2 - t1);

        let t3 = 0.1;
        let t4 = 0.9;
        let df3 = interp.interp(t3).unwrap();
        let df4 = interp.interp(t4).unwrap();
        let implied_rate2 = -(df4.ln() - df3.ln()) / (t4 - t3);

        assert!(approx_eq(implied_rate, implied_rate2, 1e-10));
    }
}

mod cubic_hermite_specific {
    use super::*;

    #[test]
    fn derivative_numerical_consistency() {
        let interp = new_interp!(
            Interpolator<CubicHermiteStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        let h = 6e-6;
        let x = 1.5;
        let numerical = (interp.interp(x + h).unwrap() - interp.interp(x - h).unwrap()) / (2.0 * h);
        let analytical = interp.interp_prime(x).unwrap();

        let relative_error = (analytical - numerical).abs() / numerical.abs();
        assert!(
            relative_error < 1e-8,
            "Derivative error {} too large",
            relative_error
        );
    }

    #[test]
    fn derivative_monotonicity_preserved() {
        let interp = new_interp!(
            Interpolator<CubicHermiteStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        for &x in &[0.5, 1.5, 2.5] {
            let deriv = interp.interp_prime(x).unwrap();
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
        let interp = new_interp!(
            Interpolator<CubicHermiteStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        let derivative_at_knots = vec![
            interp.interp_prime(0.0).unwrap(),
            interp.interp_prime(1.0).unwrap(),
            interp.interp_prime(2.0).unwrap(),
            interp.interp_prime(3.0).unwrap(),
        ];

        for &deriv in &derivative_at_knots {
            assert!(deriv.is_finite());
            assert!(deriv <= 0.0);
        }
    }

    #[test]
    fn extrapolation_uses_boundary_slopes() {
        let interp = new_interp!(
            Interpolator<CubicHermiteStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::Extend
        )
        .unwrap();

        let left_deriv = interp.interp_prime(-1.0).unwrap();
        let first_deriv = interp.interp_prime(0.0).unwrap();
        assert!(
            approx_eq(left_deriv, first_deriv, 1e-10),
            "Left extrapolation slope should match boundary slope"
        );

        let right_deriv = interp.interp_prime(5.0).unwrap();
        let last_deriv = interp.interp_prime(3.0).unwrap();
        assert!(
            approx_eq(right_deriv, last_deriv, 1e-10),
            "Right extrapolation slope should match boundary slope"
        );
    }

    #[test]
    fn derivative_continuity_across_knots() {
        let interp = new_interp!(
            Interpolator<CubicHermiteStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        let h = 1e-8;
        for &x in &[1.0, 2.0] {
            let left_deriv = interp.interp_prime(x - h).unwrap();
            let right_deriv = interp.interp_prime(x + h).unwrap();
            let knot_deriv = interp.interp_prime(x).unwrap();

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
    /// supports negative-rate curves (EUR/CHF/JPY) where DF > 1 is legitimate.
    #[test]
    fn accepts_increasing_dfs_negative_rates() {
        let increasing_dfs = vec![0.9, 0.95, 1.0, 1.05].into_boxed_slice();
        let interp = new_interp!(
            Interpolator<MonotoneConvexStrategy>,
            standard_knots(),
            increasing_dfs,
            ExtrapolationPolicy::Flat
        )
        .expect("increasing DFs (negative rates) should build");
        let mid = interp.interp(0.5).unwrap();
        assert!(mid.is_finite() && mid > 0.0);
    }

    #[test]
    fn near_flat_curve_handled() {
        let near_flat = vec![1.0, 0.999999, 0.999998, 0.999997].into_boxed_slice();
        let interp = new_interp!(
            Interpolator<MonotoneConvexStrategy>,
            standard_knots(),
            near_flat,
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        let mid = interp.interp(0.5).unwrap();
        assert!(mid.is_finite());
        assert!(mid > 0.999997 && mid <= 1.0);
    }

    #[test]
    fn shape_preservation() {
        let interp = new_interp!(
            Interpolator<MonotoneConvexStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        let test_points = [0.25, 0.5, 0.75, 1.25, 1.5, 1.75, 2.25, 2.5, 2.75];
        let mut values = Vec::new();

        for &t in &test_points {
            let df = interp.interp(t).unwrap();
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
        let interp = new_interp!(
            Interpolator<MonotoneConvexStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        for &x in &[0.0, 1.0, 2.0, 3.0] {
            let deriv = interp.interp_prime(x).unwrap();
            assert!(deriv.is_finite());
            assert!(deriv <= 0.0);
        }
    }

    #[test]
    fn derivative_extrapolation_extend_left() {
        let interp = new_interp!(
            Interpolator<MonotoneConvexStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::Extend
        )
        .unwrap();

        for &x in &[-0.5, -1.0, -2.0] {
            let deriv = interp.interp_prime(x).unwrap();
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
    fn derivative_extrapolation_extend_right() {
        let interp = new_interp!(
            Interpolator<MonotoneConvexStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::Extend
        )
        .unwrap();

        for &x in &[4.0, 5.0, 10.0] {
            let deriv = interp.interp_prime(x).unwrap();
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
        let interp = new_interp!(
            Interpolator<MonotoneConvexStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        assert_eq!(interp.interp(0.0).unwrap().to_bits(), 1.0_f64.to_bits());
        assert_eq!(interp.interp(1.0).unwrap().to_bits(), 0.95_f64.to_bits());
        assert_eq!(interp.interp(2.0).unwrap().to_bits(), 0.9_f64.to_bits());
        assert_eq!(interp.interp(3.0).unwrap().to_bits(), 0.85_f64.to_bits());
    }

    #[test]
    fn extrapolation_extend_values_left() {
        let interp = new_interp!(
            Interpolator<MonotoneConvexStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::Extend
        )
        .unwrap();

        let left_val = interp.interp(-0.5).unwrap();
        assert!(left_val > 1.0, "Left extrapolated value should be > 1.0");
        assert!(left_val.is_finite());
    }

    #[test]
    fn extrapolation_extend_values_right() {
        let interp = new_interp!(
            Interpolator<MonotoneConvexStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::Extend
        )
        .unwrap();

        let right_val = interp.interp(4.0).unwrap();
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

        let interp = new_interp!(
            Interpolator<MonotoneConvexStrategy>,
            knots,
            steep_dfs,
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        for x in (0..30).map(|i| i as f64 * 0.1) {
            let val = interp.interp(x).unwrap();
            assert!(val > 0.0 && val.is_finite(), "Value at {} = {}", x, val);
        }
    }

    #[test]
    fn near_zero_slope_handling() {
        let knots = vec![0.0, 1.0, 2.0, 3.0].into_boxed_slice();
        let near_flat = vec![1.0, 0.9999, 0.9998, 0.9997].into_boxed_slice();

        let interp = new_interp!(
            Interpolator<MonotoneConvexStrategy>,
            knots,
            near_flat,
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        for x in (0..30).map(|i| i as f64 * 0.1) {
            let val = interp.interp(x).unwrap();
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

        let interp = new_interp!(
            Interpolator<MonotoneConvexStrategy>,
            knots,
            non_monotone.clone().into_boxed_slice(),
            ExtrapolationPolicy::Flat
        )
        .expect("non-monotone (mixed-sign forward) input should build");

        for (i, &df) in non_monotone.iter().enumerate() {
            assert_eq!(
                interp.interp(i as f64).unwrap().to_bits(),
                df.to_bits(),
                "knot {i} DF should preserve exact stored bits"
            );
        }
    }

    #[test]
    fn two_point_curve() {
        let knots = vec![0.0, 1.0].into_boxed_slice();
        let dfs = vec![1.0, 0.95].into_boxed_slice();

        let interp = new_interp!(
            Interpolator<MonotoneConvexStrategy>,
            knots,
            dfs,
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        assert_eq!(interp.interp(0.0).unwrap(), 1.0);
        assert_eq!(interp.interp(1.0).unwrap(), 0.95);

        let mid = interp.interp(0.5).unwrap();
        assert!(mid > 0.95 && mid < 1.0, "Midpoint should be between values");
    }

    #[test]
    fn five_point_curve() {
        let knots = vec![0.0, 0.5, 1.0, 2.0, 3.0].into_boxed_slice();
        let dfs = vec![1.0, 0.98, 0.95, 0.9, 0.85].into_boxed_slice();

        let interp = new_interp!(
            Interpolator<MonotoneConvexStrategy>,
            knots,
            dfs,
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        assert_eq!(interp.interp(0.0).unwrap(), 1.0);
        assert_eq!(interp.interp(0.5).unwrap(), 0.98);
        assert_eq!(interp.interp(1.0).unwrap(), 0.95);
        assert_eq!(interp.interp(2.0).unwrap(), 0.9);
        assert_eq!(interp.interp(3.0).unwrap(), 0.85);
    }

    #[test]
    fn zirp_preserves_curvature_at_left_boundary() {
        let fd0 = 0.001_f64;
        let fd1 = 0.005_f64;
        let fd2 = 0.001_f64;
        let df0 = 1.0_f64;
        let df1 = df0 * (-fd0).exp();
        let df2 = df1 * (-fd1).exp();
        let df3 = df2 * (-fd2).exp();

        let knots = vec![0.0, 1.0, 2.0, 3.0].into_boxed_slice();
        let dfs = vec![df0, df1, df2, df3].into_boxed_slice();
        let interp = new_interp!(
            Interpolator<MonotoneConvexStrategy>,
            knots,
            dfs,
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        let df_at_0 = interp.interp(0.0).unwrap();
        let dfp_at_0 = interp.interp_prime(0.0).unwrap();
        let fwd_at_0 = -dfp_at_0 / df_at_0;
        assert!(fwd_at_0.abs() < 1e-12);
    }

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
        let interp = new_interp!(
            Interpolator<MonotoneConvexStrategy>,
            knots,
            dfs,
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        let df_at_3 = interp.interp(3.0).unwrap();
        let dfp_at_3 = interp.interp_prime(3.0).unwrap();
        let fwd_at_3 = -dfp_at_3 / df_at_3;
        assert!(fwd_at_3.abs() < 1e-12);
    }

    #[test]
    fn uncollared_hagan_west_preserves_interval_averages() {
        // CORE-NUMERICS-03: no positivity collar; q = [0.01, 0.10, 0.01] on unit widths
        // yields uncollared boundary forwards F[0] = F[3] = -0.0125, F[1] = F[2] = 0.055.
        let knots = vec![0.0, 1.0, 2.0, 3.0].into_boxed_slice();
        let dfs = forwards_to_dfs(&[0.01, 0.10, 0.01], &[1.0, 1.0, 1.0]).into_boxed_slice();
        let interp = new_interp!(
            Interpolator<MonotoneConvexStrategy>,
            knots,
            dfs,
            ExtrapolationPolicy::Flat
        )
        .unwrap();
        let f0 = -interp.interp_prime(0.0).unwrap() / interp.interp(0.0).unwrap();
        let f1 = -interp.interp_prime(1.0).unwrap() / interp.interp(1.0).unwrap();
        let f2 = -interp.interp_prime(2.0).unwrap() / interp.interp(2.0).unwrap();
        let f3 = -interp.interp_prime(3.0).unwrap() / interp.interp(3.0).unwrap();
        assert!((f0 - (-0.0125)).abs() < 1e-14);
        assert!((f1 - 0.055).abs() < 1e-14);
        assert!((f2 - 0.055).abs() < 1e-14);
        assert!((f3 - (-0.0125)).abs() < 1e-14);
    }

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

// Trait Tests

mod traits {
    use super::*;

    #[test]
    fn interp_fn_default_derivative_for_linear_function() {
        let knots = vec![0.0, 1.0, 2.0].into_boxed_slice();
        let values = vec![1.0, 0.5, 0.1].into_boxed_slice();
        let interp = new_interp!(
            Interpolator<LinearStrategy>,
            knots,
            values,
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        let deriv_0_5 = interp.interp_prime(0.5).unwrap();
        let deriv_1_5 = interp.interp_prime(1.5).unwrap();

        assert!((deriv_0_5 - (-0.5)).abs() < 1e-6);
        assert!((deriv_1_5 - (-0.4)).abs() < 1e-6);
    }

    #[test]
    fn interp_fn_derivative_for_exponential_function() {
        let knots = vec![0.0, 1.0, 2.0].into_boxed_slice();
        let dfs = vec![1.0, 0.95, 0.9].into_boxed_slice();
        let interp = new_interp!(
            Interpolator<LogLinearStrategy>,
            knots,
            dfs,
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        let deriv_0_5 = interp.interp_prime(0.5).unwrap();
        let deriv_1_5 = interp.interp_prime(1.5).unwrap();

        assert!(deriv_0_5 < 0.0);
        assert!(deriv_1_5 < 0.0);
        assert!(deriv_0_5.is_finite());
        assert!(deriv_1_5.is_finite());
    }

    #[test]
    fn interp_fn_derivative_consistency() {
        let knots = vec![0.0, 1.0, 2.0, 3.0].into_boxed_slice();
        let values = vec![1.0, 0.8, 0.6, 0.4].into_boxed_slice();
        let interp = new_interp!(
            Interpolator<LinearStrategy>,
            knots,
            values,
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        let x = 1.5;
        let h = 6e-6;
        let numerical = (interp.interp(x + h).unwrap() - interp.interp(x - h).unwrap()) / (2.0 * h);
        let analytical = interp.interp_prime(x).unwrap();

        assert!((numerical - analytical).abs() < 1e-8);
    }

    #[test]
    fn interp_fn_derivative_at_boundaries() {
        let knots = vec![0.0, 1.0, 2.0].into_boxed_slice();
        let values = vec![1.0, 0.5, 0.25].into_boxed_slice();
        let interp = new_interp!(
            Interpolator<LinearStrategy>,
            knots,
            values,
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        let deriv_0 = interp.interp_prime(0.0).unwrap();
        let deriv_1 = interp.interp_prime(1.0).unwrap();
        let deriv_2 = interp.interp_prime(2.0).unwrap();

        assert!(deriv_0.is_finite());
        assert!(deriv_1.is_finite());
        assert!(deriv_2.is_finite());

        assert!(deriv_0 <= 0.0);
        assert!(deriv_1 <= 0.0);
        assert!(deriv_2 <= 0.0);
    }

    #[test]
    fn interp_fn_derivative_extrapolation_flat() {
        let knots = vec![0.0, 1.0].into_boxed_slice();
        let values = vec![1.0, 0.5].into_boxed_slice();
        let interp = new_interp!(
            Interpolator<LinearStrategy>,
            knots,
            values,
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        let deriv_below = interp.interp_prime(-0.5).unwrap();
        let deriv_above = interp.interp_prime(2.0).unwrap();

        assert_eq!(deriv_below, 0.0);
        assert_eq!(deriv_above, 0.0);
    }

    #[test]
    fn interp_fn_derivative_extrapolation_extend() {
        let knots = vec![0.0, 1.0].into_boxed_slice();
        let values = vec![1.0, 0.5].into_boxed_slice();
        let interp = new_interp!(
            Interpolator<LinearStrategy>,
            knots,
            values,
            ExtrapolationPolicy::Extend
        )
        .unwrap();

        let deriv_interior = interp.interp_prime(0.5).unwrap();
        let deriv_below = interp.interp_prime(-0.5).unwrap();
        let deriv_above = interp.interp_prime(2.0).unwrap();

        assert!((deriv_interior - deriv_below).abs() < 1e-6);
        assert!((deriv_interior - deriv_above).abs() < 1e-6);
    }
}

// Serialization Tests

mod serde_tests {
    use super::*;

    #[test]
    fn linear_df_roundtrip() {
        let knots = vec![0.0, 1.0, 2.0, 3.0, 5.0].into_boxed_slice();
        let dfs = vec![1.0, 0.98, 0.95, 0.92, 0.87].into_boxed_slice();

        let linear = new_interp!(
            Interpolator<LinearStrategy>,
            knots,
            dfs,
            ExtrapolationPolicy::Flat
        )
        .unwrap();
        let json = serde_json::to_string_pretty(&linear).unwrap();
        let deserialized: Interpolator<LinearStrategy> = serde_json::from_str(&json).unwrap();
        assert_eq!(
            linear.interp(1.5).unwrap(),
            deserialized.interp(1.5).unwrap()
        );
    }

    #[test]
    fn log_linear_df_roundtrip() {
        let knots = vec![0.0, 1.0, 2.0, 3.0, 5.0].into_boxed_slice();
        let dfs = vec![1.0, 0.98, 0.95, 0.92, 0.87].into_boxed_slice();

        let log_linear = new_interp!(
            Interpolator<LogLinearStrategy>,
            knots,
            dfs,
            ExtrapolationPolicy::Flat
        )
        .unwrap();
        let json = serde_json::to_string_pretty(&log_linear).unwrap();
        let deserialized: Interpolator<LogLinearStrategy> = serde_json::from_str(&json).unwrap();
        assert!(
            (log_linear.interp(1.5).unwrap() - deserialized.interp(1.5).unwrap()).abs() < 1e-10
        );
    }

    #[test]
    fn monotone_convex_roundtrip() {
        let knots = vec![0.0, 1.0, 2.0, 3.0, 5.0].into_boxed_slice();
        let dfs = vec![1.0, 0.98, 0.95, 0.92, 0.87].into_boxed_slice();

        let monotone = new_interp!(
            Interpolator<MonotoneConvexStrategy>,
            knots,
            dfs,
            ExtrapolationPolicy::Flat
        )
        .unwrap();
        let json = serde_json::to_string_pretty(&monotone).unwrap();
        let deserialized: Interpolator<MonotoneConvexStrategy> =
            serde_json::from_str(&json).unwrap();
        assert!((monotone.interp(1.5).unwrap() - deserialized.interp(1.5).unwrap()).abs() < 1e-10);
    }

    #[test]
    fn cubic_hermite_roundtrip() {
        let knots = vec![0.0, 1.0, 2.0, 3.0, 5.0].into_boxed_slice();
        let dfs = vec![1.0, 0.98, 0.95, 0.92, 0.87].into_boxed_slice();

        let cubic = new_interp!(
            Interpolator<CubicHermiteStrategy>,
            knots,
            dfs,
            ExtrapolationPolicy::Flat
        )
        .unwrap();
        let json = serde_json::to_string_pretty(&cubic).unwrap();
        let deserialized: Interpolator<CubicHermiteStrategy> = serde_json::from_str(&json).unwrap();
        assert!((cubic.interp(1.5).unwrap() - deserialized.interp(1.5).unwrap()).abs() < 1e-10);
    }

    #[test]
    fn flat_fwd_roundtrip() {
        let knots = vec![0.0, 1.0, 2.0, 3.0, 5.0].into_boxed_slice();
        let dfs = vec![1.0, 0.98, 0.95, 0.92, 0.87].into_boxed_slice();

        let flat_fwd = new_interp!(
            Interpolator<LogLinearStrategy>,
            knots,
            dfs,
            ExtrapolationPolicy::Flat
        )
        .unwrap();
        let json = serde_json::to_string_pretty(&flat_fwd).unwrap();
        let deserialized: Interpolator<LogLinearStrategy> = serde_json::from_str(&json).unwrap();
        assert!((flat_fwd.interp(1.5).unwrap() - deserialized.interp(1.5).unwrap()).abs() < 1e-10);
    }
}

// ExtrapolationPolicy Tests

#[cfg(test)]
mod extrapolation_policy_tests {
    use super::*;

    #[test]
    fn flat_behavior() {
        let interp = new_interp!(
            Interpolator<LinearStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::Flat
        )
        .unwrap();

        let val = interp.interp(-1.0).unwrap();
        assert!(approx_eq(val, 1.0, 1e-10), "Should use boundary value");

        let val_high = interp.interp(10.0).unwrap();
        assert!(
            approx_eq(val_high, 0.85, 1e-10),
            "Should use boundary value"
        );
    }

    #[test]
    fn extend_behavior() {
        let interp = new_interp!(
            Interpolator<LinearStrategy>,
            standard_knots(),
            standard_dfs(),
            ExtrapolationPolicy::Extend
        )
        .unwrap();

        assert!(interp.interp(-1.0).unwrap().is_finite());
        assert!(interp.interp(10.0).unwrap().is_finite());
    }

    #[test]
    fn extrapolation_policy_serialization() {
        let flat = ExtrapolationPolicy::Flat;
        let json = serde_json::to_string(&flat).unwrap();
        assert_eq!(json, r#""flat""#);
        assert_eq!(
            serde_json::from_str::<ExtrapolationPolicy>(&json).unwrap(),
            ExtrapolationPolicy::Flat
        );

        let extend = ExtrapolationPolicy::Extend;
        let json = serde_json::to_string(&extend).unwrap();
        assert_eq!(json, r#""extend""#);
        assert_eq!(
            serde_json::from_str::<ExtrapolationPolicy>(&json).unwrap(),
            ExtrapolationPolicy::Extend
        );

        let error = ExtrapolationPolicy::Error;
        let json = serde_json::to_string(&error).unwrap();
        assert_eq!(json, r#""error""#);
        assert_eq!(
            serde_json::from_str::<ExtrapolationPolicy>(&json).unwrap(),
            ExtrapolationPolicy::Error
        );
    }
}

// Uncollared Monotone-Convex interval-average property tests (CORE-NUMERICS-03)

mod monotone_convex_interval_averages {
    use super::*;
    use finstack_quant_core::math::random::{Pcg64Rng, RandomNumberGenerator};

    #[test]
    fn interval_average_forwards_are_preserved_across_rate_regimes() {
        let knots: [f64; 4] = [0.0, 0.125, 1.0, 7.0];
        let regimes: [f64; 6] = [0.0, 1e-8, 1e-4, 0.005, 0.03, 0.2];
        for first in regimes {
            for middle in regimes {
                for last in regimes {
                    let forwards = [first, middle, last];
                    let mut dfs = vec![1.0];
                    for (i, forward) in forwards.iter().enumerate() {
                        dfs.push(dfs[i] * (-forward * (knots[i + 1] - knots[i])).exp());
                    }
                    let interp = new_interp!(
                        Interpolator<MonotoneConvexStrategy>,
                        knots.to_vec().into_boxed_slice(),
                        dfs.clone().into_boxed_slice(),
                        ExtrapolationPolicy::Extend
                    )
                    .expect("valid positive DFs");

                    for (i, &df) in dfs.iter().enumerate() {
                        assert_eq!(interp.interp(knots[i]).unwrap().to_bits(), df.to_bits());
                    }
                    for i in 0..forwards.len() {
                        let width = knots[i + 1] - knots[i];
                        let recovered = -(interp.interp(knots[i + 1]).unwrap()
                            / interp.interp(knots[i]).unwrap())
                        .ln()
                            / width;
                        assert!(
                            (recovered - forwards[i]).abs() < 1e-13,
                            "interval {i} average mismatch: {recovered} vs {}",
                            forwards[i]
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn stressed_curves_build_and_preserve_positive_dfs_and_integrals() {
        for seed in [7_u64, 42, 1234, 987_654] {
            let mut rng = Pcg64Rng::new(seed, 0);
            for curve_idx in 0..50 {
                let n = 4 + (rng.uniform() * 7.0) as usize;
                let mut knots = Vec::with_capacity(n);
                let mut t = 0.0;
                knots.push(t);
                for _ in 1..n {
                    t += 0.05 + 1.95 * rng.uniform();
                    knots.push(t);
                }

                let mut dfs = Vec::with_capacity(n);
                dfs.push(1.0_f64);
                for i in 1..n {
                    let fd = if rng.uniform() < 0.5 {
                        0.0005 + 0.002 * rng.uniform()
                    } else {
                        0.08 + 0.12 * rng.uniform()
                    };
                    let dt = knots[i] - knots[i - 1];
                    dfs.push(dfs[i - 1] * (-fd * dt).exp());
                }

                let interp = new_interp!(
                    Interpolator<MonotoneConvexStrategy>,
                    knots.clone().into(),
                    dfs.clone().into(),
                    ExtrapolationPolicy::Extend
                )
                .expect("stressed curve should build");

                for i in 0..n - 1 {
                    let avg_expected = -(dfs[i + 1] / dfs[i]).ln() / (knots[i + 1] - knots[i]);
                    let mid = 0.5 * (knots[i] + knots[i + 1]);
                    let df_mid = interp.interp(mid).unwrap();
                    assert!(
                        df_mid.is_finite() && df_mid > 0.0,
                        "non-positive DF at seed={seed}, curve={curve_idx}"
                    );
                    let avg_actual = -(interp.interp(knots[i + 1]).unwrap()
                        / interp.interp(knots[i]).unwrap())
                    .ln()
                        / (knots[i + 1] - knots[i]);
                    assert!((avg_actual - avg_expected).abs() < 1e-13);
                }
            }
        }
    }
}

mod monotone_convex_negative_rates {
    use super::*;
    use finstack_quant_core::math::random::{Pcg64Rng, RandomNumberGenerator};

    /// Sample the instantaneous forward f(t) = -DF'(t)/DF(t).
    fn fwd_at(interp: &Interpolator<MonotoneConvexStrategy>, t: f64) -> f64 {
        -interp.interp_prime(t).unwrap() / interp.interp(t).unwrap()
    }

    #[test]
    fn flat_negative_one_percent_curve() {
        let rate = -0.01_f64;
        let knot_times = [0.0, 1.0, 2.0, 3.0, 5.0, 10.0];
        let dfs: Vec<f64> = knot_times.iter().map(|&t| (-rate * t).exp()).collect();

        let interp = new_interp!(
            Interpolator<MonotoneConvexStrategy>,
            knot_times.to_vec().into_boxed_slice(),
            dfs.clone().into_boxed_slice(),
            ExtrapolationPolicy::Extend
        )
        .expect("flat -1% curve (DF > 1) should build");

        for (&t, &df) in knot_times.iter().zip(dfs.iter()) {
            assert_eq!(interp.interp(t).unwrap().to_bits(), df.to_bits());
        }

        for i in 0..80 {
            let t = 0.05 + 10.0 * (i as f64) / 80.0;
            if t > 10.0 {
                break;
            }
            let df = interp.interp(t).unwrap();
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

    #[test]
    fn mixed_sign_forwards_not_clamped() {
        let fds = [0.005_f64, -0.003, 0.002];
        let knot_times = [0.0_f64, 1.0, 2.0, 3.0];
        let mut dfs = vec![1.0_f64];
        for (i, fd) in fds.iter().enumerate() {
            let dt = knot_times[i + 1] - knot_times[i];
            dfs.push(dfs[i] * (-fd * dt).exp());
        }

        let interp = new_interp!(
            Interpolator<MonotoneConvexStrategy>,
            knot_times.to_vec().into_boxed_slice(),
            dfs.clone().into_boxed_slice(),
            ExtrapolationPolicy::Extend
        )
        .expect("mixed-sign forward curve should build");

        for (&t, &df) in knot_times.iter().zip(dfs.iter()) {
            assert_eq!(interp.interp(t).unwrap().to_bits(), df.to_bits());
        }

        for (i, fd) in fds.iter().enumerate() {
            let (t0, t1) = (knot_times[i], knot_times[i + 1]);
            let avg = (interp.interp(t0).unwrap() / interp.interp(t1).unwrap()).ln() / (t1 - t0);
            assert!(
                approx_eq(avg, *fd, 1e-10),
                "segment {i} average forward {avg} should equal discrete {fd}"
            );
        }

        let fwd_mid = fwd_at(&interp, 1.5);
        assert!(
            fwd_mid < -0.003,
            "midpoint forward of -30bp segment should be negative, got {fwd_mid}"
        );
    }

    #[test]
    fn negative_rate_curves_interpolate_faithfully() {
        for seed in [7_u64, 42, 1234, 987_654] {
            let mut rng = Pcg64Rng::new(seed, 0);

            for curve_idx in 0..50 {
                let n = 4 + (rng.uniform() * 7.0) as usize;
                let mut knots = Vec::with_capacity(n);
                let mut t = 0.0;
                knots.push(t);
                for _ in 1..n {
                    t += 0.05 + 1.95 * rng.uniform();
                    knots.push(t);
                }

                let mut dfs = vec![1.0_f64];
                for i in 1..n {
                    let fd = -(0.0005 + 0.0145 * rng.uniform());
                    let dt = knots[i] - knots[i - 1];
                    dfs.push(dfs[i - 1] * (-fd * dt).exp());
                }

                let interp = new_interp!(
                    Interpolator<MonotoneConvexStrategy>,
                    knots.clone().into(),
                    dfs.clone().into(),
                    ExtrapolationPolicy::Extend
                )
                .expect("negative-rate curve should build");

                for (i, &df) in dfs.iter().enumerate() {
                    assert_eq!(interp.interp(knots[i]).unwrap().to_bits(), df.to_bits());
                }

                let t_max = knots[n - 1];
                let mut min_fwd = f64::INFINITY;
                for s in 0..=200 {
                    let x = t_max * (s as f64) / 200.0;
                    let df = interp.interp(x).unwrap();
                    let fwd = -interp.interp_prime(x).unwrap() / df;
                    assert!(df.is_finite() && df > 0.0 && fwd.is_finite());
                    min_fwd = min_fwd.min(fwd);
                }
                assert!(
                    min_fwd < -0.0005,
                    "forwards were clamped (min {min_fwd}; seed={seed}, curve={curve_idx})"
                );
            }
        }
    }
}

/// Numerical fixtures derived from Hagan & West (2008), equations (22)-(34).
mod hagan_west_reference {
    use super::*;

    fn from_forwards(knots: &[f64], forwards: &[f64]) -> Interpolator<MonotoneConvexStrategy> {
        let mut log_discount = 0.0;
        let mut dfs = vec![1.0];
        for (interval, &forward) in knots.windows(2).zip(forwards) {
            log_discount += forward * (interval[1] - interval[0]);
            dfs.push((-log_discount).exp());
        }
        Interpolator::<MonotoneConvexStrategy>::new(
            knots.to_vec().into(),
            dfs.into(),
            ExtrapolationPolicy::Extend,
        )
        .unwrap()
    }

    fn forward(interp: &Interpolator<MonotoneConvexStrategy>, t: f64) -> f64 {
        -interp.interp_prime(t).unwrap() / interp.interp(t).unwrap()
    }

    #[test]
    fn all_four_regions_match_reference_forwards_and_integrals() {
        let fixtures = [
            (-1.0, 1.0, 0.25, -0.5, -0.1875),
            (-1.0, 1.0, 0.50, 0.0, -0.25),
            (-1.0, 1.0, 0.75, 0.5, -0.1875),
            (-1.0, 3.0, 0.125, -1.0, -0.125),
            (-1.0, 3.0, 0.25, -1.0, -0.25),
            (-1.0, 3.0, 0.50, -5.0 / 9.0, -25.0 / 54.0),
            (-1.0, 3.0, 0.75, 7.0 / 9.0, -49.0 / 108.0),
            (-3.0, 1.0, 0.25, -7.0 / 9.0, -49.0 / 108.0),
            (-3.0, 1.0, 0.50, 5.0 / 9.0, -25.0 / 54.0),
            (-3.0, 1.0, 0.75, 1.0, -0.25),
            (-3.0, 1.0, 0.875, 1.0, -0.125),
            (1.0, 3.0, 0.25, 1.0 / 36.0, 13.0 / 108.0),
            (1.0, 3.0, 0.50, -5.0 / 9.0, 5.0 / 108.0),
            (1.0, 3.0, 0.75, -0.75, -0.125),
            (1.0, 3.0, 0.875, 3.0 / 16.0, -23.0 / 128.0),
        ];
        for (g0, g1, x, expected_g, expected_integral) in fixtures {
            for sign in [-1.0, 1.0] {
                let mean = sign * 0.1;
                let left = mean + sign * 0.02 * g0;
                let right = mean + sign * 0.02 * g1;
                let interp = from_forwards(&[0.0, 1.0, 2.0, 3.0], &[left, mean, right]);
                let actual_forward = forward(&interp, 1.0 + x);
                let actual_integral =
                    -(interp.interp(1.0 + x).unwrap() / interp.interp(1.0).unwrap()).ln();
                let expected_forward = mean + sign * 0.01 * expected_g;
                let expected_total = mean * x + sign * 0.01 * expected_integral;
                assert!(
                    (actual_forward - expected_forward).abs() < 2e-14,
                    "g0={g0}, g1={g1}, x={x}, sign={sign}: {actual_forward} != {expected_forward}"
                );
                assert!(
                    (actual_integral - expected_total).abs() < 2e-14,
                    "g0={g0}, g1={g1}, x={x}, sign={sign}: integral {actual_integral} != {expected_total}"
                );
                let h = 1e-5;
                let numeric = (interp.interp(1.0 + x + h).unwrap()
                    - interp.interp(1.0 + x - h).unwrap())
                    / (2.0 * h);
                assert!((interp.interp_prime(1.0 + x).unwrap() - numeric).abs() < 2e-10);
            }
        }
    }

    #[test]
    fn increasing_discrete_forwards_have_no_interior_reversal() {
        let interp = from_forwards(&[0.0, 1.0, 2.0, 3.0], &[0.01, 0.02, 0.05]);
        assert!((forward(&interp, 1.1) - 0.015).abs() < 1e-14);
        assert!((interp.interp(1.25).unwrap() - (-0.01375_f64).exp()).abs() < 1e-14);
        let mut previous = forward(&interp, 1.0);
        for step in 1..=100 {
            let current = forward(&interp, 1.0 + f64::from(step) / 100.0);
            assert!(current >= previous - 1e-14);
            previous = current;
        }
    }

    #[test]
    fn boundary_forwards_use_adjacent_instantaneous_rates() {
        for (knots, expected) in [
            ([0.0, 1.0, 2.0, 3.0], [0.0075, 0.015, 0.035, 0.0575]),
            ([0.0, 0.5, 2.0, 5.0], [0.00875, 0.0125, 0.03, 0.06]),
        ] {
            let interp = from_forwards(&knots, &[0.01, 0.02, 0.05]);
            for (&t, expected_forward) in knots.iter().zip(expected) {
                assert!((forward(&interp, t) - expected_forward).abs() < 2e-14);
            }
            assert!((forward(&interp, -0.5) - expected[0]).abs() < 2e-14);
            assert!((forward(&interp, knots[3] + 5.0) - expected[3]).abs() < 2e-14);
        }
    }

    #[test]
    fn regional_joins_and_boundaries_are_continuous() {
        for (g0, g1) in [(-0.01, 0.02), (-0.02, 0.01), (0.0, 0.03), (0.03, 0.0)] {
            for sign in [-1.0, 1.0] {
                let curves: Vec<_> = [-1e-10, 0.0, 1e-10]
                    .into_iter()
                    .map(|shift| {
                        let mean = sign * 0.1;
                        from_forwards(
                            &[0.0, 1.0, 2.0, 3.0],
                            &[
                                mean + 2.0 * sign * (g0 + shift),
                                mean,
                                mean + 2.0 * sign * (g1 + shift),
                            ],
                        )
                    })
                    .collect();
                for x in [0.1, 0.25, 0.5, 0.75, 0.9] {
                    for perturbed in [&curves[0], &curves[2]] {
                        assert!(
                            (forward(perturbed, 1.0 + x) - forward(&curves[1], 1.0 + x)).abs()
                                < 2e-9
                        );
                        assert!(
                            (perturbed.interp(1.0 + x).unwrap()
                                - curves[1].interp(1.0 + x).unwrap())
                            .abs()
                                < 2e-9
                        );
                    }
                }
            }
        }
        for (forwards, join) in [
            ([0.08, 0.1, 0.16], 1.25),
            ([0.04, 0.1, 0.12], 1.75),
            ([0.12, 0.1, 0.16], 1.75),
        ] {
            let interp = from_forwards(&[0.0, 1.0, 2.0, 3.0], &forwards);
            assert!((forward(&interp, join - 1e-8) - forward(&interp, join + 1e-8)).abs() < 1e-10);
            assert!(
                (interp.interp(join - 1e-8).unwrap() - interp.interp(join + 1e-8).unwrap()).abs()
                    < 3e-9
            );
        }
    }
}

mod validated_interpolator_serde {
    use super::*;
    use serde_json::json;

    #[test]
    fn deserialization_rejects_invalid_shapes_and_positive_constraints() {
        for (knots, values) in [
            (json!([]), json!([])),
            (json!([0.0]), json!([1.0])),
            (json!([0.0, 1.0]), json!([1.0])),
            (json!([0.0, 0.0]), json!([1.0, 0.9])),
            (json!([1.0, 0.0]), json!([1.0, 0.9])),
            (json!([0.0, null]), json!([1.0, 0.9])),
        ] {
            let raw = json!({"knots":knots,"values":values,"extrapolation":"flat"});
            assert!(serde_json::from_value::<Interpolator<LinearStrategy>>(raw).is_err());
        }

        // Legacy validation/extrapolation keys must be rejected cleanly.
        let legacy_policy =
            json!({"knots":[0.0,1.0],"values":[1.0,0.9],"extrapolation":"flat_zero"});
        assert!(serde_json::from_value::<Interpolator<LinearStrategy>>(legacy_policy).is_err());
        let legacy_val = json!({"knots":[0.0,1.0],"values":[1.0,0.9],"extrapolation":"flat","validation":"strict"});
        assert!(serde_json::from_value::<Interpolator<LinearStrategy>>(legacy_val).is_err());
    }

    #[test]
    fn deserialization_rebuilds_caches_for_every_strategy() {
        macro_rules! check_strategy {
            ($strategy:ty) => {{
                let raw = json!({"knots":[0.0,1.0,2.0],"values":[1.0,0.9,0.7],"extrapolation":"extend"});
                let original: Interpolator<$strategy> = serde_json::from_value(raw).unwrap();
                let mut saved = serde_json::to_value(&original).unwrap();
                assert!(saved.get("strategy").is_none());
                assert!(saved.get("validation").is_none());
                saved["values"][1] = json!(0.8);
                let rebuilt: Interpolator<$strategy> = serde_json::from_value(saved).unwrap();
                assert_eq!(rebuilt.interp(1.0).unwrap().to_bits(), 0.8_f64.to_bits());
                let fresh = Interpolator::<$strategy>::new(
                    vec![0.0, 1.0, 2.0].into(),
                    vec![1.0, 0.8, 0.7].into(),
                    ExtrapolationPolicy::Extend,
                )
                .unwrap();
                for x in [-0.2, 0.4, 1.0, 1.5, 3.0] {
                    assert!((rebuilt.interp(x).unwrap() - fresh.interp(x).unwrap()).abs() < 1e-14);
                    assert!(
                        (rebuilt.interp_prime(x).unwrap() - fresh.interp_prime(x).unwrap()).abs()
                            < 1e-14
                    );
                }
            }};
        }
        check_strategy!(LinearStrategy);
        check_strategy!(LogLinearStrategy);
        check_strategy!(CubicHermiteStrategy);
        check_strategy!(MonotoneConvexStrategy);
        check_strategy!(PiecewiseQuadraticForwardStrategy);
    }

    #[test]
    fn monotone_convex_region_iv_axis_has_continuous_endpoint_forward_and_extrapolation() {
        // Y = [0, 1, 2, 4] => q = [1, 1, 2], provisional f = [1, 1, 1.5, 2.25].
        // Interval [1, 2] lies on the Region IV zero-deviation axis (g_left = 0, g_right = 0.5)
        // so interior and left-limit forward at t = 2 equal 1.0 -> D'(2) = -exp(-2).
        let knots = vec![0.0, 1.0, 2.0, 3.0].into_boxed_slice();
        let dfs =
            vec![1.0, (-1.0_f64).exp(), (-2.0_f64).exp(), (-4.0_f64).exp()].into_boxed_slice();
        let interp =
            Interpolator::<MonotoneConvexStrategy>::new(knots, dfs, ExtrapolationPolicy::Extend)
                .expect("valid monotone convex curve");

        let eps = 1e-9;
        let expected_left_limit = -(-2.0_f64).exp();
        let prime_left_of_2 = interp.interp_prime(2.0 - eps).unwrap();
        let prime_at_2 = interp.interp_prime(2.0).unwrap();
        assert!((prime_left_of_2 - expected_left_limit).abs() < 1e-8);
        assert!((prime_at_2 - expected_left_limit).abs() < 1e-14);

        // Right-boundary degenerate axis: Y = [0, 2, 3, 4] => q = [2, 1, 1], f = [2.25, 1.5, 1, 1],
        // so segment [2, 3] has actual forward 1.0 and Extend right slope matches -exp(-4).
        let right_knots = vec![0.0, 1.0, 2.0, 3.0].into_boxed_slice();
        let right_dfs =
            vec![1.0, (-2.0_f64).exp(), (-3.0_f64).exp(), (-4.0_f64).exp()].into_boxed_slice();
        let right_interp = Interpolator::<MonotoneConvexStrategy>::new(
            right_knots,
            right_dfs,
            ExtrapolationPolicy::Extend,
        )
        .expect("valid right axis curve");
        let prime_inside = right_interp.interp_prime(3.0 - eps).unwrap();
        let prime_at_end = right_interp.interp_prime(3.0).unwrap();
        let prime_outside = right_interp.interp_prime(3.0 + eps).unwrap();
        assert!((prime_inside - prime_at_end).abs() < 1e-8);
        assert!((prime_inside - prime_outside).abs() < 1e-8);
    }

    #[test]
    fn cubic_hermite_pchip_handles_extreme_knot_and_secant_scales() {
        let large_knots = vec![-1.0e308, 0.0, 1.0e308].into_boxed_slice();
        let large_vals = vec![0.0, 1.0, 2.0].into_boxed_slice();
        let large_interp = Interpolator::<CubicHermiteStrategy>::new(
            large_knots,
            large_vals,
            ExtrapolationPolicy::Extend,
        )
        .expect("extreme knot scale should build");
        assert!((large_interp.interp(0.5e308).unwrap() - 1.5).abs() < 1e-14);
        let p_mid = large_interp.interp_prime(0.5e308).unwrap();
        assert!(p_mid.is_finite() && p_mid > 0.0);

        // PCHIP plateau reference: x=[0,1,2], y=[0,1,1] => slopes [1.5, 0, 0], midpoint 0.6875.
        let pchip = Interpolator::<CubicHermiteStrategy>::new(
            vec![0.0, 1.0, 2.0].into(),
            vec![0.0, 1.0, 1.0].into(),
            ExtrapolationPolicy::Flat,
        )
        .unwrap();
        assert!((pchip.interp(0.5).unwrap() - 0.6875).abs() < 1e-15);
        assert!((pchip.interp_prime(0.0).unwrap() - 1.5).abs() < 1e-15);
        assert_eq!(pchip.interp_prime(1.0).unwrap(), 0.0);
        assert_eq!(pchip.interp_prime(2.0).unwrap(), 0.0);

        // Subnormal secants whose reciprocal overflows f64::MAX are preserved without false flat.
        let min_sub = f64::from_bits(1);
        let sub_slope = Interpolator::<CubicHermiteStrategy>::new(
            vec![0.0, 1.0, 2.0].into(),
            vec![0.0, min_sub, 2.0 * min_sub].into(),
            ExtrapolationPolicy::Flat,
        )
        .unwrap();
        assert!(sub_slope.interp_prime(0.5).unwrap() > 0.0);

        let eta = f64::from_bits(4);
        let sub = Interpolator::<CubicHermiteStrategy>::new(
            vec![0.0, 1.0, 2.0].into(),
            vec![0.0, eta, 2.0 * eta].into(),
            ExtrapolationPolicy::Flat,
        )
        .unwrap();
        assert!(sub.interp(0.5).unwrap() > 0.0);
        assert!(sub.interp_prime(0.5).unwrap() > 0.0);

        // Non-finite segment slope fails closed.
        assert!(
            Interpolator::<CubicHermiteStrategy>::new(
                vec![0.0, 1.0e-308, 2.0].into(),
                vec![0.0, 1.0e308, 2.0].into(),
                ExtrapolationPolicy::Flat,
            )
            .is_err()
        );
    }

    #[test]
    fn core_numerics_02_and_03_contract_edge_cases() {
        // 1. Linear extreme signed blend succeeds at midpoint while overflowing derivative errors.
        let extreme_linear = Interpolator::<LinearStrategy>::new(
            vec![0.0, 1.0].into(),
            vec![-f64::MAX, f64::MAX].into(),
            ExtrapolationPolicy::Flat,
        )
        .unwrap();
        assert_eq!(extreme_linear.interp(0.5).unwrap(), 0.0);
        assert_eq!(
            extreme_linear.interp(0.0).unwrap().to_bits(),
            (-f64::MAX).to_bits()
        );
        assert!(extreme_linear.interp_prime(0.5).is_err());

        // 2. Signed zero bit preservation on exact knot hits.
        let neg_zero_lin = Interpolator::<LinearStrategy>::new(
            vec![0.0, 1.0].into(),
            vec![-0.0, 1.0].into(),
            ExtrapolationPolicy::Flat,
        )
        .unwrap();
        assert_eq!(
            neg_zero_lin.interp(0.0).unwrap().to_bits(),
            (-0.0_f64).to_bits()
        );

        // 3. Natural spline on tiny 1e-18 widths succeeds without pivot < EPS false rejection,
        // and allows negative forwards (Y = [0, 0.01, 0.10] -> M1 = 0.12).
        let tiny_pqf = Interpolator::<PiecewiseQuadraticForwardStrategy>::new(
            vec![0.0, 1.0e-18, 2.0e-18].into(),
            vec![1.0, (-1.0e-20_f64).exp(), (-2.0e-20_f64).exp()].into(),
            ExtrapolationPolicy::Extend,
        )
        .expect("scaled tridiagonal solve must handle 1e-18 knot widths");
        assert!((tiny_pqf.interp(0.5e-18).unwrap() - 1.0).abs() < 1e-15);

        let neg_fwd_pqf = Interpolator::<PiecewiseQuadraticForwardStrategy>::new(
            vec![0.0, 1.0, 2.0].into(),
            vec![1.0, (-0.01_f64).exp(), (-0.10_f64).exp()].into(),
            ExtrapolationPolicy::Flat,
        )
        .unwrap();
        // Forward at t = 0 is -0.01 + 0.06 * 0^2 = -0.01 (negative forward without clamping).
        let fwd0 = -neg_fwd_pqf.interp_prime(0.0).unwrap() / neg_fwd_pqf.interp(0.0).unwrap();
        assert!((fwd0 - (-0.01)).abs() < 1e-14);
    }

    #[test]
    fn serialized_strategy_cache_is_rejected() {
        let raw = json!({"knots":[0.0,1.0],"values":[1.0,0.9],"strategy":{"log_diffs":[-0.1]},"extrapolation":"flat"});
        assert!(serde_json::from_value::<Interpolator<LogLinearStrategy>>(raw).is_err());
    }

    #[test]
    fn signed_linear_roundtrips_while_positive_strategies_reject_negative_values() {
        let raw = json!({"knots":[0.0,1.0],"values":[-0.01,0.02],"extrapolation":"flat"});
        let interp: Interpolator<LinearStrategy> = serde_json::from_value(raw.clone()).unwrap();
        let saved = serde_json::to_value(interp).unwrap();
        let restored: Interpolator<LinearStrategy> = serde_json::from_value(saved).unwrap();
        assert!((restored.interp(0.5).unwrap() - 0.005).abs() < 1e-14);
        assert!(serde_json::from_value::<Interpolator<LogLinearStrategy>>(raw.clone()).is_err());
        assert!(
            serde_json::from_value::<Interpolator<MonotoneConvexStrategy>>(raw.clone()).is_err()
        );
        assert!(
            serde_json::from_value::<Interpolator<PiecewiseQuadraticForwardStrategy>>(raw).is_err()
        );
    }
}
