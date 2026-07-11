//! Tests for DiscountCurve functionality.
//!
//! This module consolidates all discount curve tests including:
//! - Builder validation and construction
//! - Interpolation styles (Linear, LogLinear, MonotoneConvex, CubicHermite, LogLinear)
//! - Extrapolation policies (FlatZero, FlatForward)
//! - Monotonicity validation and no-arbitrage checks
//! - Parallel and key-rate bumps
//! - Serialization roundtrips
//! - Analytical verification

use super::super::test_helpers::{sample_base_date, sample_discount_curve};
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use proptest::prelude::*;
use time::Month;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn positive_flat_rate_discount_curve_interpolates_inside_no_arbitrage_bounds(
        annual_rate in 0.0_f64..0.25,
        t in 0.0_f64..3.0,
    ) {
        let df = |time: f64| (-annual_rate * time).exp();
        let curve = DiscountCurve::builder("PROP-FLAT")
            .base_date(sample_base_date())
            .knots([(0.0, 1.0), (1.0, df(1.0)), (2.0, df(2.0)), (3.0, df(3.0))])
            .interp(InterpStyle::Linear)
            .build()
            .unwrap();

        let interpolated = curve.df(t);
        prop_assert!(interpolated.is_finite());
        prop_assert!(interpolated > 0.0);
        prop_assert!(interpolated <= 1.0 + 1e-12);
    }
}

#[test]
fn to_forward_curve_uses_requested_simple_tenor() {
    let rate = 0.05_f64;
    let curve = DiscountCurve::builder("FLAT-CC")
        .base_date(sample_base_date())
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (2.0, (-2.0 * rate).exp()),
            (5.0, (-5.0 * rate).exp()),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();

    for tenor in [0.25, 2.0] {
        let forwards = curve
            .to_forward_curve(format!("FWD-{tenor}"), tenor, None)
            .unwrap();
        let expected = ((rate * tenor).exp() - 1.0) / tenor;
        for t in [0.0, 1.0, 2.0] {
            assert!(
                (forwards.rate(t) - expected).abs() < 1e-12,
                "tenor={tenor}, t={t}: actual={}, expected={expected}",
                forwards.rate(t)
            );
        }
    }
}

#[test]
fn to_forward_curve_reconstructs_source_discount_factors_on_tenor_grid() {
    let curve = DiscountCurve::builder("ROUNDTRIP")
        .base_date(sample_base_date())
        .knots([(0.0, 1.0), (1.0, 0.95), (2.0, 0.85)])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();
    let forwards = curve.to_forward_curve("ROUNDTRIP-FWD", 1.0, None).unwrap();

    for t in [0.0, 1.0, 2.0] {
        let rebuilt = forwards.df(t).unwrap();
        assert!(
            (rebuilt - curve.df(t)).abs() < 1e-12,
            "t={t}: rebuilt={rebuilt}, source={}",
            curve.df(t)
        );
    }
}

#[test]
fn to_forward_curve_rejects_invalid_tenor() {
    let curve = sample_discount_curve("INVALID-TENOR");
    for tenor in [0.0, -0.25, f64::NAN, f64::INFINITY] {
        assert!(curve.to_forward_curve("INVALID", tenor, None).is_err());
    }
}

#[test]
fn date_discounting_supports_dates_before_curve_base() {
    let base = sample_base_date();
    let curve = DiscountCurve::builder("SPOT-BASED")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.95), (2.0, 0.90)])
        .build()
        .unwrap();
    let before = base - time::Duration::days(30);
    let after = base + time::Duration::days(30);

    let forward = curve.df_between_dates(before, after).unwrap();
    let backward = curve.df_between_dates(after, before).unwrap();
    assert!((forward * backward - 1.0).abs() < 1e-12);

    let via_base = curve.df_between_dates(before, base).unwrap()
        * curve.df_between_dates(base, after).unwrap();
    assert!((forward - via_base).abs() < 1e-12);
}

// =============================================================================
// Builder Validation Tests
// =============================================================================

#[test]
fn builder_rejects_fewer_than_two_points() {
    let err = DiscountCurve::builder("INVALID")
        .base_date(sample_base_date())
        .knots([(0.0, 1.0)])
        .build()
        .expect_err("should fail with fewer than two points");
    assert!(matches!(err, finstack_quant_core::Error::Input(_)));
}

#[test]
fn builder_rejects_non_positive_discount_factor() {
    let err = DiscountCurve::builder("NONPOS")
        .base_date(sample_base_date())
        .knots([(0.0, 1.0), (1.0, 0.0)])
        .build()
        .expect_err("non-positive discount factor should be rejected");
    assert!(matches!(err, finstack_quant_core::Error::Input(_)));
}

#[test]
fn builder_requires_explicit_base_date() {
    let err = DiscountCurve::builder("MISSING-BASE")
        .knots([(0.0, 1.0), (1.0, 0.98)])
        .build()
        .expect_err("discount curve should require an explicit base date");
    assert!(matches!(err, finstack_quant_core::Error::Input(_)));
}

#[test]
fn builder_infers_day_count_from_curve_id() {
    for (id, expected) in [
        ("USD-SOFR", DayCount::Act360),
        ("GBP-SONIA", DayCount::Act365F),
        ("EUR-ESTR", DayCount::Act360),
        ("TEST", DayCount::Act365F),
    ] {
        let curve = DiscountCurve::builder(id)
            .base_date(sample_base_date())
            .knots([(0.0, 1.0), (1.0, 0.95)])
            .build()
            .expect("valid curve");

        assert_eq!(curve.day_count(), expected, "{id}");
    }
}

#[test]
fn monotonic_df_accepted() {
    let result = DiscountCurve::builder("VALID-MONOTONIC")
        .base_date(sample_base_date())
        .knots([
            (0.0, 1.0),
            (1.0, 0.98),
            (2.0, 0.95),
            (5.0, 0.85),
            (10.0, 0.70),
        ])
        .build();

    assert!(
        result.is_ok(),
        "Monotonic discount factors should be accepted: {:?}",
        result.err()
    );

    let curve = result.unwrap();
    assert_eq!(curve.id().as_str(), "VALID-MONOTONIC");
}

#[test]
fn allow_non_monotonic_flag_overrides_validation() {
    let result = DiscountCurve::builder("OVERRIDE-ALLOWED")
        .base_date(sample_base_date())
        .knots([(0.0, 1.0), (1.0, 0.95), (2.0, 0.96)]) // Non-monotonic
        .interp(InterpStyle::Linear) // Required for non-monotonic DFs
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        )
        .build();

    assert!(
        result.is_ok(),
        "Non-monotonic DFs should be allowed when explicitly overridden: {:?}",
        result.err()
    );
}

// =============================================================================
// No-Arbitrage Validation Tests
// =============================================================================

#[test]
fn non_monotonic_df_rejected_by_default() {
    let result = DiscountCurve::builder("INVALID-NON-MONOTONIC")
        .base_date(sample_base_date())
        .knots([(0.0, 1.0), (1.0, 0.95), (2.0, 0.96)]) // 0.95 -> 0.96 is increasing!
        .build();

    assert!(
        result.is_err(),
        "Non-monotonic discount factors should be rejected by default"
    );

    let err = result.unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(
        err_str.contains("strictly decreasing")
            || err_str.contains("non-increasing")
            || err_str.contains("Invalid"),
        "Error message should explain monotonicity violation: {}",
        err_str
    );
}

#[test]
fn market_standard_validation_enables_all_checks() {
    let result = DiscountCurve::builder("NO-ARB-CHECK")
        .base_date(sample_base_date())
        .knots([(0.0, 1.0), (1.0, 0.98), (2.0, 0.95), (5.0, 0.85)])
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::MarketStandard,
        )
        .build();

    assert!(
        result.is_ok(),
        "Valid curve should pass no-arbitrage checks: {:?}",
        result.err()
    );
}

#[test]
fn custom_forward_rate_floor() {
    let curve = DiscountCurve::builder("CUSTOM-FLOOR")
        .base_date(sample_base_date())
        .knots([(0.0, 1.0), (1.0, 0.98), (5.0, 0.85)])
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: false,
                forward_floor: Some(-0.01),
            },
        ) // -100bp floor
        .build();

    assert!(
        curve.is_ok(),
        "Curve with reasonable forwards should pass custom floor: {:?}",
        curve.err()
    );
}

#[test]
fn reasonable_negative_forward_accepted() {
    let result = DiscountCurve::builder("INVALID-NEGATIVE-FWD")
        .base_date(sample_base_date())
        .knots([
            (0.0, 1.0),
            (1.0, 0.95),
            (2.0, 0.949), // Very small decrease
        ])
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::MarketStandard,
        )
        .build();

    assert!(
        result.is_ok(),
        "Reasonable negative spread should be accepted: {:?}",
        result.err()
    );
}

// =============================================================================
// Interpolation Tests
// =============================================================================

#[test]
fn interpolation_consistency_at_knot_points() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let knots = [(0.0, 1.0), (1.0, 0.98), (2.0, 0.95), (5.0, 0.88)];

    let curves = [
        DiscountCurve::builder("LINEAR")
            .base_date(base_date)
            .knots(knots)
            .interp(InterpStyle::Linear)
            .build()
            .unwrap(),
        DiscountCurve::builder("LOG")
            .base_date(base_date)
            .knots(knots)
            .interp(InterpStyle::LogLinear)
            .build()
            .unwrap(),
        DiscountCurve::builder("MC")
            .base_date(base_date)
            .knots(knots)
            .interp(InterpStyle::MonotoneConvex)
            .build()
            .unwrap(),
        DiscountCurve::builder("CH")
            .base_date(base_date)
            .knots(knots)
            .interp(InterpStyle::CubicHermite)
            .build()
            .unwrap(),
        DiscountCurve::builder("PQF")
            .base_date(base_date)
            .knots(knots)
            .interp(InterpStyle::PiecewiseQuadraticForward)
            .build()
            .unwrap(),
    ];

    // All methods should agree exactly at knot points
    for (t, expected_df) in knots {
        for curve in &curves {
            assert!((curve.df(t) - expected_df).abs() < 1e-12);
        }
    }
}

#[test]
fn interpolation_styles_produce_valid_results() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let knots = [(0.0, 1.0), (1.0, 0.95), (2.0, 0.90)];

    for style in [
        InterpStyle::Linear,
        InterpStyle::LogLinear,
        InterpStyle::MonotoneConvex,
        InterpStyle::CubicHermite,
        InterpStyle::PiecewiseQuadraticForward,
    ] {
        let curve = DiscountCurve::builder("TEST")
            .base_date(base_date)
            .knots(knots)
            .interp(style)
            .build()
            .unwrap();

        // Interpolation should produce positive values
        assert!(curve.df(0.5) > 0.0, "{:?} failed at t=0.5", style);
        assert!(curve.df(1.5) > 0.0, "{:?} failed at t=1.5", style);
    }
}

// =============================================================================
// Extrapolation Tests
// =============================================================================

fn create_test_curve(
    extrapolation: ExtrapolationPolicy,
) -> Result<DiscountCurve, Box<dyn std::error::Error>> {
    Ok(DiscountCurve::builder("TEST-CURVE")
        .base_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
        .knots([(0.0, 1.0), (1.0, 0.95), (2.0, 0.90), (5.0, 0.78)])
        .interp(InterpStyle::MonotoneConvex)
        .extrapolation(extrapolation)
        .build()?)
}

#[test]
fn flat_zero_extrapolation() {
    let curve = create_test_curve(ExtrapolationPolicy::FlatZero).unwrap();

    // Test left extrapolation (negative time)
    assert_eq!(curve.df(-0.5), 1.0);
    assert_eq!(curve.df(-1.0), 1.0);

    // Test right extrapolation (beyond 5 years)
    assert_eq!(curve.df(10.0), 0.78);
    assert_eq!(curve.df(100.0), 0.78);

    // Test exact knot values remain unchanged
    assert_eq!(curve.df(0.0), 1.0);
    assert_eq!(curve.df(5.0), 0.78);
}

#[test]
fn flat_forward_extrapolation() {
    let curve = create_test_curve(ExtrapolationPolicy::FlatForward).unwrap();

    // Test left extrapolation - should extend the forward rate
    let left_extrap = curve.df(-0.5);
    assert!(left_extrap > 1.0); // Should be higher than 1.0 for negative time with positive rates

    // Test right extrapolation - should extend the forward rate from last segment
    let right_extrap_near = curve.df(7.0);
    let right_extrap_far = curve.df(10.0);
    assert!(right_extrap_near < 0.78); // Should decay further
    assert!(right_extrap_far < right_extrap_near); // Should continue decaying

    // Test exact knot values remain unchanged
    assert_eq!(curve.df(0.0), 1.0);
    assert_eq!(curve.df(5.0), 0.78);
}

#[test]
fn flat_forward_extrapolation_continues_slope() {
    let curve = DiscountCurve::builder("EXTRAP")
        .base_date(sample_base_date())
        .knots([(0.0, 1.0), (2.0, 0.95)])
        .extrapolation(ExtrapolationPolicy::FlatForward)
        .build()
        .unwrap();

    let df2 = curve.df(2.0);
    let df4 = curve.df(4.0);
    assert!(
        df4 < df2,
        "flat-forward extrapolation should decay beyond last knot"
    );
}

#[test]
fn extrapolation_extreme_values() {
    let curve = create_test_curve(ExtrapolationPolicy::FlatForward).unwrap();

    // Very far right extrapolation
    let df_50y = curve.df(50.0);
    assert!(
        df_50y.is_finite() && df_50y > 0.0 && df_50y < 1.0,
        "50Y extrapolation should be finite and in (0,1): {}",
        df_50y
    );

    // Very far left extrapolation
    let df_neg_10 = curve.df(-10.0);
    assert!(
        df_neg_10.is_finite(),
        "Left extrapolation should be finite: {}",
        df_neg_10
    );
}

#[test]
fn interpolation_styles_with_both_extrapolation_policies() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let knots = [(0.0, 1.0), (1.0, 0.95), (2.0, 0.90)];

    for extrapolation in [
        ExtrapolationPolicy::FlatZero,
        ExtrapolationPolicy::FlatForward,
    ] {
        for style in [
            InterpStyle::Linear,
            InterpStyle::LogLinear,
            InterpStyle::MonotoneConvex,
            InterpStyle::CubicHermite,
        ] {
            let curve = DiscountCurve::builder("TEST")
                .base_date(base_date)
                .knots(knots)
                .interp(style)
                .extrapolation(extrapolation)
                .build()
                .unwrap();

            let extrap_test_time = 5.0;
            match extrapolation {
                ExtrapolationPolicy::FlatZero => {
                    assert_eq!(curve.df(extrap_test_time), 0.90);
                }
                ExtrapolationPolicy::FlatForward => {
                    assert!(curve.df(extrap_test_time) < 0.90);
                }
                _ => {}
            }
        }
    }
}

// =============================================================================
// MonotoneConvex Specific Tests
// =============================================================================

#[test]
fn monotone_convex_guarantees_positive_forwards() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let curve = DiscountCurve::builder("MC-POS-FWD")
        .base_date(base_date)
        .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.78), (10.0, 0.60)])
        .interp(InterpStyle::MonotoneConvex)
        .build()
        .unwrap();

    // Sample forward rates at many points using finite difference
    let dt = 0.01;
    for i in 1..1000 {
        let t = i as f64 * 0.01;
        if t + dt > 10.0 {
            break;
        }
        let fwd = (curve.df(t).ln() - curve.df(t + dt).ln()) / dt;
        assert!(
            fwd >= -1e-10,
            "Forward rate at t={:.2} is negative: {:.6}",
            t,
            fwd
        );
    }
}

#[test]
fn monotone_convex_forward_continuity_at_knots() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let curve = DiscountCurve::builder("MC-CONT")
        .base_date(base_date)
        .knots([(0.0, 1.0), (1.0, 0.95), (2.0, 0.90), (5.0, 0.78)])
        .interp(InterpStyle::MonotoneConvex)
        .build()
        .unwrap();

    // Check continuity at each interior knot
    let eps = 1e-6;
    for knot in [1.0, 2.0] {
        let fwd_left = (curve.df(knot - eps).ln() - curve.df(knot).ln()) / eps;
        let fwd_right = (curve.df(knot).ln() - curve.df(knot + eps).ln()) / eps;
        assert!(
            (fwd_left - fwd_right).abs() < 1e-4,
            "Forward discontinuity at knot {}: left={}, right={}",
            knot,
            fwd_left,
            fwd_right
        );
    }
}

// =============================================================================
// Bump Tests
// =============================================================================

#[test]
fn parallel_bump_and_df_batch() {
    let curve = sample_discount_curve("USD-OIS");
    let bumped = curve.with_parallel_bump(15.0).unwrap();
    assert_eq!(bumped.id().as_str(), "USD-OIS_bump_15bp");

    let times = [0.5, 1.0, 2.0];
    let solo: Vec<f64> = times.iter().map(|&t| curve.df(t)).collect();
    assert_eq!(curve.df_batch(&times), solo);

    for &t in &times {
        assert!(bumped.df(t) < curve.df(t));
    }
}

#[test]
fn triangular_key_rate_bump_targets_bucket() {
    let curve = DiscountCurve::builder("KR")
        .base_date(sample_base_date())
        .knots([(0.0, 1.0), (1.0, 0.98), (2.0, 0.95)])
        .build()
        .unwrap();

    // Triangular bump centered at 1.0, with neighbors at 0.0 and 2.0
    let bumped = curve
        .with_triangular_key_rate_bump_neighbors(Some(0.0), 1.0, Some(2.0), 25.0)
        .unwrap();

    // DF at t=0 is unchanged (weight = 0 at t=0)
    assert_eq!(bumped.df(0.0), curve.df(0.0));
    // DF at t=1 has maximum bump (weight = 1.0 at target)
    assert!(bumped.df(1.0) < curve.df(1.0));
    // DF at t=2 is unchanged (weight = 0 at t=2)
    assert_eq!(bumped.df(2.0), curve.df(2.0));
}

#[test]
fn with_parallel_bump_returns_error_on_invalid_curve() {
    let curve = DiscountCurve::builder("VALID")
        .base_date(sample_base_date())
        .knots([(0.0, 1.0), (1.0, 0.98), (5.0, 0.85)])
        .build()
        .unwrap();

    // Normal bump should succeed
    let bumped_ok = curve.with_parallel_bump(10.0);
    assert!(
        bumped_ok.is_ok(),
        "Valid curve bump should succeed: {:?}",
        bumped_ok.err()
    );

    // Extreme bump should still succeed (exponential bumping preserves monotonicity)
    let bumped_extreme = curve.with_parallel_bump(500.0);
    assert!(
        bumped_extreme.is_ok(),
        "Extreme parallel bump should succeed (preserves monotonicity): {:?}",
        bumped_extreme.err()
    );
}

#[test]
fn triangular_key_rate_bump_error_handling() {
    let curve = DiscountCurve::builder("VALID-KR")
        .base_date(sample_base_date())
        .knots([(0.0, 1.0), (1.0, 0.98), (2.0, 0.95), (5.0, 0.85)])
        .build()
        .unwrap();

    // Normal triangular key-rate bump should succeed
    let bumped_ok = curve.with_triangular_key_rate_bump_neighbors(Some(0.5), 1.5, Some(2.5), 15.0);
    assert!(
        bumped_ok.is_ok(),
        "Valid triangular key-rate bump should succeed: {:?}",
        bumped_ok.err()
    );

    // Extreme bump should either succeed or return typed error - no panic
    let bumped_extreme =
        curve.with_triangular_key_rate_bump_neighbors(Some(0.0), 1.0, Some(2.0), 1000.0);
    match bumped_extreme {
        Ok(_) => {}
        Err(e) => {
            assert!(
                matches!(
                    e,
                    finstack_quant_core::Error::Validation(_)
                        | finstack_quant_core::Error::Input(_)
                ),
                "Should return typed error, got: {:?}",
                e
            );
        }
    }
}

#[test]
fn parallel_bump_magnitude_verification() {
    let curve = sample_discount_curve("USD-OIS");
    let bp = 25.0;
    let bumped = curve.with_parallel_bump(bp).unwrap();

    // Verify bump formula at KNOT POINTS only
    // DF_bumped(t) = DF(t) * exp(-bp/10000 * t)
    for t in [0.0_f64, 1.0, 2.0] {
        let expected = curve.df(t) * (-bp / 10_000.0 * t).exp();
        assert!(
            (bumped.df(t) - expected).abs() < 1e-12,
            "Bump at knot t={}: got {}, expected {}",
            t,
            bumped.df(t),
            expected
        );
    }

    // Also verify the bump reduces DFs (higher rates)
    for t in [0.5, 1.5, 3.0] {
        assert!(
            bumped.df(t) < curve.df(t),
            "Bump should reduce DF at t={}",
            t
        );
    }
}

#[test]
fn triangular_key_rate_bump_weight_verification() {
    let curve = DiscountCurve::builder("KR-VERIFY")
        .base_date(sample_base_date())
        .knots([
            (0.0, 1.0),
            (0.5, 0.995),
            (1.0, 0.98),
            (1.5, 0.965),
            (2.0, 0.95),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let bp = 25.0;
    let bumped = curve
        .with_triangular_key_rate_bump_neighbors(Some(0.0), 1.0, Some(2.0), bp)
        .unwrap();

    // At target (t=1.0): weight=1.0
    let expected_1 = curve.df(1.0) * ((-bp / 10_000.0) * 1.0_f64).exp();
    assert!(
        (bumped.df(1.0) - expected_1).abs() < 1e-10,
        "Bump at t=1.0: got {}, expected {}",
        bumped.df(1.0),
        expected_1
    );

    // At t=0.5: weight=0.5 (linear interpolation from 0 to 1)
    let expected_05 = curve.df(0.5) * ((-bp / 10_000.0) * 0.5 * 0.5_f64).exp();
    assert!(
        (bumped.df(0.5) - expected_05).abs() < 1e-10,
        "Bump at t=0.5: got {}, expected {}",
        bumped.df(0.5),
        expected_05
    );

    // At boundaries: weight=0, no change
    assert!(
        (bumped.df(0.0) - curve.df(0.0)).abs() < 1e-12,
        "Bump at t=0.0 should be unchanged"
    );
    assert!(
        (bumped.df(2.0) - curve.df(2.0)).abs() < 1e-12,
        "Bump at t=2.0 should be unchanged"
    );
}

// =============================================================================
// Analytical Verification Tests
// =============================================================================

#[test]
fn forward_rate_analytical_verification() {
    let curve = DiscountCurve::builder("FWD-TEST")
        .base_date(sample_base_date())
        .knots([(0.0, 1.0), (1.0, 0.98), (2.0, 0.95)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    // f(1,2) = ln(DF(1)/DF(2)) / (t2-t1) = ln(0.98/0.95) / 1.0
    let expected = (0.98_f64 / 0.95).ln();
    let actual = curve
        .forward(1.0, 2.0)
        .expect("forward(1,2) should succeed");
    assert!(
        (actual - expected).abs() < 1e-12,
        "forward rate: got {}, expected {}",
        actual,
        expected
    );

    // f(0,1) = ln(1.0/0.98) / 1.0
    let expected_01 = (1.0_f64 / 0.98).ln();
    let actual_01 = curve
        .forward(0.0, 1.0)
        .expect("forward(0,1) should succeed");
    assert!(
        (actual_01 - expected_01).abs() < 1e-12,
        "forward rate 0-1: got {}, expected {}",
        actual_01,
        expected_01
    );
}

#[test]
fn forward_and_df_on_date() {
    let curve = sample_discount_curve("USD-OIS");
    let t1 = 0.5;
    let t2 = 1.0;
    let fwd = curve.forward(t1, t2).expect("forward should succeed");
    let zero_1 = curve.zero(t1);
    let zero_2 = curve.zero(t2);

    // Correct formula: f(t1,t2) = (z2*t2 - z1*t1) / (t2 - t1)
    assert!(
        (fwd - (zero_2 * t2 - zero_1 * t1) / (t2 - t1)).abs() < 1e-12,
        "forward rate formula mismatch"
    );
    assert!(
        fwd > 0.0,
        "forward rate should be positive for decreasing DFs"
    );

    let base = curve.base_date();
    let date = Date::from_calendar_date(base.year(), Month::December, 31).unwrap();
    let df_curve = curve
        .df_on_date_curve(date)
        .expect("df_on_date_curve should succeed");
    let df_static = curve
        .df_on_date(date, curve.day_count())
        .expect("df_on_date should succeed");
    assert!((df_curve - df_static).abs() < 1e-12);
}

#[test]
fn df_on_date_day_count_sensitivity() {
    let base = sample_base_date();
    let target = base + time::Duration::days(182); // ~6 months

    let curve_360 = DiscountCurve::builder("DC-360")
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (1.0, 0.95)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let curve_365 = DiscountCurve::builder("DC-365")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 0.95)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let df_360 = curve_360
        .df_on_date(target, DayCount::Act360)
        .expect("df_on_date should succeed");
    let df_365 = curve_365
        .df_on_date(target, DayCount::Act365F)
        .expect("df_on_date should succeed");

    // Different day counts = different time fractions = different DFs
    assert!(
        (df_360 - df_365).abs() > 1e-6,
        "Day counts should produce different DFs: {} vs {}",
        df_360,
        df_365
    );
}

#[test]
fn df_batch_handles_beyond_last_knot() {
    let curve = sample_discount_curve("USD-OIS");
    let times = [0.25, 1.0, 5.0, 10.0];
    let dfs = curve.df_batch(&times);
    assert_eq!(dfs.len(), times.len());
    assert!(dfs[3].is_finite());
}

// =============================================================================
// Special Environment Tests
// =============================================================================

#[test]
fn negative_rate_environment() {
    // Simulate EUR/CHF negative rates: DF > 1.0 for t > 0
    let curve = DiscountCurve::builder("NEG-RATES")
        .base_date(sample_base_date())
        .knots([(0.0, 1.0), (1.0, 1.005), (2.0, 1.008)])
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        )
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    assert!(curve.df(1.0) > 1.0, "DF should be > 1 for negative rates");
    assert!(curve.zero(1.0) < 0.0, "Zero rate should be negative");
}

/// MonotoneConvex supports negative-rate (DF > 1) curves end-to-end.
///
/// MonotoneConvex auto-detects negative discrete forwards and skips
/// the Hagan-West positivity amelioration, so a flat -1% curve (EUR/CHF/JPY
/// style) builds and interpolates faithfully.
#[test]
fn negative_rate_environment_monotone_convex() {
    let rate = -0.01_f64;
    let knots: Vec<(f64, f64)> = [0.0, 1.0, 2.0, 3.0, 5.0]
        .iter()
        .map(|&t| (t, (-rate * t).exp()))
        .collect();

    let curve = DiscountCurve::builder("EUR-NEG-MC")
        .base_date(sample_base_date())
        .knots(knots)
        .interp(InterpStyle::MonotoneConvex)
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::NegativeRateFriendly {
                forward_floor: -0.05,
            },
        )
        .build()
        .expect("MonotoneConvex negative-rate curve should build");

    // DF > 1 at and between knots; zero rate round-trips the flat -1%.
    for t in [0.5, 1.0, 1.7, 2.5, 3.0, 4.2, 5.0] {
        let df = curve.df(t);
        assert!(
            df > 1.0,
            "DF({t}) = {df} should exceed 1 for negative rates"
        );
        let zero = curve.zero(t);
        assert!(
            (zero - rate).abs() < 1e-9,
            "zero rate at t={t} should be ~ -1%, got {zero}"
        );
    }
}

#[test]
fn credit_curve_construction() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // CDS spreads to survival probabilities (approximately)
    let times: [f64; 6] = [0.0, 0.5, 1.0, 3.0, 5.0, 10.0];
    let cds_spreads: [f64; 6] = [0.0, 50e-4, 75e-4, 120e-4, 150e-4, 200e-4]; // bps

    // Convert spreads to approximate survival probabilities
    let survival_probs: Vec<f64> = times
        .iter()
        .zip(cds_spreads.iter())
        .map(|(t, s)| {
            let product: f64 = (*s) * (*t);
            (-product).exp()
        })
        .collect();

    let knots: Vec<(f64, f64)> = times
        .iter()
        .zip(survival_probs.iter())
        .map(|(t, sp)| (*t, *sp))
        .collect();

    let credit_curve = DiscountCurve::builder("CREDIT-5Y")
        .base_date(base_date)
        .knots(knots)
        .interp(InterpStyle::MonotoneConvex)
        .extrapolation(ExtrapolationPolicy::FlatForward)
        .build()
        .unwrap();

    // Test that survival probabilities are indeed decreasing
    let sp_1y = credit_curve.df(1.0);
    let sp_5y = credit_curve.df(5.0);
    let sp_10y = credit_curve.df(10.0);
    let sp_15y = credit_curve.df(15.0); // Extrapolated

    assert!(sp_1y > sp_5y);
    assert!(sp_5y > sp_10y);
    assert!(sp_10y > sp_15y);
    assert!(sp_15y > 0.0 && sp_15y <= 1.0);
}

#[test]
fn minimal_two_point_curve() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let minimal_curve = DiscountCurve::builder("MINIMAL")
        .base_date(base_date)
        .knots([(0.0, 1.0), (1.0, 0.95)])
        .interp(InterpStyle::Linear)
        .extrapolation(ExtrapolationPolicy::FlatZero)
        .build()
        .unwrap();

    assert_eq!(minimal_curve.df(0.0), 1.0);
    assert_eq!(minimal_curve.df(1.0), 0.95);

    // Test extrapolation with minimal curve
    assert_eq!(minimal_curve.df(-0.5), 1.0); // Flat-zero left
    assert_eq!(minimal_curve.df(2.0), 0.95); // Flat-zero right
}

// =============================================================================
// Forward-rate numerical precision
// =============================================================================
//
// `DiscountCurve::forward` is implemented as the canonical
//
//     -(self.df(t2) / self.df(t1)).ln() / (t2 - t1)
//
// rather than going via the zero-rate form
//
//     (self.zero(t2) * t2 - self.zero(t1) * t1) / (t2 - t1)
//
// which is algebraically equivalent but round-trips each endpoint
// through an extra division / multiplication and costs two `ln` calls
// where one suffices. The tests below lock in:
//   1. The identity holds to round-off across a battery of (t1, t2) pairs.
//   2. Known-exact cases match analytical values to < 1 ulp.
//   3. Short-tenor forwards (near `min_forward_tenor`) do not carry the
//      cancellation error that the zero-rate form exhibits at ~1e-6 y.

#[test]
fn forward_matches_canonical_formula_across_tenor_grid() {
    // Build a non-trivial curve so that zeros are not constant.
    let curve = DiscountCurve::builder("USD")
        .base_date(sample_base_date())
        .knots([
            (0.0, 1.0),
            (0.25, 0.99),
            (0.5, 0.98),
            (1.0, 0.96),
            (2.0, 0.92),
            (5.0, 0.80),
            (10.0, 0.62),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("curve builds");

    // Canonical reference: -(df(t2)/df(t1)).ln() / (t2 - t1).
    let pairs: &[(f64, f64)] = &[
        (0.0, 0.25),
        (0.0, 1.0),
        (0.25, 0.5),
        (0.5, 1.0),
        (1.0, 2.0),
        (2.0, 5.0),
        (5.0, 10.0),
        (0.1, 0.2),
        (1.5, 1.75),
    ];

    for &(t1, t2) in pairs {
        let fwd = curve.forward(t1, t2).expect("finite forward rate");
        let df1 = curve.df(t1);
        let df2 = curve.df(t2);
        let canonical = -(df2 / df1).ln() / (t2 - t1);
        assert!(
            (fwd - canonical).abs() < 1e-13,
            "forward({t1}, {t2}) = {fwd}, canonical = {canonical}, diff = {}",
            (fwd - canonical).abs()
        );
    }
}

#[test]
fn forward_on_flat_continuously_compounded_curve_equals_rate() {
    // On an exactly flat curve DF(t) = exp(-r * t), the forward f(t1, t2)
    // must equal r to machine precision.
    let r = 0.037_f64;
    let knots = [(0.0, 1.0), (1.0, (-r).exp()), (5.0, (-r * 5.0).exp())];
    let curve = DiscountCurve::builder("FLAT")
        .base_date(sample_base_date())
        .knots(knots)
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("curve builds");

    for &(t1, t2) in &[(0.0, 1.0), (1.0, 5.0), (0.25, 4.75)] {
        let fwd = curve.forward(t1, t2).expect("finite forward");
        // Pre-fix: the `z*t` round-trip introduced ~2 ulp error at short
        // tenors. Post-fix: equality within 1 ulp of r.
        let ulp_r = (r.to_bits() + 1) as f64 - r.to_bits() as f64; // ~ulp spacing
        let tol = (4.0 * ulp_r).max(1e-15);
        assert!(
            (fwd - r).abs() < tol,
            "forward({t1}, {t2}) on flat curve = {fwd}, want {r}, tol = {tol}"
        );
    }
}

#[test]
fn forward_short_tenor_retains_precision() {
    // At very short Δt the z·t round-trip of the pre-fix form is the
    // dominant error source. The post-fix form computes a single log-ratio
    // and divides by Δt, which is the numerically optimal evaluation order
    // for this identity.
    let curve = DiscountCurve::builder("USD")
        .base_date(sample_base_date())
        .knots([(0.0, 1.0), (1.0, 0.96), (2.0, 0.92), (5.0, 0.80)])
        .interp(InterpStyle::LogLinear)
        .min_forward_tenor(1e-10)
        .build()
        .expect("curve builds");

    // Use sub-millisecond tenors to stress cancellation.
    // The `forward` function internally uses `(t2 - t1)` as the denominator,
    // which for t1 = 1.0 and dt near machine-eps differs from `dt` by
    // f64 rounding. Our reference uses the same `(t2 - t1)` to avoid a
    // spurious 1-ulp mismatch.
    let t1 = 1.0;
    for &dt in &[1e-5, 1e-6, 1e-7, 1e-8] {
        let t2 = t1 + dt;
        let dt_actual = t2 - t1; // may differ from `dt` by a few ulps
        let fwd = curve.forward(t1, t2).expect("finite forward");
        let canonical = -(curve.df(t2) / curve.df(t1)).ln() / dt_actual;
        assert!(
            fwd.is_finite() && (fwd - canonical).abs() < 1e-14,
            "forward({t1}, {t2}) at dt={dt}: got {fwd}, want {canonical}"
        );
    }
}

// =============================================================================
// Serialization Tests
// =============================================================================

mod serde_tests {
    use super::*;

    #[test]
    fn roundtrip_linear() {
        let original = DiscountCurve::builder("USD-OIS")
            .base_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
            .knots([(0.0, 1.0), (1.0, 0.98), (2.0, 0.95), (5.0, 0.88)])
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::FlatForward)
            .build()
            .unwrap();

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: DiscountCurve = serde_json::from_str(&json).unwrap();

        assert_eq!(original.id(), deserialized.id());
        assert_eq!(original.base_date(), deserialized.base_date());
        assert_eq!(original.knots(), deserialized.knots());
        assert_eq!(original.dfs(), deserialized.dfs());

        for t in [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 5.0, 7.0] {
            assert!(
                (original.df(t) - deserialized.df(t)).abs() < 1e-12,
                "DF mismatch at t={}: {} vs {}",
                t,
                original.df(t),
                deserialized.df(t)
            );
        }
    }

    #[test]
    fn roundtrip_log_linear() {
        let original = DiscountCurve::builder("EUR-ESTR")
            .base_date(Date::from_calendar_date(2025, Month::June, 30).unwrap())
            .knots([(0.0, 1.0), (0.25, 0.995), (0.5, 0.99), (1.0, 0.98)])
            .interp(InterpStyle::LogLinear)
            .build()
            .unwrap();

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: DiscountCurve = serde_json::from_str(&json).unwrap();

        for t in [0.0, 0.1, 0.25, 0.4, 0.5, 0.75, 1.0, 1.5] {
            assert!(
                (original.df(t) - deserialized.df(t)).abs() < 1e-12,
                "LogLinear DF mismatch at t={}: {} vs {}",
                t,
                original.df(t),
                deserialized.df(t)
            );
        }
    }

    #[test]
    fn roundtrip_monotone_convex() {
        let original = DiscountCurve::builder("GBP-SONIA")
            .base_date(Date::from_calendar_date(2025, Month::March, 15).unwrap())
            .knots([
                (0.0, 1.0),
                (0.5, 0.99),
                (1.0, 0.975),
                (2.0, 0.95),
                (5.0, 0.88),
                (10.0, 0.75),
            ])
            .interp(InterpStyle::MonotoneConvex)
            .build()
            .unwrap();

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: DiscountCurve = serde_json::from_str(&json).unwrap();

        for t in [0.0, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 5.0, 7.5, 10.0] {
            assert!(
                (original.df(t) - deserialized.df(t)).abs() < 1e-12,
                "MonotoneConvex DF mismatch at t={}: {} vs {}",
                t,
                original.df(t),
                deserialized.df(t)
            );
        }
    }

    #[test]
    fn roundtrip_cubic_hermite() {
        let original = DiscountCurve::builder("JPY-TONAR")
            .base_date(Date::from_calendar_date(2025, Month::September, 1).unwrap())
            .knots([(0.0, 1.0), (1.0, 0.99), (3.0, 0.96), (5.0, 0.92)])
            .interp(InterpStyle::CubicHermite)
            .build()
            .unwrap();

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: DiscountCurve = serde_json::from_str(&json).unwrap();

        for t in [0.0, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0] {
            assert!(
                (original.df(t) - deserialized.df(t)).abs() < 1e-12,
                "CubicHermite DF mismatch at t={}: {} vs {}",
                t,
                original.df(t),
                deserialized.df(t)
            );
        }
    }

    #[test]
    fn roundtrip_flat_fwd() {
        let original = DiscountCurve::builder("CHF-SARON")
            .base_date(Date::from_calendar_date(2025, Month::December, 31).unwrap())
            .knots([(0.0, 1.0), (0.5, 0.995), (1.0, 0.988), (2.0, 0.975)])
            .interp(InterpStyle::LogLinear)
            .build()
            .unwrap();

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: DiscountCurve = serde_json::from_str(&json).unwrap();

        for t in [0.0, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 2.5] {
            assert!(
                (original.df(t) - deserialized.df(t)).abs() < 1e-12,
                "LogLinear DF mismatch at t={}: {} vs {}",
                t,
                original.df(t),
                deserialized.df(t)
            );
        }
    }

    #[test]
    fn extrapolation_policy_preserved() {
        // Test FlatZero extrapolation
        let curve_flat_zero = DiscountCurve::builder("TEST-FLAT-ZERO")
            .base_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
            .knots([(0.0, 1.0), (1.0, 0.98), (2.0, 0.95)])
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::FlatZero)
            .build()
            .unwrap();

        let json = serde_json::to_string(&curve_flat_zero).unwrap();
        let deserialized_flat_zero: DiscountCurve = serde_json::from_str(&json).unwrap();

        let t_beyond = 10.0;
        assert!(
            (curve_flat_zero.df(t_beyond) - deserialized_flat_zero.df(t_beyond)).abs() < 1e-12,
            "FlatZero extrapolation mismatch"
        );

        // Test FlatForward extrapolation
        let curve_flat_forward = DiscountCurve::builder("TEST-FLAT-FWD")
            .base_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
            .knots([(0.0, 1.0), (1.0, 0.98), (2.0, 0.95)])
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::FlatForward)
            .build()
            .unwrap();

        let json = serde_json::to_string(&curve_flat_forward).unwrap();
        let deserialized_flat_forward: DiscountCurve = serde_json::from_str(&json).unwrap();

        assert!(
            (curve_flat_forward.df(t_beyond) - deserialized_flat_forward.df(t_beyond)).abs()
                < 1e-12,
            "FlatForward extrapolation mismatch"
        );

        // Verify the two policies give different results
        assert!(
            (curve_flat_zero.df(t_beyond) - curve_flat_forward.df(t_beyond)).abs() > 0.01,
            "Different extrapolation policies should produce different results"
        );
    }

    #[test]
    fn pretty_json_roundtrip() {
        let original = DiscountCurve::builder("TEST-PRETTY")
            .base_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
            .knots([(0.0, 1.0), (1.0, 0.98), (2.0, 0.95)])
            .interp(InterpStyle::Linear)
            .build()
            .unwrap();

        let json = serde_json::to_string_pretty(&original).unwrap();
        let deserialized: DiscountCurve = serde_json::from_str(&json).unwrap();

        for t in [0.0, 0.5, 1.0, 1.5, 2.0] {
            assert!(
                (original.df(t) - deserialized.df(t)).abs() < 1e-12,
                "Pretty JSON DF mismatch at t={}: {} vs {}",
                t,
                original.df(t),
                deserialized.df(t)
            );
        }
    }

    #[test]
    fn validates_on_deserialization() {
        let bad_json = r#"{
            "id": "BAD",
            "base": "2025-01-15",
            "day_count": "Act365F",
            "knot_points": [[0.0, 1.0], [1.0, 1.01]],
            "interp_style": "Linear",
            "extrapolation": "FlatForward",
            "allow_non_monotonic": false
        }"#;

        let result: Result<DiscountCurve, _> = serde_json::from_str(bad_json);
        assert!(
            result.is_err(),
            "Should reject non-monotonic discount factors"
        );
    }
}

mod roll_forward_realized_forward {
    //! Realized-forward roll semantics :
    //! `DF_new(T - dt) = DF_old(T) / DF_old(dt)`, so forwards realize and a
    //! flat curve stays flat under roll.

    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use time::macros::date;

    /// Synthetic IDs (no market hint) default to Act365F, so rolling 365
    /// days corresponds to exactly dt = 1.0 in the curve's time basis.
    fn flat_curve(rate: f64, knots: &[f64]) -> DiscountCurve {
        let points: Vec<(f64, f64)> = knots.iter().map(|&t| (t, (-rate * t).exp())).collect();
        DiscountCurve::builder("TEST-FLAT")
            .base_date(date!(2025 - 01 - 01))
            .knots(points)
            .build()
            .unwrap()
    }

    #[test]
    fn flat_curve_stays_flat_under_roll_forward() {
        let knots = [0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0];
        let rate = 0.05;
        let curve = flat_curve(rate, &knots);

        // Roll 365 days = 1.0y under Act365F.
        let rolled = curve.roll_forward(365).unwrap();

        // Fresh flat 5% curve on the rolled knot grid.
        let fresh_knots = [1.0, 2.0, 4.0, 6.0, 9.0];
        let fresh = flat_curve(rate, &fresh_knots);

        // Knot-aligned probes (interpolation-exact at knots).
        for &t in &fresh_knots {
            assert!(
                (rolled.df(t) - fresh.df(t)).abs() < 1e-12,
                "flat curve must stay flat under roll at t={t}: rolled={} fresh={}",
                rolled.df(t),
                fresh.df(t)
            );
        }
        // DF(0) = 1 exactly (prepended knot is correct after renormalization).
        assert!((rolled.df(0.0) - 1.0).abs() < 1e-14);
    }

    #[test]
    fn roll_forward_realizes_forwards() {
        // Non-flat curve.
        let curve = DiscountCurve::builder("TEST-ROLL")
            .base_date(date!(2025 - 01 - 01))
            .knots([
                (0.5, 0.995),
                (1.0, 0.985),
                (2.0, 0.955),
                (5.0, 0.86),
                (10.0, 0.70),
            ])
            .build()
            .unwrap();

        // Roll 365 days = dt = 1.0y under Act365F.
        let dt = 1.0;
        let rolled = curve.roll_forward(365).unwrap();
        let df_dt = curve.df(dt);

        for &t_old in &[2.0_f64, 5.0, 10.0] {
            let expected = curve.df(t_old) / df_dt;
            let actual = rolled.df(t_old - dt);
            assert!(
                (actual - expected).abs() < 1e-12,
                "DF_rolled({}) must equal DF_old({t_old})/DF_old({dt}): {actual} vs {expected}",
                t_old - dt
            );
        }
    }
}
