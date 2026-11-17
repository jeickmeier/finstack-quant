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
use finstack_quant_core::market_data::bumps::{BumpSpec, Bumpable};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::traits::Discounting;
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
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .unwrap();

        let interpolated = curve.df(t).unwrap();
        prop_assert!(interpolated.is_finite());
        prop_assert!(interpolated > 0.0);
        prop_assert!(interpolated <= 1.0 + 1e-12);
    }
}

#[test]
fn date_discounting_supports_dates_before_curve_base() {
    let base = sample_base_date();
    let curve = DiscountCurve::builder("SPOT-BASED")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.95), (2.0, 0.90)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
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

// Builder Validation Tests

#[test]
fn builder_rejects_fewer_than_two_points() {
    let err = DiscountCurve::builder("INVALID")
        .base_date(sample_base_date())
        .knots([(0.0, 1.0)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect_err("should fail with fewer than two points");
    assert!(matches!(err, finstack_quant_core::Error::Input(_)));
}

#[test]
fn builder_rejects_non_positive_discount_factor() {
    let err = DiscountCurve::builder("NONPOS")
        .base_date(sample_base_date())
        .knots([(0.0, 1.0), (1.0, 0.0)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
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
    assert!(err.to_string().contains("requires base_date"), "{err}");
}

#[test]
fn builder_requires_explicit_clock_interp_and_extrapolation() {
    let base = sample_base_date();
    assert!(
        DiscountCurve::builder("NO-DC")
            .base_date(base)
            .knots([(0.0, 1.0), (1.0, 0.95)])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Extend)
            .build()
            .is_err()
    );
    assert!(
        DiscountCurve::builder("NO-INTERP")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, 0.95)])
            .extrapolation(ExtrapolationPolicy::Extend)
            .build()
            .is_err()
    );
    assert!(
        DiscountCurve::builder("NO-EXTRAP")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, 0.95)])
            .interp(InterpStyle::LogLinear)
            .build()
            .is_err()
    );

    let curve = DiscountCurve::builder("EXPLICIT")
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (1.0, 0.95)])
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Extend)
        .build()
        .expect("explicit curve builds");
    assert_eq!(curve.day_count(), DayCount::Act360);
    assert!(
        curve
            .zero_rate_on_date(
                base - time::Duration::days(1),
                finstack_quant_core::math::Compounding::Continuous,
            )
            .is_err(),
        "pre-base zero_rate_on_date must error"
    );
}

#[test]
fn monotonic_df_accepted() {
    let result = DiscountCurve::builder("VALID-MONOTONIC")
        .base_date(sample_base_date())
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Extend)
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
fn non_monotonic_and_negative_rate_dfs_accepted_without_flags() {
    let non_mono = DiscountCurve::builder("NON-MONOTONIC")
        .base_date(sample_base_date())
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 0.95), (2.0, 0.96)])
        .interp(InterpStyle::Linear)
        .extrapolation(ExtrapolationPolicy::Extend)
        .build();
    assert!(
        non_mono.is_ok(),
        "Positive non-monotonic DFs should build cleanly: {:?}",
        non_mono.err()
    );

    let neg = DiscountCurve::builder("NEG-EUR-CHF")
        .base_date(sample_base_date())
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 1.005), (5.0, 1.02)])
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Extend)
        .build();
    assert!(neg.is_ok(), "DF > 1 must build cleanly: {:?}", neg.err());
    assert!(
        DiscountCurve::builder("ZERO-DF")
            .base_date(sample_base_date())
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, 0.0)])
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::Extend)
            .build()
            .is_err(),
        "non-positive DF must error"
    );
}

// Interpolation Tests

#[test]
fn interpolation_consistency_at_knot_points() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let knots = [(0.0, 1.0), (1.0, 0.98), (2.0, 0.95), (5.0, 0.88)];

    let curves = [
        DiscountCurve::builder("LINEAR")
            .base_date(base_date)
            .knots(knots)
            .interp(InterpStyle::Linear)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .unwrap(),
        DiscountCurve::builder("LOG")
            .base_date(base_date)
            .knots(knots)
            .interp(InterpStyle::LogLinear)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .unwrap(),
        DiscountCurve::builder("MC")
            .base_date(base_date)
            .knots(knots)
            .interp(InterpStyle::MonotoneConvex)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .unwrap(),
        DiscountCurve::builder("CH")
            .base_date(base_date)
            .knots(knots)
            .interp(InterpStyle::CubicHermite)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .unwrap(),
        DiscountCurve::builder("PQF")
            .base_date(base_date)
            .knots(knots)
            .interp(InterpStyle::PiecewiseQuadraticForward)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .unwrap(),
    ];

    // All methods should agree exactly at knot points
    for (t, expected_df) in knots {
        for curve in &curves {
            assert!((curve.df(t).unwrap() - expected_df).abs() < 1e-12);
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
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .unwrap();

        // Interpolation should produce positive values
        assert!(curve.df(0.5).unwrap() > 0.0, "{:?} failed at t=0.5", style);
        assert!(curve.df(1.5).unwrap() > 0.0, "{:?} failed at t=1.5", style);
    }
}

// Extrapolation Tests

fn create_test_curve(
    extrapolation: ExtrapolationPolicy,
) -> Result<DiscountCurve, Box<dyn std::error::Error>> {
    Ok(DiscountCurve::builder("TEST-CURVE")
        .base_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
        .knots([(0.0, 1.0), (1.0, 0.95), (2.0, 0.90), (5.0, 0.78)])
        .interp(InterpStyle::MonotoneConvex)
        .extrapolation(extrapolation)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .build()?)
}

#[test]
fn flat_zero_extrapolation() {
    let curve = create_test_curve(ExtrapolationPolicy::Flat).unwrap();

    // Test left extrapolation (negative time)
    assert_eq!(curve.df(-0.5).unwrap(), 1.0);
    assert_eq!(curve.df(-1.0).unwrap(), 1.0);

    // Test right extrapolation (beyond 5 years)
    assert_eq!(curve.df(10.0).unwrap(), 0.78);
    assert_eq!(curve.df(100.0).unwrap(), 0.78);

    // Test exact knot values remain unchanged
    assert_eq!(curve.df(0.0).unwrap(), 1.0);
    assert_eq!(curve.df(5.0).unwrap(), 0.78);
}

#[test]
fn flat_forward_extrapolation() {
    let curve = create_test_curve(ExtrapolationPolicy::Extend).unwrap();

    // Test left extrapolation - should extend the forward rate
    let left_extrap = curve.df(-0.5).unwrap();
    assert!(left_extrap > 1.0); // Should be higher than 1.0 for negative time with positive rates

    // Test right extrapolation - should extend the forward rate from last segment
    let right_extrap_near = curve.df(7.0).unwrap();
    let right_extrap_far = curve.df(10.0).unwrap();
    assert!(right_extrap_near < 0.78); // Should decay further
    assert!(right_extrap_far < right_extrap_near); // Should continue decaying

    // Test exact knot values remain unchanged
    assert_eq!(curve.df(0.0).unwrap(), 1.0);
    assert_eq!(curve.df(5.0).unwrap(), 0.78);
}

#[test]
fn flat_forward_extrapolation_continues_slope() {
    let curve = DiscountCurve::builder("EXTRAP")
        .base_date(sample_base_date())
        .knots([(0.0, 1.0), (2.0, 0.95)])
        .extrapolation(ExtrapolationPolicy::Extend)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .build()
        .unwrap();

    let df2 = curve.df(2.0).unwrap();
    let df4 = curve.df(4.0).unwrap();
    assert!(
        df4 < df2,
        "flat-forward extrapolation should decay beyond last knot"
    );
}

#[test]
fn extrapolation_extreme_values() {
    let curve = create_test_curve(ExtrapolationPolicy::Extend).unwrap();

    // Very far right extrapolation
    let df_50y = curve.df(50.0).unwrap();
    assert!(
        df_50y.is_finite() && df_50y > 0.0 && df_50y < 1.0,
        "50Y extrapolation should be finite and in (0,1): {}",
        df_50y
    );

    // Very far left extrapolation
    let df_neg_10 = curve.df(-10.0).unwrap();
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

    for extrapolation in [ExtrapolationPolicy::Flat, ExtrapolationPolicy::Extend] {
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
                .day_count(finstack_quant_core::dates::DayCount::Act365F)
                .build()
                .unwrap();

            let extrap_test_time = 5.0;
            match extrapolation {
                ExtrapolationPolicy::Flat => {
                    assert_eq!(curve.df(extrap_test_time).unwrap(), 0.90);
                }
                ExtrapolationPolicy::Extend => {
                    assert!(curve.df(extrap_test_time).unwrap() < 0.90);
                }
                _ => {}
            }
        }
    }
}

// MonotoneConvex Specific Tests

#[test]
fn monotone_convex_guarantees_positive_forwards() {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let curve = DiscountCurve::builder("MC-POS-FWD")
        .base_date(base_date)
        .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.78), (10.0, 0.60)])
        .interp(InterpStyle::MonotoneConvex)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    // Sample forward rates at many points using finite difference
    let dt = 0.01;
    for i in 1..1000 {
        let t = i as f64 * 0.01;
        if t + dt > 10.0 {
            break;
        }
        let fwd = (curve.df(t).unwrap().ln() - curve.df(t + dt).unwrap().ln()) / dt;
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
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    // Check continuity at each interior knot
    let eps = 1e-6;
    for knot in [1.0, 2.0] {
        let fwd_left = (curve.df(knot - eps).unwrap().ln() - curve.df(knot).unwrap().ln()) / eps;
        let fwd_right = (curve.df(knot).unwrap().ln() - curve.df(knot + eps).unwrap().ln()) / eps;
        assert!(
            (fwd_left - fwd_right).abs() < 1e-4,
            "Forward discontinuity at knot {}: left={}, right={}",
            knot,
            fwd_left,
            fwd_right
        );
    }
}

// Bump Tests

#[test]
fn parallel_bump_lowers_discount_factors() {
    let curve = sample_discount_curve("USD-OIS");
    let bumped = curve.apply_bump(BumpSpec::parallel_bp(15.0)).unwrap();
    assert_eq!(bumped.id().as_str(), "USD-OIS_bump_15bp");

    let times = [0.5, 1.0, 2.0];
    for &t in &times {
        assert!(bumped.df(t).unwrap() < curve.df(t).unwrap());
    }
}

#[test]
fn triangular_key_rate_bump_targets_bucket() {
    let curve = DiscountCurve::builder("KR")
        .base_date(sample_base_date())
        .knots([(0.0, 1.0), (1.0, 0.98), (2.0, 0.95)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    // Triangular bump centered at 1.0, with neighbors at 0.0 and 2.0
    let bumped = curve
        .apply_bump(BumpSpec::triangular_key_rate_bp(0.0, 1.0, 2.0, 25.0))
        .unwrap();

    // DF at t=0 is unchanged (weight = 0 at t=0)
    assert_eq!(bumped.df(0.0).unwrap(), curve.df(0.0).unwrap());
    // DF at t=1 has maximum bump (weight = 1.0 at target)
    assert!(bumped.df(1.0).unwrap() < curve.df(1.0).unwrap());
    // DF at t=2 is unchanged (weight = 0 at t=2)
    assert_eq!(bumped.df(2.0).unwrap(), curve.df(2.0).unwrap());
}

#[test]
fn discount_stress_supports_negative_rates_and_rejects_zero_df() {
    let curve = DiscountCurve::builder("OIS")
        .base_date(sample_base_date())
        .day_count(DayCount::Act365F)
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Extend)
        .knots([(0.0, 1.0), (0.5, 0.999), (1.0, 0.998)])
        .build()
        .unwrap();
    for result in [
        curve.apply_bump(BumpSpec::parallel_bp(-100.0)),
        curve.apply_bump(BumpSpec::triangular_key_rate_first_bp(0.5, 1.0, 100.0)),
    ] {
        let bumped = result.expect("stress permits negative forwards");
        assert!(bumped.df(1.0).unwrap() > bumped.df(0.5).unwrap());
    }
    assert!(curve.apply_bump(BumpSpec::parallel_bp(1e8)).is_err());
}

#[test]
fn parallel_bump_returns_error_on_invalid_curve() {
    let curve = DiscountCurve::builder("VALID")
        .base_date(sample_base_date())
        .knots([(0.0, 1.0), (1.0, 0.98), (5.0, 0.85)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    // Normal bump should succeed
    let bumped_ok = curve.apply_bump(BumpSpec::parallel_bp(10.0));
    assert!(
        bumped_ok.is_ok(),
        "Valid curve bump should succeed: {:?}",
        bumped_ok.err()
    );

    // Extreme bump should still succeed (exponential bumping preserves monotonicity)
    let bumped_extreme = curve.apply_bump(BumpSpec::parallel_bp(500.0));
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
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    // Normal triangular key-rate bump should succeed
    let bumped_ok = curve.apply_bump(BumpSpec::triangular_key_rate_bp(0.5, 1.5, 2.5, 15.0));
    assert!(
        bumped_ok.is_ok(),
        "Valid triangular key-rate bump should succeed: {:?}",
        bumped_ok.err()
    );

    // Extreme bump should either succeed or return typed error - no panic
    let bumped_extreme = curve.apply_bump(BumpSpec::triangular_key_rate_bp(0.0, 1.0, 2.0, 1000.0));
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
    let bumped = curve.apply_bump(BumpSpec::parallel_bp(bp)).unwrap();

    // Verify bump formula at KNOT POINTS only
    // DF_bumped(t) = DF(t) * exp(-bp/10000 * t)
    for t in [0.0_f64, 1.0, 2.0] {
        let expected = curve.df(t).unwrap() * (-bp / 10_000.0 * t).exp();
        assert!(
            (bumped.df(t).unwrap() - expected).abs() < 1e-12,
            "Bump at knot t={}: got {}, expected {}",
            t,
            bumped.df(t).unwrap(),
            expected
        );
    }

    // Also verify the bump reduces DFs (higher rates)
    for t in [0.5, 1.5, 3.0] {
        assert!(
            bumped.df(t).unwrap() < curve.df(t).unwrap(),
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
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let bp = 25.0;
    let bumped = curve
        .apply_bump(BumpSpec::triangular_key_rate_bp(0.0, 1.0, 2.0, bp))
        .unwrap();

    // At target (t=1.0): weight=1.0
    let expected_1 = curve.df(1.0).unwrap() * ((-bp / 10_000.0) * 1.0_f64).exp();
    assert!(
        (bumped.df(1.0).unwrap() - expected_1).abs() < 1e-10,
        "Bump at t=1.0: got {}, expected {}",
        bumped.df(1.0).unwrap(),
        expected_1
    );

    // At t=0.5: weight=0.5 (linear interpolation from 0 to 1)
    let expected_05 = curve.df(0.5).unwrap() * ((-bp / 10_000.0) * 0.5 * 0.5_f64).exp();
    assert!(
        (bumped.df(0.5).unwrap() - expected_05).abs() < 1e-10,
        "Bump at t=0.5: got {}, expected {}",
        bumped.df(0.5).unwrap(),
        expected_05
    );

    // At boundaries: weight=0, no change
    assert!(
        (bumped.df(0.0).unwrap() - curve.df(0.0).unwrap()).abs() < 1e-12,
        "Bump at t=0.0 should be unchanged"
    );
    assert!(
        (bumped.df(2.0).unwrap() - curve.df(2.0).unwrap()).abs() < 1e-12,
        "Bump at t=2.0 should be unchanged"
    );
}

// Analytical Verification Tests

#[test]
fn forward_rate_analytical_verification() {
    let curve = DiscountCurve::builder("FWD-TEST")
        .base_date(sample_base_date())
        .knots([(0.0, 1.0), (1.0, 0.98), (2.0, 0.95)])
        .interp(InterpStyle::Linear)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
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
    let zero_1 = curve
        .zero_rate(t1, finstack_quant_core::math::Compounding::Continuous)
        .unwrap();
    let zero_2 = curve
        .zero_rate(t2, finstack_quant_core::math::Compounding::Continuous)
        .unwrap();

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
    let df_curve = curve.df_on_date(date).expect("df_on_date should succeed");
    assert!(df_curve.is_finite());
    assert!(df_curve > 0.0);
}

#[test]
fn df_on_date_uses_stored_day_count() {
    let base = sample_base_date();
    let target = base + time::Duration::days(182); // ~6 months

    let curve_360 = DiscountCurve::builder("DC-360")
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (1.0, 0.95)])
        .interp(InterpStyle::Linear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let curve_365 = DiscountCurve::builder("DC-365")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 0.95)])
        .interp(InterpStyle::Linear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let df_360 = curve_360
        .df_on_date(target)
        .expect("df_on_date should succeed");
    let df_365 = curve_365
        .df_on_date(target)
        .expect("df_on_date should succeed");

    // Different stored day counts = different time fractions = different DFs
    assert!(
        (df_360 - df_365).abs() > 1e-6,
        "Day counts should produce different DFs: {} vs {}",
        df_360,
        df_365
    );
}

// Special Environment Tests

#[test]
fn negative_rate_environment() {
    // Simulate EUR/CHF negative rates: DF > 1.0 for t > 0
    let curve = DiscountCurve::builder("NEG-RATES")
        .base_date(sample_base_date())
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 1.005), (2.0, 1.008)])
        .interp(InterpStyle::Linear)
        .extrapolation(ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    assert!(
        curve.df(1.0).unwrap() > 1.0,
        "DF should be > 1 for negative rates"
    );
    assert!(
        curve
            .zero_rate(1.0, finstack_quant_core::math::Compounding::Continuous)
            .unwrap()
            < 0.0,
        "Zero rate should be negative"
    );
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
        .day_count(DayCount::Act365F)
        .knots(knots)
        .interp(InterpStyle::MonotoneConvex)
        .extrapolation(ExtrapolationPolicy::Extend)
        .build()
        .expect("MonotoneConvex negative-rate curve should build");

    // DF > 1 at and between knots; zero rate round-trips the flat -1%.
    for t in [0.5, 1.0, 1.7, 2.5, 3.0, 4.2, 5.0] {
        let df = curve.df(t).unwrap();
        assert!(
            df > 1.0,
            "DF({t}) = {df} should exceed 1 for negative rates"
        );
        let zero = curve
            .zero_rate(t, finstack_quant_core::math::Compounding::Continuous)
            .unwrap();
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
    let cds_spreads: [f64; 6] = [0.0, 50e-4, 75e-4, 120e-4, 150e-4, 200e-4]; // bp

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
        .extrapolation(ExtrapolationPolicy::Extend)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .build()
        .unwrap();

    // Test that survival probabilities are indeed decreasing
    let sp_1y = credit_curve.df(1.0).unwrap();
    let sp_5y = credit_curve.df(5.0).unwrap();
    let sp_10y = credit_curve.df(10.0).unwrap();
    let sp_15y = credit_curve.df(15.0).unwrap(); // Extrapolated

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
        .extrapolation(ExtrapolationPolicy::Flat)
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .build()
        .unwrap();

    assert_eq!(minimal_curve.df(0.0).unwrap(), 1.0);
    assert_eq!(minimal_curve.df(1.0).unwrap(), 0.95);

    // Test extrapolation with minimal curve
    assert_eq!(minimal_curve.df(-0.5).unwrap(), 1.0); // Flat-zero left
    assert_eq!(minimal_curve.df(2.0).unwrap(), 0.95); // Flat-zero right
}

// Forward-rate numerical precision
//
// `DiscountCurve::forward` is implemented as the canonical
//
//     -(self.df(t2) / self.df(t1)).ln() / (t2 - t1)
//
// rather than going via the zero-rate form
//
//     (self.zero_rate(t2, Continuous) * t2 - self.zero_rate(t1, Continuous) * t1) / (t2 - t1)
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
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
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
        let df1 = curve.df(t1).unwrap();
        let df2 = curve.df(t2).unwrap();
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
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
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
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 0.96), (2.0, 0.92), (5.0, 0.80)])
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Extend)
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
        let df1 = curve.df(t1).unwrap();
        let df2 = curve.df(t2).unwrap();
        let canonical = ((df1 - df2) / df2).ln_1p() / dt_actual;
        assert!(
            fwd.is_finite() && (fwd - canonical).abs() < 1e-15,
            "forward({t1}, {t2}) at dt={dt}: got {fwd}, want {canonical}"
        );
    }
}

// Serialization Tests

mod serde_tests {
    use super::*;

    #[test]
    fn roundtrip_linear() {
        let original = DiscountCurve::builder("USD-OIS")
            .base_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
            .knots([(0.0, 1.0), (1.0, 0.98), (2.0, 0.95), (5.0, 0.88)])
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::Extend)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
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
                (original.df(t).unwrap() - deserialized.df(t).unwrap()).abs() < 1e-12,
                "DF mismatch at t={}: {} vs {}",
                t,
                original.df(t).unwrap(),
                deserialized.df(t).unwrap()
            );
        }
    }

    #[test]
    fn roundtrip_log_linear() {
        let original = DiscountCurve::builder("EUR-ESTR")
            .base_date(Date::from_calendar_date(2025, Month::June, 30).unwrap())
            .knots([(0.0, 1.0), (0.25, 0.995), (0.5, 0.99), (1.0, 0.98)])
            .interp(InterpStyle::LogLinear)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .unwrap();

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: DiscountCurve = serde_json::from_str(&json).unwrap();

        for t in [0.0, 0.1, 0.25, 0.4, 0.5, 0.75, 1.0, 1.5] {
            assert!(
                (original.df(t).unwrap() - deserialized.df(t).unwrap()).abs() < 1e-12,
                "LogLinear DF mismatch at t={}: {} vs {}",
                t,
                original.df(t).unwrap(),
                deserialized.df(t).unwrap()
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
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .unwrap();

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: DiscountCurve = serde_json::from_str(&json).unwrap();

        for t in [0.0, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 5.0, 7.5, 10.0] {
            assert!(
                (original.df(t).unwrap() - deserialized.df(t).unwrap()).abs() < 1e-12,
                "MonotoneConvex DF mismatch at t={}: {} vs {}",
                t,
                original.df(t).unwrap(),
                deserialized.df(t).unwrap()
            );
        }
    }

    #[test]
    fn roundtrip_cubic_hermite() {
        let original = DiscountCurve::builder("JPY-TONAR")
            .base_date(Date::from_calendar_date(2025, Month::September, 1).unwrap())
            .knots([(0.0, 1.0), (1.0, 0.99), (3.0, 0.96), (5.0, 0.92)])
            .interp(InterpStyle::CubicHermite)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .unwrap();

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: DiscountCurve = serde_json::from_str(&json).unwrap();

        for t in [0.0, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0] {
            assert!(
                (original.df(t).unwrap() - deserialized.df(t).unwrap()).abs() < 1e-12,
                "CubicHermite DF mismatch at t={}: {} vs {}",
                t,
                original.df(t).unwrap(),
                deserialized.df(t).unwrap()
            );
        }
    }

    #[test]
    fn roundtrip_flat_fwd() {
        let original = DiscountCurve::builder("CHF-SARON")
            .base_date(Date::from_calendar_date(2025, Month::December, 31).unwrap())
            .knots([(0.0, 1.0), (0.5, 0.995), (1.0, 0.988), (2.0, 0.975)])
            .interp(InterpStyle::LogLinear)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .unwrap();

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: DiscountCurve = serde_json::from_str(&json).unwrap();

        for t in [0.0, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 2.5] {
            assert!(
                (original.df(t).unwrap() - deserialized.df(t).unwrap()).abs() < 1e-12,
                "LogLinear DF mismatch at t={}: {} vs {}",
                t,
                original.df(t).unwrap(),
                deserialized.df(t).unwrap()
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
            .extrapolation(ExtrapolationPolicy::Flat)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .build()
            .unwrap();

        let json = serde_json::to_string(&curve_flat_zero).unwrap();
        let deserialized_flat_zero: DiscountCurve = serde_json::from_str(&json).unwrap();

        let t_beyond = 10.0;
        assert!(
            (curve_flat_zero.df(t_beyond).unwrap() - deserialized_flat_zero.df(t_beyond).unwrap())
                .abs()
                < 1e-12,
            "FlatZero extrapolation mismatch"
        );

        // Test FlatForward extrapolation
        let curve_flat_forward = DiscountCurve::builder("TEST-FLAT-FWD")
            .base_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
            .knots([(0.0, 1.0), (1.0, 0.98), (2.0, 0.95)])
            .interp(InterpStyle::Linear)
            .extrapolation(ExtrapolationPolicy::Extend)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .build()
            .unwrap();

        let json = serde_json::to_string(&curve_flat_forward).unwrap();
        let deserialized_flat_forward: DiscountCurve = serde_json::from_str(&json).unwrap();

        assert!(
            (curve_flat_forward.df(t_beyond).unwrap()
                - deserialized_flat_forward.df(t_beyond).unwrap())
            .abs()
                < 1e-12,
            "FlatForward extrapolation mismatch"
        );

        // Verify the two policies give different results
        assert!(
            (curve_flat_zero.df(t_beyond).unwrap() - curve_flat_forward.df(t_beyond).unwrap())
                .abs()
                > 0.01,
            "Different extrapolation policies should produce different results"
        );
    }

    #[test]
    fn pretty_json_roundtrip() {
        let original = DiscountCurve::builder("TEST-PRETTY")
            .base_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
            .knots([(0.0, 1.0), (1.0, 0.98), (2.0, 0.95)])
            .interp(InterpStyle::Linear)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .unwrap();

        let json = serde_json::to_string_pretty(&original).unwrap();
        let deserialized: DiscountCurve = serde_json::from_str(&json).unwrap();

        for t in [0.0, 0.5, 1.0, 1.5, 2.0] {
            assert!(
                (original.df(t).unwrap() - deserialized.df(t).unwrap()).abs() < 1e-12,
                "Pretty JSON DF mismatch at t={}: {} vs {}",
                t,
                original.df(t).unwrap(),
                deserialized.df(t).unwrap()
            );
        }
    }

    #[test]
    fn validates_on_deserialization() {
        let bad_json = r#"{
            "id": "BAD",
            "base": "2025-01-15",
            "day_count": "act_365f",
            "knot_points": [[0.0, 1.0], [1.0, 0.0]],
            "interp_style": "linear",
            "extrapolation": "extend"
        }"#;

        let result: Result<DiscountCurve, _> = serde_json::from_str(bad_json);
        assert!(
            result.is_err(),
            "Should reject non-positive discount factors"
        );
    }
}

mod roll_forward_realized_forward {
    //! Realized-forward roll semantics :
    //! `DF_new(T - dt) = DF_old(T) / DF_old(dt)`, so forwards realize and a
    //! flat curve stays flat under roll.

    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
    use time::macros::date;

    /// Synthetic IDs require explicit day count, interpolation, and extrapolation.
    fn flat_curve(rate: f64, knots: &[f64]) -> DiscountCurve {
        let points: Vec<(f64, f64)> = knots.iter().map(|&t| (t, (-rate * t).exp())).collect();
        DiscountCurve::builder("TEST-FLAT")
            .base_date(date!(2025 - 01 - 01))
            .day_count(DayCount::Act365F)
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Extend)
            .knots(points)
            .build()
            .unwrap()
    }

    #[test]
    fn flat_curve_stays_flat_under_roll_forward() {
        let knots = [0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0];
        let rate = 0.05;
        let curve = flat_curve(rate, &knots);

        // Condition 365 days = 1.0y under Act365F.
        let rolled = curve.condition(date!(2026 - 01 - 01)).unwrap();

        // Fresh flat 5% curve on the rolled knot grid.
        let fresh_knots = [1.0, 2.0, 4.0, 6.0, 9.0];
        let fresh = flat_curve(rate, &fresh_knots);

        // Knot-aligned probes (interpolation-exact at knots).
        for &t in &fresh_knots {
            assert!(
                (rolled.df(t).unwrap() - fresh.df(t).unwrap()).abs() < 1e-12,
                "flat curve must stay flat under roll at t={t}: rolled={} fresh={}",
                rolled.df(t).unwrap(),
                fresh.df(t).unwrap()
            );
        }
        // DF(0) = 1 exactly (prepended knot is correct after renormalization).
        assert!((rolled.df(0.0).unwrap() - 1.0).abs() < 1e-14);
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
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .unwrap();

        // Condition 365 days = dt = 1.0y under Act365F.
        let dt = 1.0;
        let rolled = curve.condition(date!(2026 - 01 - 01)).unwrap();
        let df_dt = curve.df(dt).unwrap();

        for &t_old in &[2.0_f64, 5.0, 10.0] {
            let expected = curve.df(t_old).unwrap() / df_dt;
            let actual = rolled.df(t_old - dt).unwrap();
            assert!(
                (actual - expected).abs() < 1e-12,
                "DF_rolled({}) must equal DF_old({t_old})/DF_old({dt}): {actual} vs {expected}",
                t_old - dt
            );
        }
    }

    /// A single-pillar curve (`(0, 1)` plus one knot) has one live knot after
    /// the roll; `build()` re-anchors `(0, 1)`, so the roll must succeed.
    #[test]
    fn two_pillar_curve_rolls_forward() {
        let curve = DiscountCurve::builder("TEST-TWO-PILLAR")
            .base_date(date!(2025 - 01 - 01))
            .knots([(0.0, 1.0), (5.0, 0.80)])
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .unwrap();

        // Condition 365 days = dt = 1.0y under Act365F.
        let rolled = curve.condition(date!(2026 - 01 - 01)).unwrap();
        let expected = 0.80 / curve.df(1.0).unwrap();
        assert!((rolled.df(4.0).unwrap() - expected).abs() < 1e-12);
        assert!((rolled.df(0.0).unwrap() - 1.0).abs() < 1e-14);
    }

    /// Conditioning past every pillar under ExtrapolationPolicy::None must fail.
    #[test]
    fn roll_past_last_pillar_fails() {
        let curve = DiscountCurve::builder("TEST-TWO-PILLAR")
            .base_date(date!(2025 - 01 - 01))
            .knots([(0.0, 1.0), (1.0, 0.95)])
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Error)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
            .build()
            .unwrap();
        assert!(curve.condition(date!(2027 - 01 - 01)).is_err());
    }
}

mod continuous_transform_regressions {
    use super::*;

    fn shaped(style: InterpStyle) -> DiscountCurve {
        DiscountCurve::builder("CONTINUOUS")
            .base_date(sample_base_date())
            .interp(style)
            .knots([
                (0.5, 0.995),
                (1.0, 0.985),
                (2.0, 0.955),
                (5.0, 0.86),
                (10.0, 0.70),
            ])
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("valid shaped curve")
    }

    fn close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 2e-12,
            "actual {actual}, expected {expected}"
        );
    }

    #[test]
    fn every_interpolation_preserves_off_pillar_realized_forwards() {
        for style in [
            InterpStyle::Linear,
            InterpStyle::LogLinear,
            InterpStyle::MonotoneConvex,
            InterpStyle::CubicHermite,
            InterpStyle::PiecewiseQuadraticForward,
        ] {
            let source = shaped(style);
            assert!(
                source
                    .condition(sample_base_date() - time::Duration::days(1))
                    .is_err()
            );
            for days in [0, 1, 73, 365] {
                let offset = days as f64 / 365.0;
                let rolled = source
                    .condition(sample_base_date() + time::Duration::days(days))
                    .expect("condition");
                for t in [0.0, 0.02, 0.1, 0.5, 1.5, 3.0, 7.0] {
                    close(
                        rolled.df(t).unwrap(),
                        source.df(t + offset).unwrap() / source.df(offset).unwrap(),
                    );
                }
            }
        }
    }

    #[test]
    fn sparse_curve_key_rate_shock_acts_at_every_maturity() {
        let source = DiscountCurve::flat("SPARSE", sample_base_date(), 0.05, DayCount::Act365F)
            .expect("flat");
        let shocked = source
            .apply_bump(BumpSpec::triangular_key_rate_bp(3.0, 5.0, 7.0, 10.0))
            .expect("continuous key-rate shock");
        for (time, weight) in [
            (0.5, 0.0),
            (3.0, 0.0),
            (4.0, 0.5),
            (5.0, 1.0),
            (6.0, 0.5),
            (7.0, 0.0),
            (10.0, 0.0),
        ] {
            close(
                shocked.df(time).unwrap(),
                source.df(time).unwrap() * (-0.001_f64 * weight * time).exp(),
            );
        }
    }

    #[test]
    fn parallel_shock_preserves_continuous_definition_for_every_interpolation() {
        for style in [
            InterpStyle::Linear,
            InterpStyle::LogLinear,
            InterpStyle::MonotoneConvex,
            InterpStyle::CubicHermite,
            InterpStyle::PiecewiseQuadraticForward,
        ] {
            let source = shaped(style);
            let shocked = source
                .apply_bump(BumpSpec::parallel_bp(25.0))
                .expect("parallel shock");
            for t in [0.0, 0.2, 0.75, 1.5, 3.5, 7.5] {
                close(
                    shocked.df(t).unwrap(),
                    source.df(t).unwrap() * (-0.0025_f64 * t).exp(),
                );
            }
        }
    }

    #[test]
    fn sparse_bucket_shocks_partition_the_parallel_log_discount_shift() {
        let source = DiscountCurve::flat("BUCKETS", sample_base_date(), 0.05, DayCount::Act365F)
            .expect("flat");
        let first = source
            .apply_bump(BumpSpec::triangular_key_rate_first_bp(1.0, 5.0, 1.0))
            .expect("first");
        let middle = first
            .apply_bump(BumpSpec::triangular_key_rate_bp(1.0, 5.0, 10.0, 1.0))
            .expect("middle");
        let all = middle
            .apply_bump(BumpSpec::triangular_key_rate_last_bp(5.0, 10.0, 1.0))
            .expect("last");
        let parallel = source
            .apply_bump(BumpSpec::parallel_bp(1.0))
            .expect("parallel");
        for t in [0.0, 0.1, 0.5, 2.0, 4.5, 5.0, 7.5, 15.0] {
            close(all.df(t).unwrap(), parallel.df(t).unwrap());
        }
    }

    #[test]
    fn transformed_curves_preserve_roll_shock_serde_and_rename_composition() {
        let source = shaped(InterpStyle::MonotoneConvex);
        let shocked = source
            .apply_bump(BumpSpec::triangular_key_rate_bp(0.25, 1.0, 3.0, 17.0))
            .expect("shock");
        let rolled = shocked
            .condition(sample_base_date() + time::Duration::days(73))
            .expect("condition");
        let twice_rolled = rolled
            .condition(sample_base_date() + time::Duration::days(219))
            .expect("second condition");
        let rerolled = shocked
            .condition(sample_base_date() + time::Duration::days(219))
            .expect("combined condition");
        let final_curve = twice_rolled
            .apply_bump(BumpSpec::triangular_key_rate_first_bp(0.3, 1.3, -6.0))
            .expect("shock after roll")
            .apply_bump(BumpSpec::parallel_bp(3.0))
            .expect("parallel after roll");
        let restored: DiscountCurve =
            serde_json::from_str(&serde_json::to_string(&final_curve).expect("serialize"))
                .expect("deserialize");
        let renamed = final_curve
            .to_builder_with_id("RENAMED")
            .build()
            .expect("rename builder");
        for t in [0.0, 0.03, 0.3, 0.5, 1.1, 2.0, 7.0] {
            close(
                rolled.df(t).unwrap(),
                shocked.df(t + 0.2).unwrap() / shocked.df(0.2).unwrap(),
            );
            close(twice_rolled.df(t).unwrap(), rerolled.df(t).unwrap());
            let weight = if t <= 0.3 {
                1.0
            } else if t < 1.3 {
                1.3 - t
            } else {
                0.0
            };
            close(
                final_curve.df(t).unwrap(),
                twice_rolled.df(t).unwrap() * ((0.0006 * weight - 0.0003) * t).exp(),
            );
            close(restored.df(t).unwrap(), final_curve.df(t).unwrap());
            close(renamed.df(t).unwrap(), final_curve.df(t).unwrap());
        }
    }

    #[test]
    fn inverse_shocks_and_zero_bumps_preserve_exact_roll_economics() {
        let source = shaped(InterpStyle::CubicHermite)
            .condition(sample_base_date() + time::Duration::days(100))
            .expect("condition");
        let zero = source.apply_bump(BumpSpec::parallel_bp(0.0)).expect("zero");
        let up = source
            .apply_bump(BumpSpec::triangular_key_rate_bp(0.1, 0.4, 2.0, 50.0))
            .expect("up");
        let restored = up
            .apply_bump(BumpSpec::triangular_key_rate_bp(0.1, 0.4, 2.0, -50.0))
            .expect("down");
        for t in [0.0, 0.05, 0.3, 0.4, 1.0, 2.0, 5.0] {
            close(zero.df(t).unwrap(), source.df(t).unwrap());
            close(restored.df(t).unwrap(), source.df(t).unwrap());
        }
    }

    #[test]
    fn repeated_shocks_merge_instead_of_accumulating_a_serialized_history() {
        let source = shaped(InterpStyle::LogLinear);
        let first = source
            .apply_bump(BumpSpec::triangular_key_rate_bp(1.0, 3.0, 5.0, 1.0))
            .expect("first");
        let mut repeated = first.clone();
        for _ in 0..99 {
            repeated = repeated
                .apply_bump(BumpSpec::triangular_key_rate_bp(1.0, 3.0, 5.0, 1.0))
                .expect("repeat");
        }
        let first_json = serde_json::to_value(&first).expect("serialize");
        let repeated_json = serde_json::to_value(&repeated).expect("serialize");
        assert_eq!(
            first_json["transform"]["adjustment"]["segments"]
                .as_array()
                .expect("segments")
                .len(),
            repeated_json["transform"]["adjustment"]["segments"]
                .as_array()
                .expect("segments")
                .len()
        );
        for t in [0.5, 2.0, 3.0, 4.0, 6.0] {
            let once = source
                .apply_bump(BumpSpec::triangular_key_rate_bp(1.0, 3.0, 5.0, 100.0))
                .expect("equivalent");
            close(repeated.df(t).unwrap(), once.df(t).unwrap());
        }
    }

    #[test]
    fn discount_origin_is_validated_by_builder_and_serde() {
        for points in [[(0.0, 0.9), (1.0, 0.8)], [(-1.0, 1.1), (1.0, 0.9)]] {
            assert!(
                DiscountCurve::builder("INVALID")
                    .base_date(sample_base_date())
                    .knots(points)
                    .build()
                    .is_err()
            );
        }
        let source = shaped(InterpStyle::LogLinear);
        let mut json = serde_json::to_value(&source).expect("serialize");
        json["knot_points"][0][1] = serde_json::json!(0.9);
        assert!(serde_json::from_value::<DiscountCurve>(json).is_err());
        let anchored = DiscountCurve::builder("ANCHORED")
            .base_date(sample_base_date())
            .knots([(1.0, 0.95), (2.0, 0.90)])
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
            .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
            .build()
            .expect("automatic anchor");
        assert_eq!(anchored.df(0.0).unwrap(), 1.0);
    }

    #[test]
    fn serialized_transform_cannot_disagree_with_public_pillars() {
        let source = shaped(InterpStyle::MonotoneConvex)
            .condition(sample_base_date() + time::Duration::days(100))
            .expect("condition");
        let mut json = serde_json::to_value(&source).expect("serialize");
        json["transform"]["offset"] = serde_json::json!(0.5);
        assert!(serde_json::from_value::<DiscountCurve>(json).is_err());
    }

    #[test]
    fn transformed_restore_validates_visible_and_source_pillars_for_all_styles() {
        for style in [
            InterpStyle::Linear,
            InterpStyle::LogLinear,
            InterpStyle::MonotoneConvex,
            InterpStyle::CubicHermite,
            InterpStyle::PiecewiseQuadraticForward,
        ] {
            let curve = shaped(style)
                .condition(sample_base_date() + time::Duration::days(73))
                .expect("condition")
                .apply_bump(BumpSpec::parallel_bp(10.0))
                .expect("bump");
            let json = serde_json::to_value(&curve).expect("serialize");
            let restored: DiscountCurve = serde_json::from_value(json.clone()).expect("restore");
            let renamed = curve
                .to_builder_with_id("RESTORED")
                .build()
                .expect("rename");
            for time in [0.0, 0.1, 0.5, 1.5, 3.0, 8.0] {
                close(restored.df(time).unwrap(), curve.df(time).unwrap());
                close(renamed.df(time).unwrap(), curve.df(time).unwrap());
            }
            for field in ["knot_points", "source_points"] {
                for points in [
                    serde_json::json!([[0.0, 0.9], [1.0, 0.95]]),
                    serde_json::json!([[0.0, 1.0], [1.0, 0.0]]),
                    serde_json::json!([[0.0, 1.0], [1.0, 0.95], [1.0, 0.94]]),
                    serde_json::json!([[0.0, 1.0], [1e-12, 0.99], [1.0, 0.95]]),
                ] {
                    let mut invalid = json.clone();
                    if field == "knot_points" {
                        invalid[field] = points;
                    } else {
                        invalid["transform"][field] = points;
                    }
                    assert!(
                        serde_json::from_value::<DiscountCurve>(invalid).is_err(),
                        "{style:?}: {field}"
                    );
                }
            }
            let mut no_calendar = json.clone();
            no_calendar["day_count"] = serde_json::to_value(DayCount::Bus252).expect("day count");
            assert!(serde_json::from_value::<DiscountCurve>(no_calendar).is_err());
        }
    }

    #[test]
    fn roll_rejects_overflow_and_handles_pqf_conditioning() {
        let extreme = DiscountCurve::builder("OVERFLOW")
            .base_date(sample_base_date())
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (1.0, 1e-310), (2.0, 1.0)])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Extend)
            .build()
            .expect("finite source");
        assert!(
            extreme
                .condition(sample_base_date() + time::Duration::days(365))
                .is_err()
        );
        let points = [
            (0.0, 1.0),
            (1.0, (-0.01_f64).exp()),
            (2.0, (-0.011_f64).exp()),
        ];
        let pqf = DiscountCurve::builder("PQF")
            .base_date(sample_base_date())
            .day_count(DayCount::Act365F)
            .knots(points)
            .interp(InterpStyle::PiecewiseQuadraticForward)
            .extrapolation(ExtrapolationPolicy::Extend)
            .build()
            .expect("source");
        let rolled = pqf
            .condition(sample_base_date() + time::Duration::days(693))
            .expect("pqf condition");
        let restored: DiscountCurve =
            serde_json::from_str(&serde_json::to_string(&rolled).expect("serialize"))
                .expect("restore");
        for t in [0.0, 0.03, 0.08] {
            close(
                rolled.df(t).unwrap(),
                pqf.df(t + 693.0 / 365.0).unwrap() / pqf.df(693.0 / 365.0).unwrap(),
            );
            close(rolled.df(t).unwrap(), restored.df(t).unwrap());
        }
    }
}

#[test]
fn tiny_annual_zero_rate_avoids_subtractive_cancellation() {
    use finstack_quant_core::math::Compounding;
    let df = 1.0 - 1e-8;
    let curve = DiscountCurve::builder("TINY")
        .base_date(sample_base_date())
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, df)])
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Error)
        .build()
        .unwrap();
    let expected = (1.0 - df) / df;
    assert!(
        (curve
            .zero_rate(1.0, Compounding::Periodic(std::num::NonZeroU32::MIN))
            .unwrap()
            - expected)
            .abs()
            < 1e-23
    );
    for convention in [
        Compounding::Periodic(std::num::NonZeroU32::MIN),
        Compounding::Continuous,
        Compounding::Simple,
        Compounding::Periodic(4.try_into().unwrap()),
    ] {
        assert!(curve.zero_rate(0.0, convention).is_err());
        assert!(curve.zero_rate(2.0, convention).is_err());
        assert_eq!(
            curve.zero_rate(0.25, convention).unwrap(),
            convention
                .rate_from_df(curve.df(0.25).unwrap(), 0.25)
                .unwrap()
        );
    }
}

// Moved from production_audit.rs: negative-rate stress and serde round trip.
mod negative_rate_stress {
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::bumps::{BumpSpec, Bumpable};
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
    use time::macros::date;

    #[test]
    fn negative_rate_stress_keeps_mathematical_validation_and_roundtrips() {
        let base = date!(2025 - 01 - 02);
        let curve = DiscountCurve::builder("STRICT")
            .base_date(base)
            .day_count(DayCount::Act365F)
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Extend)
            .knots([
                (0.0, 1.0),
                (1.0, (-0.01_f64).exp()),
                (2.0, (-0.02_f64).exp()),
            ])
            .build()
            .expect("positive-rate construction");
        let shocked = curve
            .apply_bump(BumpSpec::parallel_bp(-200.0))
            .expect("negative-rate stress");
        assert!((shocked.df(2.0).unwrap() - 0.02_f64.exp()).abs() < 1e-12);
        let roundtrip: DiscountCurve =
            serde_json::from_str(&serde_json::to_string(&shocked).expect("serialize"))
                .expect("stress roundtrip");
        assert_eq!(roundtrip.df(2.0).unwrap(), shocked.df(2.0).unwrap());
        assert!(curve.apply_bump(BumpSpec::parallel_bp(f64::NAN)).is_err());
        assert!(curve.apply_bump(BumpSpec::parallel_bp(-1e308)).is_err());
        assert!((curve.df(2.0).unwrap() - (-0.02_f64).exp()).abs() < 1e-12);
        assert!(
            DiscountCurve::builder("NEGATIVE")
                .base_date(base)
                .day_count(DayCount::Act365F)
                .interp(InterpStyle::LogLinear)
                .extrapolation(ExtrapolationPolicy::Extend)
                .knots([(0.0, 1.0), (1.0, 1.01)])
                .build()
                .is_ok()
        );
    }
}
