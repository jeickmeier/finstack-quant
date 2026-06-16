//! Tests for ForwardCurve functionality.
//!
//! This module covers:
//! - Builder validation and construction
//! - Interpolation styles
//! - Rate calculations
//! - Clone safety (panic-free)
//! - Serialization roundtrips

use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use time::Month;

fn test_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

// =============================================================================
// Clone Safety Tests
// =============================================================================

/// Verifies that cloning a ForwardCurve is infallible and produces identical results.
#[test]
fn clone_is_panic_free_and_equivalent() {
    let original = ForwardCurve::builder("USD-SOFR3M", 0.25)
        .base_date(test_date())
        .reset_lag(2)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 0.03),
            (0.25, 0.032),
            (0.5, 0.035),
            (1.0, 0.04),
            (2.0, 0.042),
            (5.0, 0.045),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    // Clone should not panic
    let cloned = original.clone();

    // Verify structural equality
    assert_eq!(original.id(), cloned.id());
    assert_eq!(original.base_date(), cloned.base_date());
    assert_eq!(original.reset_lag(), cloned.reset_lag());
    assert_eq!(original.day_count(), cloned.day_count());
    assert_eq!(original.tenor(), cloned.tenor());
    assert_eq!(original.knots(), cloned.knots());
    assert_eq!(original.forwards(), cloned.forwards());

    // Verify interpolation produces identical results
    for t in [0.0, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0] {
        let orig_rate = original.rate(t);
        let cloned_rate = cloned.rate(t);
        assert!(
            (orig_rate - cloned_rate).abs() < 1e-14,
            "Rate mismatch after clone at t={}: {} vs {}",
            t,
            orig_rate,
            cloned_rate
        );
    }
}

/// Verifies clone works correctly for all interpolation styles.
#[test]
fn clone_works_for_all_interp_styles() {
    let interp_styles = [
        InterpStyle::Linear,
        InterpStyle::LogLinear,
        InterpStyle::CubicHermite,
        InterpStyle::PiecewiseQuadraticForward,
    ];

    for style in interp_styles {
        let curve = ForwardCurve::builder("TEST", 0.25)
            .base_date(test_date())
            .knots([(0.0, 0.03), (1.0, 0.04), (5.0, 0.05)])
            .interp(style)
            .build()
            .unwrap();

        // Clone should not panic for any interpolation style
        let cloned = curve.clone();

        // Verify rates match
        for t in [0.0, 0.5, 1.0, 3.0, 5.0] {
            let orig_rate = curve.rate(t);
            let cloned_rate = cloned.rate(t);
            assert!(
                (orig_rate - cloned_rate).abs() < 1e-14,
                "Clone rate mismatch for {:?} at t={}: {} vs {}",
                style,
                t,
                orig_rate,
                cloned_rate
            );
        }
    }
}

/// Verifies clone works correctly with different extrapolation policies.
#[test]
fn clone_works_for_all_extrapolation_policies() {
    let policies = [
        ExtrapolationPolicy::FlatZero,
        ExtrapolationPolicy::FlatForward,
    ];

    for policy in policies {
        let curve = ForwardCurve::builder("TEST", 0.25)
            .base_date(test_date())
            .knots([(0.0, 0.03), (1.0, 0.04), (5.0, 0.05)])
            .extrapolation(policy)
            .build()
            .unwrap();

        // Clone should not panic for any extrapolation policy
        let cloned = curve.clone();

        // Verify extrapolated rates match beyond the knot range
        for t in [0.0, 2.5, 5.0, 10.0, 20.0] {
            let orig_rate = curve.rate(t);
            let cloned_rate = cloned.rate(t);
            assert!(
                (orig_rate - cloned_rate).abs() < 1e-14,
                "Clone rate mismatch for {:?} at t={}: {} vs {}",
                policy,
                t,
                orig_rate,
                cloned_rate
            );
        }
    }
}

#[test]
fn builder_requires_explicit_base_date() {
    let result = ForwardCurve::builder("USD-SOFR3M", 0.25)
        .knots([(0.0, 0.03), (1.0, 0.04)])
        .build();
    assert!(
        result.is_err(),
        "forward curve should require an explicit base date"
    );
}

#[test]
fn builder_rejects_non_positive_tenor() {
    let result = ForwardCurve::builder("USD-SOFR0", 0.0)
        .base_date(test_date())
        .knots([(0.0, 0.03), (1.0, 0.04)])
        .build();
    assert!(
        result.is_err(),
        "forward curve tenor must be strictly positive"
    );
}

#[test]
fn df_chains_simple_forward_periods() {
    let curve = ForwardCurve::builder("USD-SOFR6M", 0.5)
        .base_date(test_date())
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.05), (1.0, 0.05)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    assert_eq!(curve.df(0.0).unwrap(), 1.0);
    let one_year_df = curve.df(1.0).unwrap();
    let expected = 1.0 / (1.0_f64 + 0.05 * 0.5).powi(2);
    assert!((one_year_df - expected).abs() < 1e-14);
}

#[test]
fn df_rejects_invalid_times_and_accrual_steps() {
    let curve = ForwardCurve::builder("USD-SOFR6M", 0.5)
        .base_date(test_date())
        .knots([(0.0, 0.03), (1.0, 0.03)])
        .build()
        .unwrap();

    assert!(curve.df(f64::NAN).is_err());
    assert!(curve.df(-0.25).is_err());

    let pathological = ForwardCurve::builder("USD-SOFR6M", 0.5)
        .base_date(test_date())
        .knots([(0.0, -3.0), (1.0, -3.0)])
        .build()
        .unwrap();
    assert!(
        pathological.df(1.0).is_err(),
        "negative forwards that imply non-positive accrual denominators must error"
    );
}

#[test]
fn df_on_date_curve_uses_curve_day_count() {
    let curve = ForwardCurve::builder("USD-SOFR6M", 0.5)
        .base_date(test_date())
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.05), (1.0, 0.05)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let date = test_date() + time::Duration::days(180);

    let df = curve.df_on_date_curve(date).unwrap();
    let expected = 1.0 / (1.0 + 0.05 * 0.5);
    assert!((df - expected).abs() < 1e-14);
}

#[test]
fn non_linear_df_uses_rate_period_path() {
    let curve = ForwardCurve::builder("USD-SOFR6M", 0.5)
        .base_date(test_date())
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.05), (0.5, 0.05), (1.0, 0.05)])
        .interp(InterpStyle::CubicHermite)
        .build()
        .unwrap();

    let df = curve.df(1.0).unwrap();
    let expected = 1.0 / (1.0_f64 + 0.05 * 0.5).powi(2);
    assert!((df - expected).abs() < 1e-12);
}

#[test]
fn rate_period_matches_linear_curve_integral_average() {
    let curve = ForwardCurve::builder("USD-SOFR6M", 0.5)
        .base_date(test_date())
        .knots([(0.0, 0.02), (10.0, 0.12)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let avg = curve.rate_period(2.0, 8.0);
    let expected = 0.02 + 0.01 * ((2.0 + 8.0) / 2.0);
    assert!((avg - expected).abs() < 1e-14);
}

#[test]
fn rate_period_handles_zero_width_and_long_flat_intervals() {
    let curve = ForwardCurve::builder("USD-SOFR6M", 0.5)
        .base_date(test_date())
        .knots([(0.0, 0.04), (30.0, 0.04)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    assert_eq!(curve.rate_period(3.0, 3.0), curve.rate(3.0));
    assert!((curve.rate_period(0.0, 25.0) - 0.04).abs() < 1e-14);
}

// =============================================================================
// Serialization Tests
// =============================================================================

mod serde_tests {
    use super::*;

    #[test]
    fn roundtrip_with_all_fields() {
        let original = ForwardCurve::builder("USD-SOFR3M", 0.25)
            .base_date(test_date())
            .reset_lag(2)
            .day_count(DayCount::Act360)
            .knots([
                (0.0, 0.03),
                (0.25, 0.032),
                (0.5, 0.035),
                (1.0, 0.04),
                (2.0, 0.042),
                (5.0, 0.045),
            ])
            .interp(InterpStyle::Linear)
            .build()
            .unwrap();

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: ForwardCurve = serde_json::from_str(&json).unwrap();

        assert_eq!(original.id(), deserialized.id());
        assert_eq!(original.base_date(), deserialized.base_date());
        assert_eq!(original.reset_lag(), deserialized.reset_lag());
        assert_eq!(original.day_count(), deserialized.day_count());
        assert_eq!(original.tenor(), deserialized.tenor());
        assert_eq!(original.knots(), deserialized.knots());
        assert_eq!(original.forwards(), deserialized.forwards());

        // Test rate interpolation
        for t in [0.0, 0.1, 0.25, 0.4, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 5.0] {
            let original_rate = original.rate(t);
            let deserialized_rate = deserialized.rate(t);
            assert!(
                (original_rate - deserialized_rate).abs() < 1e-12,
                "Forward rate mismatch at t={}: {} vs {}",
                t,
                original_rate,
                deserialized_rate
            );
        }
    }

    #[test]
    fn roundtrip_multiple_interp_styles() {
        // Note: MonotoneConvex interprets values as discount factors (Hagan-West
        // forward-rate construction), so it's not suitable for forward rates
        let interp_styles = [
            InterpStyle::Linear,
            InterpStyle::LogLinear,
            InterpStyle::CubicHermite,
        ];

        for style in interp_styles {
            let original = ForwardCurve::builder("EUR-EURIBOR6M", 0.5)
                .base_date(Date::from_calendar_date(2025, Month::June, 30).unwrap())
                .reset_lag(2)
                .day_count(DayCount::Act360)
                .knots([(0.0, 0.025), (1.0, 0.03), (2.0, 0.035), (5.0, 0.04)])
                .interp(style)
                .build()
                .unwrap();

            let json = serde_json::to_string(&original).unwrap();
            let deserialized: ForwardCurve = serde_json::from_str(&json).unwrap();

            // Test interpolation accuracy for each style
            for t in [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 5.0] {
                let original_rate = original.rate(t);
                let deserialized_rate = deserialized.rate(t);
                assert!(
                    (original_rate - deserialized_rate).abs() < 1e-12,
                    "Forward rate mismatch for {:?} at t={}: {} vs {}",
                    style,
                    t,
                    original_rate,
                    deserialized_rate
                );
            }
        }
    }
}

// =============================================================================
// Additional forward curve behavior
// =============================================================================

#[test]
fn test_forward_curve_spread_based_construction() {
    // Test spread over base curve
    let _disc_curve =
        finstack_quant_core::market_data::term_structures::DiscountCurve::builder("DISC")
            .base_date(test_date())
            .knots([(0.0, 1.0), (1.0, 0.95), (2.0, 0.90)])
            .build()
            .unwrap();

    let fwd_curve = ForwardCurve::builder("FWD-SPREAD", 0.25)
        .base_date(test_date())
        .knots([(0.0, 0.01), (1.0, 0.015), (2.0, 0.02)]) // Spread (simple forwards)
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    // Verify exact knot values
    assert!((fwd_curve.rate(0.0) - 0.01).abs() < 1e-15);
    assert!((fwd_curve.rate(1.0) - 0.015).abs() < 1e-15);
    assert!((fwd_curve.rate(2.0) - 0.02).abs() < 1e-15);

    // Verify linear interpolation between knots
    assert!((fwd_curve.rate(0.5) - 0.0125).abs() < 1e-15);
    assert!((fwd_curve.rate(1.5) - 0.0175).abs() < 1e-15);
}

#[test]
fn test_forward_curve_tenor_mismatch() {
    // Test with different tenor than knots
    let curve = ForwardCurve::builder("TEST", 0.5) // 6-month tenor
        .base_date(test_date())
        .knots([(0.0, 0.03), (0.25, 0.032), (1.0, 0.04)])
        .build()
        .unwrap();

    assert_eq!(curve.tenor(), 0.5);
    let rate = curve.rate(0.75);
    assert!(rate > 0.03 && rate < 0.05);
}

#[test]
fn test_forward_curve_rate_conversion_continuous() {
    // Test continuous compounding
    let curve = ForwardCurve::builder("TEST", 0.25)
        .base_date(test_date())
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.05), (1.0, 0.05)])
        .build()
        .unwrap();

    let rate = curve.rate(0.5);
    assert!((rate - 0.05).abs() < 0.001);
}

#[test]
fn test_forward_curve_negative_forwards() {
    // Negative rates are allowed
    let curve = ForwardCurve::builder("NEGATIVE", 0.25)
        .base_date(test_date())
        .knots([(0.0, -0.01), (1.0, -0.005)])
        .build()
        .unwrap();

    let rate = curve.rate(0.5);
    assert!(rate < 0.0);
}

#[test]
fn test_forward_curve_inverted() {
    // Inverted yield curve
    let curve = ForwardCurve::builder("INVERTED", 0.25)
        .base_date(test_date())
        .knots([(0.0, 0.05), (1.0, 0.03), (2.0, 0.02)])
        .build()
        .unwrap();

    assert!(curve.rate(0.0) > curve.rate(2.0));
}

#[test]
fn test_forward_curve_very_long_tenors() {
    // Very long dated forwards
    let curve = ForwardCurve::builder("LONG", 0.25)
        .base_date(test_date())
        .knots([(0.0, 0.03), (1.0, 0.035), (10.0, 0.04), (30.0, 0.045)])
        .build()
        .unwrap();

    let rate_30y = curve.rate(30.0);
    assert!((rate_30y - 0.045).abs() < 1e-10);
}

#[test]
fn test_forward_curve_short_tenors() {
    // Very short dated forwards
    let curve = ForwardCurve::builder("SHORT", 0.25)
        .base_date(test_date())
        .knots([(0.0, 0.03), (0.01, 0.031), (0.1, 0.032)])
        .build()
        .unwrap();

    let rate = curve.rate(0.05);
    assert!(rate > 0.03 && rate < 0.033);
}

#[test]
fn test_forward_curve_reset_lag() {
    // Test with various reset lags
    for lag in [0, 1, 2, 5] {
        let curve = ForwardCurve::builder("TEST", 0.25)
            .base_date(test_date())
            .reset_lag(lag)
            .knots([(0.0, 0.03), (1.0, 0.04)])
            .build()
            .unwrap();

        assert_eq!(curve.reset_lag(), lag);
    }
}

#[test]
fn test_forward_curve_day_count_variations() {
    // Test with different day count conventions
    let day_counts = [DayCount::Act360, DayCount::Act365F, DayCount::Thirty360];

    for dc in day_counts {
        let curve = ForwardCurve::builder("TEST", 0.25)
            .base_date(test_date())
            .day_count(dc)
            .knots([(0.0, 0.03), (1.0, 0.04)])
            .build()
            .unwrap();

        assert_eq!(curve.day_count(), dc);
    }
}

#[test]
fn test_forward_curve_single_knot() {
    // Test that single knot is rejected (need at least 2 points)
    let result = ForwardCurve::builder("FLAT", 0.25)
        .base_date(test_date())
        .knots([(0.0, 0.05)])
        .build();

    assert!(result.is_err(), "Single knot should be rejected");
}

#[test]
fn test_forward_curve_many_knots() {
    // Many knots for fine granularity
    let knots: Vec<(f64, f64)> = (0..=20)
        .map(|i| (i as f64 * 0.5, 0.03 + i as f64 * 0.001))
        .collect();

    let curve = ForwardCurve::builder("GRANULAR", 0.25)
        .base_date(test_date())
        .knots(knots)
        .build()
        .unwrap();

    // Should interpolate smoothly
    for t in [0.25, 0.75, 1.5, 5.5] {
        let rate = curve.rate(t);
        assert!(rate > 0.03 && rate < 0.05);
    }
}

#[test]
fn test_forward_curve_serde_all_fields() {
    // Test full serde round-trip with all fields
    let original = ForwardCurve::builder("FULL-TEST", 0.25)
        .base_date(test_date())
        .reset_lag(2)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.03), (1.0, 0.04), (2.0, 0.045)])
        .interp(InterpStyle::CubicHermite)
        .extrapolation(ExtrapolationPolicy::FlatForward)
        .build()
        .unwrap();

    let json = serde_json::to_string(&original).unwrap();
    let deserialized: ForwardCurve = serde_json::from_str(&json).unwrap();

    assert_eq!(original.id(), deserialized.id());
    assert_eq!(original.tenor(), deserialized.tenor());
    assert_eq!(original.reset_lag(), deserialized.reset_lag());
    assert_eq!(original.day_count(), deserialized.day_count());
    assert_eq!(original.knots(), deserialized.knots());

    // Verify rates match
    for t in [0.0, 0.5, 1.0, 1.5, 2.0] {
        assert!((original.rate(t) - deserialized.rate(t)).abs() < 1e-12);
    }
}
